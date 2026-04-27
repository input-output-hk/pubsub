use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use pallas_addresses::Address;
use pallas_crypto::hash::{Hash, Hasher};
use pallas_txbuilder::{BuildConway, ExUnits, Input, Output, ScriptKind, StagingTransaction};

use crate::{
    aiken,
    blockfrost::BlockfrostClient,
    bootstrap::{Network, parse_utxo_ref},
    cbor::{
        cbor_bytes, cbor_constr, cbor_output_ref, cbor_policy_id, cbor_uint, decode_cbor_uint,
    },
    tx::{load_signing_key, parse_policy_id},
};

pub struct CreateTopicArgs {
    pub network: Network,
    pub blockfrost_project_id: String,
    pub payment_addr: String,
    pub payment_skey_path: PathBuf,
    pub contracts_dir: PathBuf,
    pub env_file: PathBuf,
    pub funding_utxo: String,
    pub name: String,
    /// Number of relay nodes that must cache each message.
    pub replication_factor: u64,
    /// Retention window in seconds.
    pub retention_period: u64,
}

// Conservative execution units for the CreateTopic validator call.
// Both the registry spend and the registry mint run the same compiled
// script; we budget each separately at these values.
const CREATE_TOPIC_EX: ExUnits = ExUnits {
    mem: 1_400_000,
    steps: 600_000_000,
};

// Min-ADA for the topic output (token + datum, no reference script).
const TOPIC_OUTPUT_LOVELACE: u64 = 2_000_000;

pub async fn run(args: CreateTopicArgs) -> Result<()> {
    // --- read .env file -------------------------------------------------------
    let env = read_env(&args.env_file)?;
    let topic_bootstrap_utxo = env_var(&env, "PUBSUB_TOPIC_BOOTSTRAP_UTXO")?;
    let registry_addr_str    = env_var(&env, "PUBSUB_TOPIC_REGISTRY_ADDR")?;
    let topic_validator_str  = env_var(&env, "PUBSUB_TOPIC_VALIDATOR_ADDR")?;
    let registry_policy_id   = env_var(&env, "PUBSUB_REGISTRY_POLICY_ID")?;

    // --- re-derive blueprint to get compiled script bytes --------------------
    println!("Re-deriving parameterized blueprints...");
    let tmp = tempfile::tempdir().context("creating temp dir")?;
    let topic_project_dir = args.contracts_dir.join("topic-registry");
    let (topic_tx_hash, topic_tx_ix) = parse_utxo_ref(&topic_bootstrap_utxo)?;
    let topic_cbor = cbor_output_ref(&topic_tx_hash, topic_tx_ix)?;
    let policy_cbor = cbor_policy_id(&registry_policy_id)?;

    let bp_raw = topic_project_dir.join("plutus.json");
    let bp1 = tmp.path().join("bp1.json");
    let bp2 = tmp.path().join("bp2.json");
    let bp3 = tmp.path().join("bp3.json");
    aiken::apply_param(&bp_raw, &bp1, "registry",  "registry",  &topic_cbor)?;
    aiken::apply_param(&bp1,   &bp2,  "topic",     "topic",     &policy_cbor)?;
    aiken::apply_param(&bp2,   &bp3,  "publisher", "publisher", &policy_cbor)?;

    let registry_code = aiken::compiled_code(&bp3, "registry.registry.mint")
        .context("extracting registry script code")?;
    let registry_script = hex::decode(&registry_code)
        .context("decoding registry script hex")?;

    // --- Blockfrost setup + protocol params ----------------------------------
    let bf = BlockfrostClient::new(
        &args.blockfrost_project_id,
        args.network.blockfrost_base_url(),
    );
    println!("Fetching protocol parameters...");
    let params = bf.protocol_params().await.context("fetching protocol params")?;
    let cost_model = params.cost_model_v3()?;

    // --- find registry head UTxO at registry address -------------------------
    println!("Looking up registry head at {registry_addr_str}...");
    let registry_head_token_hex = hex::encode(b"registry_head");
    let registry_head_unit = format!("{registry_policy_id}{registry_head_token_hex}");
    let utxos = bf.utxos_at(&registry_addr_str).await
        .context("fetching registry address UTxOs")?;
    let head_utxo = utxos.iter()
        .find(|u| u.amount.iter().any(|a| a.unit == registry_head_unit))
        .ok_or_else(|| anyhow!("registry_head UTxO not found at {registry_addr_str}"))?;
    println!("  found: {}#{}", head_utxo.tx_hash, head_utxo.tx_index);

    let head_datum_hex = head_utxo.inline_datum.as_deref()
        .ok_or_else(|| anyhow!("registry head UTxO has no inline datum"))?;
    let head_datum_bytes = hex::decode(head_datum_hex)
        .context("decoding registry head datum hex")?;
    let (counter, epoch) = parse_registry_head_datum(&head_datum_bytes)
        .context("parsing RegistryHeadDatum")?;
    println!("  datum: counter={counter}, epoch={epoch}");

    let topic_id = counter; // new topic gets the current counter value

    // --- load signing key + derive payment key hash --------------------------
    let signing_key = load_signing_key(&args.payment_skey_path)
        .context("loading signing key")?;
    let pubkey = signing_key.public_key();
    let payment_pkh: Hash<28> = Hasher::<224>::hash(pubkey.as_ref());
    println!("  payment key hash: {}", hex::encode(payment_pkh.as_ref()));

    // --- fetch funding UTxO --------------------------------------------------
    let (fund_hash, fund_ix) = parse_utxo_ref(&args.funding_utxo)?;
    println!("Looking up funding UTxO {}...", args.funding_utxo);
    let fund_utxo = bf.find_utxo(&args.payment_addr, &fund_hash, fund_ix).await
        .context("finding funding UTxO")?;
    println!("  value: {} lovelace", fund_utxo.lovelace());

    // --- encode datums and redeemers -----------------------------------------
    // MintTopic redeemer = Constr 1 [] (second variant of RegistryMintAction)
    let mint_redeemer = vec![0xd8u8, 0x7a, 0x80];
    // CreateTopic redeemer = Constr 0 [name, rf, rp] (first variant of RegistryHeadAction)
    let spend_redeemer = encode_create_topic_redeemer(&args.name, args.replication_factor, args.retention_period);
    // Updated head datum: counter + 1, epoch unchanged
    let updated_head_datum = encode_registry_head_datum(counter + 1, epoch);
    // New topic datum
    let topic_datum = encode_topic_datum(
        topic_id,
        &args.name,
        payment_pkh.as_ref(),
        args.replication_factor,
        args.retention_period,
        epoch,
    )?;

    // Topic token asset name: 0x74 ('t') ++ bigEndian32(topic_id)
    let mut topic_token_name = vec![0x74u8];
    let topic_id_u32 = u32::try_from(topic_id)
        .map_err(|_| anyhow!("topic_id {topic_id} exceeds u32"))?;
    topic_token_name.extend_from_slice(&topic_id_u32.to_be_bytes());

    // --- build transaction (two-pass fee) ------------------------------------
    let policy_id = parse_policy_id(&registry_policy_id)?;
    let payment_addr_parsed = Address::from_bech32(&args.payment_addr)
        .map_err(|e| anyhow!("invalid payment address: {e}"))?;
    let registry_addr_parsed = Address::from_bech32(&registry_addr_str)
        .map_err(|e| anyhow!("invalid registry address: {e}"))?;
    let topic_validator_parsed = Address::from_bech32(&topic_validator_str)
        .map_err(|e| anyhow!("invalid topic validator address: {e}"))?;

    let head_hash_bytes: [u8; 32] = hex::decode(&head_utxo.tx_hash)
        .context("decoding registry head UTxO hash")?
        .try_into()
        .map_err(|_| anyhow!("registry head UTxO hash must be 32 bytes"))?;
    let fund_hash_bytes: [u8; 32] = hex::decode(&fund_hash)
        .context("decoding funding UTxO hash")?
        .try_into()
        .map_err(|_| anyhow!("funding UTxO hash must be 32 bytes"))?;

    // Both the spend redeemer (registry head) and mint redeemer (topic token)
    // declare CREATE_TOPIC_EX each — the fee covers both independently.
    let exec_fee = params.exec_prices().fee(
        CREATE_TOPIC_EX.mem * 2,
        CREATE_TOPIC_EX.steps * 2,
    );

    println!("\nBuilding create-topic transaction (name={:?}, rf={}, rp={}s)...",
        args.name, args.replication_factor, args.retention_period);

    let tx_bytes = build_create_topic_tx(
        head_hash_bytes, head_utxo.tx_index, head_utxo.lovelace(),
        fund_hash_bytes, fund_ix, fund_utxo.lovelace(),
        &registry_addr_parsed, &topic_validator_parsed, &payment_addr_parsed,
        policy_id, &topic_token_name,
        &updated_head_datum, &topic_datum,
        &registry_script, &cost_model,
        &spend_redeemer, &mint_redeemer,
        payment_pkh, exec_fee, &signing_key,
        500_000, // initial fee estimate
    )?;
    let actual_fee = params.min_fee_a * tx_bytes.len() as u64 + params.min_fee_b + exec_fee;

    let tx_bytes = build_create_topic_tx(
        head_hash_bytes, head_utxo.tx_index, head_utxo.lovelace(),
        fund_hash_bytes, fund_ix, fund_utxo.lovelace(),
        &registry_addr_parsed, &topic_validator_parsed, &payment_addr_parsed,
        policy_id, &topic_token_name,
        &updated_head_datum, &topic_datum,
        &registry_script, &cost_model,
        &spend_redeemer, &mint_redeemer,
        payment_pkh, exec_fee, &signing_key,
        actual_fee,
    )?;

    println!("Submitting...");
    let txid = bf.submit_tx(&tx_bytes).await.context("submitting create-topic tx")?;

    println!("\n{}", "=".repeat(70));
    println!("create-topic complete");
    println!("{}", "=".repeat(70));
    println!("  topic id:   {topic_id}");
    println!("  topic name: {}", args.name);
    println!("  tx:         {txid}");
    println!("  topic UTxO: {txid}#0");
    println!("{}", "=".repeat(70));

    Ok(())
}

// ---------------------------------------------------------------------------
// Transaction builder
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn build_create_topic_tx(
    head_hash: [u8; 32],
    head_ix: u64,
    head_lovelace: u64,
    fund_hash: [u8; 32],
    fund_ix: u64,
    fund_lovelace: u64,
    registry_addr: &Address,
    topic_validator_addr: &Address,
    payment_addr: &Address,
    policy_id: pallas_crypto::hash::Hash<28>,
    topic_token_name: &[u8],
    updated_head_datum: &[u8],
    topic_datum: &[u8],
    registry_script: &[u8],
    cost_model: &[i64],
    spend_redeemer: &[u8],
    mint_redeemer: &[u8],
    payment_pkh: Hash<28>,
    _exec_fee: u64,
    signing_key: &pallas_crypto::key::ed25519::SecretKey,
    fee: u64,
) -> Result<Vec<u8>> {
    let change = fund_lovelace
        .checked_sub(TOPIC_OUTPUT_LOVELACE + fee)
        .ok_or_else(|| anyhow!(
            "insufficient funds: have {} lovelace, need {} (2 ADA topic output + {} fee)",
            fund_lovelace, TOPIC_OUTPUT_LOVELACE + fee, fee
        ))?;

    let head_input = Input::new(head_hash.into(), head_ix);
    let fund_input = Input::new(fund_hash.into(), fund_ix);

    // Output 0: updated registry head (passes through its lovelace + registry_head token)
    let head_output = Output::new(registry_addr.clone(), head_lovelace)
        .add_asset(policy_id, b"registry_head".to_vec(), 1)
        .context("adding registry_head token to head output")?
        .set_inline_datum(updated_head_datum.to_vec());

    // Output 1: new topic UTxO (minted topic token + datum)
    let topic_output = Output::new(topic_validator_addr.clone(), TOPIC_OUTPUT_LOVELACE)
        .add_asset(policy_id, topic_token_name.to_vec(), 1)
        .context("adding topic token to topic output")?
        .set_inline_datum(topic_datum.to_vec());

    // Output 2: change back to payment address
    let change_output = Output::new(payment_addr.clone(), change);

    let staging = StagingTransaction::new()
        .input(head_input.clone())
        .input(fund_input.clone())
        .collateral_input(fund_input)
        .output(head_output)
        .output(topic_output)
        .output(change_output)
        .script(ScriptKind::PlutusV3, registry_script.to_vec())
        .language_view(ScriptKind::PlutusV3, cost_model.to_vec())
        .disclosed_signer(payment_pkh)
        .fee(fee);

    let staging = staging
        .add_spend_redeemer(head_input, spend_redeemer.to_vec(), Some(CREATE_TOPIC_EX));
    let staging = staging
        .mint_asset(policy_id, topic_token_name.to_vec(), 1)
        .context("adding mint")?;
    let staging = staging
        .add_mint_redeemer(policy_id, mint_redeemer.to_vec(), Some(CREATE_TOPIC_EX));

    let built = staging.build_conway_raw()
        .map_err(|e| anyhow!("tx build failed: {e}"))?;

    let sig = signing_key.sign(&built.tx_hash.0);
    let sig_bytes: [u8; 64] = sig.as_ref().try_into()
        .map_err(|_| anyhow!("unexpected signature length"))?;
    let signed = built
        .add_signature(signing_key.public_key(), sig_bytes)
        .map_err(|e| anyhow!("signing failed: {e}"))?;

    Ok(signed.tx_bytes.0)
}

// ---------------------------------------------------------------------------
// CBOR datum / redeemer encoders
// ---------------------------------------------------------------------------

fn encode_registry_head_datum(counter: u64, epoch: u64) -> Vec<u8> {
    cbor_constr(0, &[cbor_uint(counter), cbor_uint(epoch)])
}

fn encode_create_topic_redeemer(name: &str, rf: u64, rp: u64) -> Vec<u8> {
    cbor_constr(0, &[cbor_bytes(name.as_bytes()), cbor_uint(rf), cbor_uint(rp)])
}

fn encode_topic_datum(
    topic_id: u64,
    name: &str,
    owner_pkh: &[u8],
    replication_factor: u64,
    retention_period: u64,
    epoch: u64,
) -> Result<Vec<u8>> {
    if owner_pkh.len() != 28 {
        return Err(anyhow!("owner pkh must be 28 bytes, got {}", owner_pkh.len()));
    }
    let alive_true = vec![0xd8u8, 0x7a, 0x80]; // True = Constr 1 []
    let owners = {
        let mut v = vec![0x81u8]; // 1-element array
        v.extend_from_slice(&cbor_bytes(owner_pkh));
        v
    };
    let admins = vec![0x80u8]; // empty list
    Ok(cbor_constr(0, &[
        cbor_uint(topic_id),
        cbor_bytes(name.as_bytes()),
        owners,
        admins,
        cbor_uint(replication_factor),
        cbor_uint(retention_period),
        alive_true,
        cbor_uint(epoch),
    ]))
}


// ---------------------------------------------------------------------------
// RegistryHeadDatum decoder
// ---------------------------------------------------------------------------

fn parse_registry_head_datum(cbor: &[u8]) -> Result<(u64, u64)> {
    // Expected: d8 79 82 <uint(counter)> <uint(epoch)>
    if cbor.len() < 5 || cbor[0] != 0xd8 || cbor[1] != 0x79 || cbor[2] != 0x82 {
        return Err(anyhow!(
            "expected RegistryHeadDatum (d87982...) but got: {}",
            hex::encode(&cbor[..cbor.len().min(8)])
        ));
    }
    let (counter, rest) = decode_cbor_uint(&cbor[3..])?;
    let (epoch, _)      = decode_cbor_uint(rest)?;
    Ok((counter, epoch))
}

// ---------------------------------------------------------------------------
// .env helpers (duplicated from publish_scripts — TODO: extract to shared mod)
// ---------------------------------------------------------------------------

fn read_env(path: &std::path::Path) -> Result<std::collections::HashMap<String, String>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let mut map = std::collections::HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() { continue; }
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    Ok(map)
}

fn env_var(env: &std::collections::HashMap<String, String>, key: &str) -> Result<String> {
    env.get(key)
        .filter(|v| !v.is_empty())
        .cloned()
        .ok_or_else(|| anyhow!("missing '{key}' in .env file"))
}


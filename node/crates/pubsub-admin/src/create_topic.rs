use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use pallas_addresses::Address;
use pallas_crypto::hash::{Hash, Hasher};
use pallas_txbuilder::{BuildConway, ExUnits, Input, Output, ScriptKind, StagingTransaction};

use crate::{
    aiken,
    blockfrost::BlockfrostClient,
    bootstrap::{Network, split_utxo as parse_utxo_ref},
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
    let node_bootstrap_utxo  = env_var(&env, "PUBSUB_NODE_BOOTSTRAP_UTXO")?;
    let registry_addr_str    = env_var(&env, "PUBSUB_TOPIC_REGISTRY_ADDR")?;
    let topic_validator_str  = env_var(&env, "PUBSUB_TOPIC_VALIDATOR_ADDR")?;
    let registry_policy_id   = env_var(&env, "PUBSUB_REGISTRY_POLICY_ID")?;

    // --- re-derive blueprint to get compiled script bytes --------------------
    println!("Re-deriving parameterized blueprints...");
    let tmp = tempfile::tempdir().context("creating temp dir")?;
    let topic_project_dir = args.contracts_dir.join("topic-registry");
    let (topic_tx_hash, topic_tx_ix) = parse_utxo_ref(&topic_bootstrap_utxo)?;
    let (node_tx_hash, node_tx_ix)   = parse_utxo_ref(&node_bootstrap_utxo)?;
    let _ = (node_tx_hash, node_tx_ix); // not needed for topic creation
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
// CBOR helpers
// ---------------------------------------------------------------------------

fn cbor_constr(n: u8, fields: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    // Constructors 0-6: tags 121-127 (0xd8 0x79 through 0xd8 0x7f)
    // Constructors 7+: tag 102 (0xd8 0x66) with [index, fields_array]
    if n <= 6 {
        out.push(0xd8);
        out.push(0x79 + n);
    } else {
        out.push(0xd8);
        out.push(0x66);
        out.push(0x82); // 2-element array: [constructor_index, fields_array]
        out.extend_from_slice(&cbor_uint(n as u64));
    }
    out.extend_from_slice(&cbor_array_header(fields.len()));
    for f in fields {
        out.extend_from_slice(f);
    }
    out
}

fn cbor_array_header(len: usize) -> Vec<u8> {
    if len <= 23 { vec![0x80 | len as u8] }
    else if len <= 0xff { vec![0x98, len as u8] }
    else { vec![0x99, (len >> 8) as u8, len as u8] }
}

fn cbor_bytes(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let len = b.len();
    if len <= 23 {
        out.push(0x40 | len as u8);
    } else if len <= 0xff {
        out.push(0x58);
        out.push(len as u8);
    } else if len <= 0xffff {
        out.push(0x59);
        out.push((len >> 8) as u8);
        out.push(len as u8);
    } else {
        out.push(0x5a);
        out.extend_from_slice(&(len as u32).to_be_bytes());
    }
    out.extend_from_slice(b);
    out
}

fn cbor_uint(n: u64) -> Vec<u8> {
    if n <= 23 { vec![n as u8] }
    else if n <= 0xff { vec![0x18, n as u8] }
    else if n <= 0xffff { vec![0x19, (n >> 8) as u8, n as u8] }
    else if n <= 0xffff_ffff {
        vec![0x1a, (n >> 24) as u8, (n >> 16) as u8, (n >> 8) as u8, n as u8]
    } else {
        vec![0x1b,
            (n >> 56) as u8, (n >> 48) as u8, (n >> 40) as u8, (n >> 32) as u8,
            (n >> 24) as u8, (n >> 16) as u8, (n >> 8) as u8, n as u8]
    }
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

fn decode_cbor_uint(buf: &[u8]) -> Result<(u64, &[u8])> {
    if buf.is_empty() {
        return Err(anyhow!("unexpected end of CBOR data"));
    }
    match buf[0] {
        n @ 0x00..=0x17 => Ok((n as u64, &buf[1..])),
        0x18 if buf.len() >= 2 => Ok((buf[1] as u64, &buf[2..])),
        0x19 if buf.len() >= 3 => Ok((u16::from_be_bytes([buf[1], buf[2]]) as u64, &buf[3..])),
        0x1a if buf.len() >= 5 => Ok((u32::from_be_bytes([buf[1],buf[2],buf[3],buf[4]]) as u64, &buf[5..])),
        0x1b if buf.len() >= 9 => Ok((u64::from_be_bytes(buf[1..9].try_into().unwrap()), &buf[9..])),
        other => Err(anyhow!("unexpected CBOR byte 0x{other:02x}")),
    }
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

// ---------------------------------------------------------------------------
// CBOR param helpers (duplicated from bootstrap)
// ---------------------------------------------------------------------------

fn cbor_output_ref(tx_hash: &str, index: u64) -> Result<String> {
    let hash_bytes = hex::decode(tx_hash).context("decoding UTxO tx hash")?;
    if hash_bytes.len() != 32 { return Err(anyhow!("tx hash must be 32 bytes")); }
    let mut out = vec![0xd8, 0x79, 0x82, 0x58, 0x20];
    out.extend_from_slice(&hash_bytes);
    out.extend_from_slice(&cbor_uint(index));
    Ok(hex::encode(out))
}

fn cbor_policy_id(policy_id_hex: &str) -> Result<String> {
    let bytes = hex::decode(policy_id_hex).context("decoding policy ID hex")?;
    if bytes.len() != 28 { return Err(anyhow!("policy ID must be 28 bytes")); }
    let mut out = vec![0x58, 0x1c];
    out.extend_from_slice(&bytes);
    Ok(hex::encode(out))
}

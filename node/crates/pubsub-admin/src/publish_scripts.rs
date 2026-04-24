use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use pallas_addresses::Address;
use pallas_txbuilder::{BuildConway, Input, Output, ScriptKind, StagingTransaction};

use crate::{
    aiken,
    blockfrost::BlockfrostClient,
    bootstrap::{Network, split_utxo as parse_utxo_ref},
    tx::load_signing_key,
};

pub struct PublishScriptsArgs {
    pub network: Network,
    pub blockfrost_project_id: String,
    pub payment_addr: String,
    pub payment_skey_path: PathBuf,
    pub contracts_dir: PathBuf,
    pub env_file: PathBuf,
    /// UTxO to fund all four script-publication outputs (needs ~12 ADA).
    pub funding_utxo: String,
}

pub async fn run(args: PublishScriptsArgs) -> Result<()> {
    let bf = BlockfrostClient::new(&args.blockfrost_project_id, args.network.blockfrost_base_url());

    // --- read bootstrap UTxOs from the .env file ---------------------------
    let env = read_env(&args.env_file)?;
    let topic_utxo = env_var(&env, "PUBSUB_TOPIC_BOOTSTRAP_UTXO")?;
    let node_utxo  = env_var(&env, "PUBSUB_NODE_BOOTSTRAP_UTXO")?;

    // --- re-derive parameterized blueprints (same as bootstrap) ------------
    println!("Re-deriving parameterized blueprints...");
    let tmp = tempfile::tempdir().context("creating temp dir")?;

    let topic_project_dir = args.contracts_dir.join("topic-registry");
    let node_project_dir  = args.contracts_dir.join("node-registry");

    let (topic_tx_hash, topic_tx_ix) = parse_utxo_ref(&topic_utxo)?;
    let (node_tx_hash, node_tx_ix)   = parse_utxo_ref(&node_utxo)?;

    let topic_cbor = cbor_output_ref(&topic_tx_hash, topic_tx_ix)?;
    let node_cbor  = cbor_output_ref(&node_tx_hash, node_tx_ix)?;

    // topic-registry: apply utxo → get policy → apply policy to topic + publisher
    let topic_bp_raw = topic_project_dir.join("plutus.json");
    let topic_bp_1 = tmp.path().join("topic-bp-1.json");
    let topic_bp_2 = tmp.path().join("topic-bp-2.json");
    let topic_bp_3 = tmp.path().join("topic-bp-3.json");

    aiken::apply_param(&topic_bp_raw, &topic_bp_1, "registry",  "registry",  &topic_cbor)?;
    let registry_policy_id = aiken::policy_id(&topic_project_dir, &topic_bp_1, "registry", "registry")?;
    let policy_cbor = cbor_policy_id(&registry_policy_id)?;
    aiken::apply_param(&topic_bp_1, &topic_bp_2, "topic",     "topic",     &policy_cbor)?;
    aiken::apply_param(&topic_bp_2, &topic_bp_3, "publisher", "publisher", &policy_cbor)?;

    // node-registry
    let node_bp_raw = node_project_dir.join("plutus.json");
    let node_bp_1 = tmp.path().join("node-bp-1.json");
    aiken::apply_param(&node_bp_raw, &node_bp_1, "node_registry", "node_registry", &node_cbor)?;

    // --- collect scripts ---------------------------------------------------
    let scripts: &[(&str, &Path, &str)] = &[
        ("registry-mint",   &topic_bp_3, "registry.registry.mint"),
        ("topic-validator", &topic_bp_3, "topic.topic.spend"),
        ("publisher-vault", &topic_bp_3, "publisher.publisher.spend"),
        ("node-registry",   &node_bp_1,  "node_registry.node_registry.mint"),
    ];

    let mut script_bytes: Vec<(&str, Vec<u8>)> = Vec::new();
    for (label, blueprint, title) in scripts {
        let code = aiken::compiled_code(blueprint, title)
            .with_context(|| format!("extracting {label} script"))?;
        let bytes = hex::decode(&code)
            .with_context(|| format!("decoding {label} script hex"))?;
        println!("  {label}: {} bytes", bytes.len());
        script_bytes.push((label, bytes));
    }

    // --- fetch protocol params + funding UTxO ------------------------------
    println!("\nFetching protocol parameters...");
    let params = bf.protocol_params().await.context("fetching protocol params")?;

    let (fund_hash, fund_ix) = parse_utxo_ref(&args.funding_utxo)?;
    println!("Looking up funding UTxO {}...", args.funding_utxo);
    let fund_utxo = bf
        .find_utxo(&args.payment_addr, &fund_hash, fund_ix)
        .await
        .context("finding funding UTxO")?;
    println!("  value: {} lovelace", fund_utxo.lovelace());

    let signing_key = load_signing_key(&args.payment_skey_path).context("loading signing key")?;
    let payment_addr = Address::from_bech32(&args.payment_addr)
        .map_err(|e| anyhow!("invalid payment address: {e}"))?;

    let fund_hash_bytes: [u8; 32] = hex::decode(&fund_hash)
        .context("decoding funding UTxO hash")?
        .try_into()
        .map_err(|_| anyhow!("UTxO hash must be 32 bytes"))?;

    // --- build + submit one tx per script ----------------------------------
    // Each script goes in its own tx to keep tx size manageable.
    let mut refs: Vec<(&str, String)> = Vec::new();

    let mut remaining = fund_utxo.lovelace();
    let mut current_hash = fund_hash_bytes;
    let mut current_ix = fund_ix;

    for (label, bytes) in &script_bytes {
        let min_lovelace = params.min_ref_script_lovelace(bytes.len() as u64);
        println!("\nPublishing {label} ({} bytes, min-ADA {} lovelace)...", bytes.len(), min_lovelace);

        // Two-pass fee: first estimate, then exact.
        let tx = build_script_tx(
            current_hash,
            current_ix,
            remaining,
            &payment_addr,
            bytes,
            params.min_fee_a,
            params.min_fee_b,
            &signing_key,
            min_lovelace,
            500_000,
        )?;
        let actual_fee = params.min_fee_a * tx.len() as u64 + params.min_fee_b;
        let tx = build_script_tx(
            current_hash,
            current_ix,
            remaining,
            &payment_addr,
            bytes,
            params.min_fee_a,
            params.min_fee_b,
            &signing_key,
            min_lovelace,
            actual_fee,
        )?;

        let txid = bf.submit_tx(&tx).await
            .with_context(|| format!("submitting {label} script tx"))?;
        println!("  ✓ tx: {txid}");
        refs.push((label, format!("{txid}#0")));

        // The change output (#1) becomes the input for the next script tx.
        let change = remaining
            .checked_sub(min_lovelace + actual_fee)
            .ok_or_else(|| anyhow!("insufficient funds after publishing {label}"))?;
        remaining = change;
        current_hash = hex::decode(&txid)
            .context("decoding submitted txid")?
            .try_into()
            .map_err(|_| anyhow!("txid must be 32 bytes"))?;
        current_ix = 1; // change is output #1
    }

    // --- append script refs to .env file -----------------------------------
    println!("\nAppending script refs to {}...", args.env_file.display());
    append_script_refs(&args.env_file, &refs)?;

    println!("\n{}", "=".repeat(70));
    println!("publish-scripts complete");
    println!("{}", "=".repeat(70));
    for (label, utxo_ref) in &refs {
        println!("  {label}: {utxo_ref}");
    }
    println!("\nScript refs appended to {}", args.env_file.display());
    println!("{}", "=".repeat(70));

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_script_tx(
    utxo_hash: [u8; 32],
    utxo_ix: u64,
    input_lovelace: u64,
    payment_addr: &Address,
    script_bytes: &[u8],
    min_fee_a: u64,
    _min_fee_b: u64,
    signing_key: &pallas_crypto::key::ed25519::SecretKey,
    min_lovelace: u64,
    fee: u64,
) -> Result<Vec<u8>> {
    let _ = min_fee_a;

    let change = input_lovelace
        .checked_sub(min_lovelace + fee)
        .ok_or_else(|| {
            anyhow!(
                "insufficient funds: have {} lovelace, need {} ({} min-ADA output + {} fee)",
                input_lovelace, min_lovelace + fee, min_lovelace, fee
            )
        })?;

    let script_output = Output::new(payment_addr.clone(), min_lovelace)
        .set_inline_script(ScriptKind::PlutusV3, script_bytes.to_vec());
    let change_output = Output::new(payment_addr.clone(), change);

    let built = StagingTransaction::new()
        .input(Input::new(utxo_hash.into(), utxo_ix))
        .output(script_output)
        .output(change_output)
        .fee(fee)
        .build_conway_raw()
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
// .env helpers
// ---------------------------------------------------------------------------

fn read_env(path: &Path) -> Result<std::collections::HashMap<String, String>> {
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

fn append_script_refs(path: &Path, refs: &[(&str, String)]) -> Result<()> {
    let mut content = String::from("\n# Reference script UTxOs — written by publish-scripts\n");
    for (label, utxo_ref) in refs {
        let key = match *label {
            "registry-mint"   => "PUBSUB_REGISTRY_MINT_SCRIPT_REF",
            "topic-validator" => "PUBSUB_TOPIC_VALIDATOR_SCRIPT_REF",
            "publisher-vault" => "PUBSUB_PUBLISHER_VAULT_SCRIPT_REF",
            "node-registry"   => "PUBSUB_NODE_REGISTRY_SCRIPT_REF",
            other             => return Err(anyhow!("unknown script label: {other}")),
        };
        content.push_str(&format!("{key}={utxo_ref}\n"));
    }
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(path)
        .with_context(|| format!("opening {} for append", path.display()))?
        .write_all(content.as_bytes())
        .context("writing script refs")
}

// ---------------------------------------------------------------------------
// CBOR helpers (duplicated from bootstrap — TODO: extract to shared module)
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

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::{
    aiken,
    blockfrost::BlockfrostClient,
    tx::{build_bootstrap_tx, load_signing_key},
};

/// Everything needed to run the bootstrap command.
pub struct BootstrapArgs {
    pub network: Network,
    pub blockfrost_project_id: String,
    pub payment_skey_path: PathBuf,
    pub payment_addr: String,
    pub topic_bootstrap_utxo: String,  // "txhash#index"
    pub node_bootstrap_utxo: String,   // "txhash#index"
    pub min_deposit_lovelace: u64,
}

#[derive(Clone, Copy, Debug)]
pub enum Network {
    Preprod,
    Preview,
    Mainnet,
}

impl Network {
    pub fn blockfrost_base_url(self) -> &'static str {
        match self {
            Network::Preprod => "https://cardano-preprod.blockfrost.io/api/v0",
            Network::Preview => "https://cardano-preview.blockfrost.io/api/v0",
            Network::Mainnet => "https://cardano-mainnet.blockfrost.io/api/v0",
        }
    }

    pub fn is_mainnet(self) -> bool {
        matches!(self, Network::Mainnet)
    }

    pub fn env_name(self) -> &'static str {
        match self {
            Network::Preprod => "preprod",
            Network::Preview => "preview",
            Network::Mainnet => "mainnet",
        }
    }
}

pub async fn run(args: BootstrapArgs, contracts_dir: &Path, output_dir: &Path) -> Result<()> {
    let bf = BlockfrostClient::new(
        &args.blockfrost_project_id,
        args.network.blockfrost_base_url(),
    );

    println!("Fetching protocol parameters...");
    let params = bf.protocol_params().await.context("fetching protocol params")?;
    println!(
        "  min_fee_a={} min_fee_b={}",
        params.min_fee_a, params.min_fee_b
    );

    let signing_key = load_signing_key(&args.payment_skey_path)
        .context("loading signing key")?;

    // Temporary directory for parameterized blueprints.
    let tmp = tempfile::tempdir().context("creating temp dir")?;

    // ── Topic registry ──────────────────────────────────────────────────────
    println!("\nParameterizing topic-registry contracts...");

    let topic_project_dir = contracts_dir.join("topic-registry");
    let topic_bp_raw = topic_project_dir.join("plutus.json");
    let topic_bp_1 = tmp.path().join("topic-bp-1.json");
    let topic_bp_2 = tmp.path().join("topic-bp-2.json");
    let topic_bp_3 = tmp.path().join("topic-bp-3.json");

    let (topic_tx_hash, topic_tx_ix) = split_utxo(&args.topic_bootstrap_utxo)?;
    let topic_cbor = cbor_output_ref(&topic_tx_hash, topic_tx_ix)?;

    aiken::apply_param(&topic_bp_raw, &topic_bp_1, "registry", "registry", &topic_cbor)?;

    let registry_policy_id = aiken::policy_id(&topic_project_dir, &topic_bp_1, "registry", "registry")?;
    println!("  registry policy ID: {registry_policy_id}");

    let policy_cbor = cbor_policy_id(&registry_policy_id)?;

    aiken::apply_param(&topic_bp_1, &topic_bp_2, "topic", "topic", &policy_cbor)?;
    aiken::apply_param(&topic_bp_2, &topic_bp_3, "publisher", "publisher", &policy_cbor)?;

    let topic_registry_addr = aiken::address(&topic_project_dir, &topic_bp_3, "registry", "registry", args.network.is_mainnet())?;
    let topic_validator_addr = aiken::address(&topic_project_dir, &topic_bp_3, "topic", "topic", args.network.is_mainnet())?;
    let publisher_vault_addr = aiken::address(&topic_project_dir, &topic_bp_3, "publisher", "publisher", args.network.is_mainnet())?;

    println!("  topic-registry addr: {topic_registry_addr}");
    println!("  topic validator addr: {topic_validator_addr}");
    println!("  publisher vault addr: {publisher_vault_addr}");

    // Get the mint script bytes from the parameterized blueprint.
    let registry_mint_code = aiken::compiled_code(&topic_bp_3, "registry.registry.mint")?;
    let registry_mint_script =
        hex::decode(&registry_mint_code).context("decoding registry mint script hex")?;

    // Look up bootstrap UTxO value.
    println!("\nQuerying bootstrap UTxO for topic-registry ({})...", args.topic_bootstrap_utxo);
    let topic_utxo = bf
        .find_utxo(&args.payment_addr, &topic_tx_hash, topic_tx_ix)
        .await
        .context("finding topic-registry bootstrap UTxO")?;
    println!("  value: {} lovelace", topic_utxo.lovelace());

    // Datum: RegistryHeadDatum { counter: 0, epoch: 0 } = d87982 00 00
    let registry_head_datum = hex::decode("d879820000").unwrap();
    let registry_head_token_name: Vec<u8> = b"registry_head".to_vec();

    println!("Building and signing topic-registry bootstrap tx...");
    let topic_tx_cbor = build_bootstrap_tx(
        &topic_tx_hash,
        topic_tx_ix,
        topic_utxo.lovelace(),
        &topic_registry_addr,
        &registry_policy_id,
        &registry_head_token_name,
        &registry_head_datum,
        &registry_mint_script,
        &args.payment_addr,
        &params,
        &signing_key,
    )
    .context("building topic-registry bootstrap tx")?;

    println!("Submitting topic-registry bootstrap tx...");
    let topic_txid = bf.submit_tx(&topic_tx_cbor).await.context("submitting topic-registry tx")?;
    println!("  ✓ tx: {topic_txid}");

    // ── Node registry ───────────────────────────────────────────────────────
    println!("\nParameterizing node-registry contracts...");

    let node_project_dir = contracts_dir.join("node-registry");
    let node_bp_raw = node_project_dir.join("plutus.json");
    let node_bp_1 = tmp.path().join("node-bp-1.json");

    let (node_tx_hash, node_tx_ix) = split_utxo(&args.node_bootstrap_utxo)?;
    let node_cbor = cbor_output_ref(&node_tx_hash, node_tx_ix)?;

    aiken::apply_param(&node_bp_raw, &node_bp_1, "node_registry", "node_registry", &node_cbor)?;

    let node_registry_policy_id = aiken::policy_id(&node_project_dir, &node_bp_1, "node_registry", "node_registry")?;
    let node_registry_addr = aiken::address(&node_project_dir, &node_bp_1, "node_registry", "node_registry", args.network.is_mainnet())?;

    println!("  node-registry policy ID: {node_registry_policy_id}");
    println!("  node-registry addr:      {node_registry_addr}");

    let node_reg_mint_code = aiken::compiled_code(&node_bp_1, "node_registry.node_registry.mint")?;
    let node_reg_mint_script =
        hex::decode(&node_reg_mint_code).context("decoding node-registry mint script hex")?;

    println!("\nQuerying bootstrap UTxO for node-registry ({})...", args.node_bootstrap_utxo);
    let node_utxo = bf
        .find_utxo(&args.payment_addr, &node_tx_hash, node_tx_ix)
        .await
        .context("finding node-registry bootstrap UTxO")?;
    println!("  value: {} lovelace", node_utxo.lovelace());

    // NodeRegistryDatum { nodes: [], min_deposit_lovelace: N, epoch: 0 }
    let node_reg_datum = encode_node_registry_datum(args.min_deposit_lovelace);
    let node_registry_head_token_name: Vec<u8> = b"node_registry_head".to_vec();

    println!("Building and signing node-registry bootstrap tx...");
    let node_tx_cbor = build_bootstrap_tx(
        &node_tx_hash,
        node_tx_ix,
        node_utxo.lovelace(),
        &node_registry_addr,
        &node_registry_policy_id,
        &node_registry_head_token_name,
        &node_reg_datum,
        &node_reg_mint_script,
        &args.payment_addr,
        &params,
        &signing_key,
    )
    .context("building node-registry bootstrap tx")?;

    println!("Submitting node-registry bootstrap tx...");
    let node_txid = bf.submit_tx(&node_tx_cbor).await.context("submitting node-registry tx")?;
    println!("  ✓ tx: {node_txid}");

    // ── Write config file ───────────────────────────────────────────────────
    let env_file = output_dir.join(format!(".env.{}", args.network.env_name()));
    write_env_file(
        &env_file,
        args.network,
        &args.blockfrost_project_id,
        &topic_registry_addr,
        &topic_validator_addr,
        &publisher_vault_addr,
        &node_registry_addr,
        &registry_policy_id,
        &node_registry_policy_id,
        &args.topic_bootstrap_utxo,
        &args.node_bootstrap_utxo,
        &topic_txid,
        &node_txid,
    )?;

    println!("\n{}", "=".repeat(70));
    println!("Bootstrap complete — {}", args.network.env_name());
    println!("{}", "=".repeat(70));
    println!("  topic-registry tx:  {topic_txid}");
    println!("  node-registry  tx:  {node_txid}");
    println!();
    println!("  Config: {}", env_file.display());
    println!();
    println!("Next steps:");
    println!("  cp {} node/.env", env_file.display());
    println!("  cargo run -p pubsub-node --features cardano");
    println!("{}", "=".repeat(70));

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn split_utxo(utxo: &str) -> Result<(String, u64)> {
    let mut parts = utxo.splitn(2, '#');
    let hash = parts
        .next()
        .ok_or_else(|| anyhow!("invalid UTxO: {utxo}"))?
        .to_string();
    let index: u64 = parts
        .next()
        .ok_or_else(|| anyhow!("UTxO missing index: {utxo}"))?
        .parse()
        .map_err(|_| anyhow!("UTxO index is not a number: {utxo}"))?;
    Ok((hash, index))
}

/// CBOR-encode an OutputReference (Constr 0 [ByteArray(txhash), Int(index)]).
fn cbor_output_ref(tx_hash: &str, index: u64) -> Result<String> {
    let hash_bytes = hex::decode(tx_hash).context("decoding UTxO tx hash")?;
    if hash_bytes.len() != 32 {
        return Err(anyhow!("tx hash must be 32 bytes"));
    }
    // d879 = CBOR tag 121 (Constr 0), 82 = 2-element array,
    // 5820 = 32-byte bytestring, then the tx hash, then CBOR uint for index.
    let mut out = vec![0xd8, 0x79, 0x82, 0x58, 0x20];
    out.extend_from_slice(&hash_bytes);
    out.extend_from_slice(&cbor_uint(index));
    Ok(hex::encode(out))
}

/// CBOR-encode a PolicyId (28-byte raw bytestring).
fn cbor_policy_id(policy_id_hex: &str) -> Result<String> {
    let bytes = hex::decode(policy_id_hex).context("decoding policy ID hex")?;
    if bytes.len() != 28 {
        return Err(anyhow!("policy ID must be 28 bytes"));
    }
    // 581c = 0x58 0x1c = bytestring of length 28
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
        vec![
            0x1b,
            (n >> 56) as u8, (n >> 48) as u8, (n >> 40) as u8, (n >> 32) as u8,
            (n >> 24) as u8, (n >> 16) as u8, (n >> 8) as u8, n as u8,
        ]
    }
}

/// NodeRegistryDatum = Constr 0 [List([]), Int(min_deposit), Int(0)]
/// CBOR: d879 83 80 <uint(min_deposit)> 00
fn encode_node_registry_datum(min_deposit: u64) -> Vec<u8> {
    let mut out = vec![0xd8, 0x79, 0x83, 0x80]; // Constr 0 + 3-elem array + empty list
    out.extend_from_slice(&cbor_uint(min_deposit));
    out.push(0x00); // epoch = 0
    out
}

fn write_env_file(
    path: &Path,
    network: Network,
    blockfrost_project_id: &str,
    topic_registry_addr: &str,
    topic_validator_addr: &str,
    publisher_vault_addr: &str,
    node_registry_addr: &str,
    registry_policy_id: &str,
    node_registry_policy_id: &str,
    topic_utxo: &str,
    node_utxo: &str,
    topic_txid: &str,
    node_txid: &str,
) -> Result<()> {
    let content = format!(
        "# PubSub node Cardano config — generated by pubsub-admin bootstrap\n\
         # Network: {net}  (magic {magic})\n\
         # Date:    {date}\n\
         # topic-registry bootstrap: {topic_utxo}  → tx {topic_txid}\n\
         # node-registry  bootstrap: {node_utxo}   → tx {node_txid}\n\
         \n\
         BLOCKFROST_BASE_URL={base_url}\n\
         BLOCKFROST_PROJECT_ID={blockfrost_project_id}\n\
         \n\
         # Bech32 script addresses (enterprise, no staking credential)\n\
         PUBSUB_TOPIC_REGISTRY_ADDR={topic_registry_addr}\n\
         PUBSUB_TOPIC_VALIDATOR_ADDR={topic_validator_addr}\n\
         PUBSUB_PUBLISHER_VAULT_ADDR={publisher_vault_addr}\n\
         PUBSUB_NODE_REGISTRY_ADDR={node_registry_addr}\n\
         \n\
         # Minting policy IDs (56 hex chars = 28 bytes)\n\
         PUBSUB_REGISTRY_POLICY_ID={registry_policy_id}\n\
         PUBSUB_NODE_REGISTRY_POLICY_ID={node_registry_policy_id}\n\
         \n\
         # Optional: Demeter utxorpc endpoint\n\
         DEMETER_API_KEY=\n",
        net = network.env_name(),
        magic = match network {
            Network::Preprod => 1,
            Network::Preview => 2,
            Network::Mainnet => 764_824_073,
        },
        date = chrono_now(),
        base_url = network.blockfrost_base_url(),
    );

    std::fs::write(path, content)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

fn chrono_now() -> String {
    // No chrono dep — use std instead.
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("unix:{}", d.as_secs()))
        .unwrap_or_else(|_| "unknown".into())
}

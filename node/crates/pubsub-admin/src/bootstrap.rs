use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::{
    aiken,
    blockfrost::BlockfrostClient,
    cbor::{cbor_output_ref, cbor_policy_id},
    tx::{build_bootstrap_tx, load_signing_key},
};

pub struct BootstrapArgs {
    pub network: Network,
    pub blockfrost_project_id: String,
    pub payment_skey_path: PathBuf,
    pub payment_addr: String,
    pub bootstrap_utxo: String,  // "txhash#index" — one-shot param for topic-registry
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

    let tmp = tempfile::tempdir().context("creating temp dir")?;

    // ── Topic registry ──────────────────────────────────────────────────────
    println!("\nParameterizing topic-registry contracts...");

    let topic_project_dir = contracts_dir.join("topic-registry");
    let topic_bp_raw = topic_project_dir.join("plutus.json");
    let topic_bp_1 = tmp.path().join("topic-bp-1.json");
    let topic_bp_2 = tmp.path().join("topic-bp-2.json");
    let topic_bp_3 = tmp.path().join("topic-bp-3.json");

    let (tx_hash, tx_ix) = parse_utxo_ref(&args.bootstrap_utxo)?;
    let bootstrap_cbor = cbor_output_ref(&tx_hash, tx_ix)?;

    aiken::apply_param(&topic_bp_raw, &topic_bp_1, "registry", "registry", &bootstrap_cbor)?;

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

    let registry_mint_code = aiken::compiled_code(&topic_bp_3, "registry.registry.mint")?;
    let registry_mint_script =
        hex::decode(&registry_mint_code).context("decoding registry mint script hex")?;

    println!("\nQuerying bootstrap UTxO ({})...", args.bootstrap_utxo);
    let utxo = bf
        .find_utxo(&args.payment_addr, &tx_hash, tx_ix)
        .await
        .context("finding bootstrap UTxO")?;
    println!("  value: {} lovelace", utxo.lovelace());

    // RegistryHeadDatum { counter: 0, epoch: 0 } = d87982 00 00
    let registry_head_datum = hex::decode("d879820000").unwrap();
    let registry_head_token_name: Vec<u8> = b"registry_head".to_vec();

    println!("Building and signing topic-registry bootstrap tx...");
    let tx_cbor = build_bootstrap_tx(
        &tx_hash,
        tx_ix,
        utxo.lovelace(),
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
    let txid = bf.submit_tx(&tx_cbor).await.context("submitting bootstrap tx")?;
    println!("  ✓ tx: {txid}");

    // ── Write TOML config file ──────────────────────────────────────────────
    let config_file = output_dir.join(format!("config.{}.toml", args.network.env_name()));
    write_toml_config(
        &config_file,
        args.network,
        &args.blockfrost_project_id,
        &topic_registry_addr,
        &topic_validator_addr,
        &publisher_vault_addr,
        &registry_policy_id,
        &args.bootstrap_utxo,
        &txid,
    )?;

    println!("\n{}", "=".repeat(70));
    println!("Bootstrap complete — {}", args.network.env_name());
    println!("{}", "=".repeat(70));
    println!("  topic-registry tx: {txid}");
    println!();
    println!("  Config: {}", config_file.display());
    println!();
    println!("Next steps:");
    println!("  pubsub-admin publish-scripts --config {}", config_file.display());
    println!("{}", "=".repeat(70));

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse "txhash#index" into its components.
pub fn parse_utxo_ref(utxo: &str) -> Result<(String, u64)> {
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

fn write_toml_config(
    path: &Path,
    network: Network,
    blockfrost_project_id: &str,
    topic_registry_addr: &str,
    topic_validator_addr: &str,
    publisher_vault_addr: &str,
    registry_policy_id: &str,
    bootstrap_utxo: &str,
    txid: &str,
) -> Result<()> {
    let content = format!(
        "# PubSub node Cardano config — generated by pubsub-admin bootstrap\n\
         # Network: {net}  (magic {magic})\n\
         # Date:    {date}\n\
         # topic-registry bootstrap: {bootstrap_utxo}  → tx {txid}\n\
         \n\
         network = \"{net}\"\n\
         \n\
         blockfrost_url = \"{base_url}\"\n\
         blockfrost_key = \"{blockfrost_project_id}\"\n\
         \n\
         # Bech32 script addresses (enterprise, no staking credential)\n\
         topic_validator_addr = \"{topic_validator_addr}\"\n\
         publisher_vault_addr = \"{publisher_vault_addr}\"\n\
         registry_policy_id = \"{registry_policy_id}\"\n\
         \n\
         # Reference script UTxOs — set by pubsub-admin publish-scripts\n\
         # registry_mint_script_ref = \"\"\n\
         # topic_validator_script_ref = \"\"\n\
         # publisher_vault_script_ref = \"\"\n\
         \n\
         # Admin-internal — used by publish-scripts to re-derive parameterized blueprints\n\
         # _topic_registry_addr = \"{topic_registry_addr}\"\n\
         # _bootstrap_utxo = \"{bootstrap_utxo}\"\n",
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
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("unix:{}", d.as_secs()))
        .unwrap_or_else(|_| "unknown".into())
}

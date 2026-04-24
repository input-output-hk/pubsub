mod aiken;
mod blockfrost;
mod bootstrap;
mod tx;

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use dialoguer::{Input, Select};

use bootstrap::{BootstrapArgs, Network};

#[derive(Parser)]
#[command(name = "pubsub-admin", about = "PubSub Cardano contract deployment and management")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Bootstrap topic-registry and node-registry contracts on-chain.
    Bootstrap {
        /// Cardano network (preprod / preview / mainnet).
        /// Prompted interactively if omitted.
        #[arg(long)]
        network: Option<String>,

        /// Blockfrost project ID. Falls back to $BLOCKFROST_PROJECT_ID.
        #[arg(long)]
        blockfrost_project_id: Option<String>,

        /// Path to the payment signing key JSON file (cardano-cli format).
        #[arg(long)]
        payment_skey: Option<PathBuf>,

        /// Bech32 payment address corresponding to the signing key.
        #[arg(long)]
        payment_addr: Option<String>,

        /// UTxO to consume for topic-registry bootstrap ("txhash#index").
        #[arg(long)]
        topic_utxo: Option<String>,

        /// UTxO to consume for node-registry bootstrap ("txhash#index").
        #[arg(long)]
        node_utxo: Option<String>,

        /// Minimum deposit in lovelace for node registration.
        #[arg(long, default_value = "10000000")]
        min_deposit_lovelace: u64,

        /// Directory containing compiled contract blueprints.
        #[arg(long, default_value = "../contracts")]
        contracts_dir: PathBuf,

        /// Directory to write the generated .env file.
        #[arg(long, default_value = ".")]
        output_dir: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Bootstrap {
            network,
            blockfrost_project_id,
            payment_skey,
            payment_addr,
            topic_utxo,
            node_utxo,
            min_deposit_lovelace,
            contracts_dir,
            output_dir,
        } => {
            let network = resolve_network(network)?;
            let blockfrost_project_id = resolve_blockfrost_id(blockfrost_project_id)?;
            let payment_skey_path = resolve_skey_path(payment_skey)?;
            let payment_addr = resolve_payment_addr(payment_addr)?;
            let topic_bootstrap_utxo = resolve_utxo(topic_utxo, "topic-registry bootstrap UTxO")?;
            let node_bootstrap_utxo = resolve_utxo(node_utxo, "node-registry bootstrap UTxO")?;

            let args = BootstrapArgs {
                network,
                blockfrost_project_id,
                payment_skey_path,
                payment_addr,
                topic_bootstrap_utxo,
                node_bootstrap_utxo,
                min_deposit_lovelace,
            };

            bootstrap::run(args, &contracts_dir, &output_dir).await?;
        }
    }

    Ok(())
}

fn resolve_network(flag: Option<String>) -> Result<Network> {
    if let Some(s) = flag {
        return parse_network(&s);
    }

    let choices = &["preprod", "preview", "mainnet"];
    let idx = Select::new()
        .with_prompt("Network")
        .items(choices)
        .default(0)
        .interact()
        .map_err(|e| anyhow!("network selection failed: {e}"))?;

    parse_network(choices[idx])
}

fn parse_network(s: &str) -> Result<Network> {
    match s.to_lowercase().as_str() {
        "preprod" => Ok(Network::Preprod),
        "preview" => Ok(Network::Preview),
        "mainnet" => Ok(Network::Mainnet),
        _ => Err(anyhow!("unknown network '{s}' — expected preprod, preview, or mainnet")),
    }
}

fn resolve_blockfrost_id(flag: Option<String>) -> Result<String> {
    if let Some(id) = flag {
        return Ok(id);
    }
    if let Ok(id) = std::env::var("BLOCKFROST_PROJECT_ID") {
        if !id.is_empty() {
            return Ok(id);
        }
    }
    Input::new()
        .with_prompt("Blockfrost project ID")
        .validate_with(|s: &String| {
            if s.is_empty() { Err("project ID cannot be empty") } else { Ok(()) }
        })
        .interact_text()
        .map_err(|e| anyhow!("blockfrost project ID input failed: {e}"))
}

fn resolve_skey_path(flag: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = flag {
        if !p.exists() {
            return Err(anyhow!("signing key file not found: {}", p.display()));
        }
        return Ok(p);
    }
    let s: String = Input::new()
        .with_prompt("Payment signing key path (.skey)")
        .validate_with(|s: &String| {
            if std::path::Path::new(s.trim()).exists() { Ok(()) }
            else { Err("file not found") }
        })
        .interact_text()
        .map_err(|e| anyhow!("skey path input failed: {e}"))?;
    Ok(PathBuf::from(s.trim()))
}

fn resolve_payment_addr(flag: Option<String>) -> Result<String> {
    if let Some(a) = flag {
        return validate_addr(a);
    }
    let s: String = Input::new()
        .with_prompt("Payment address (bech32, addr1... or addr_test1...)")
        .validate_with(|s: &String| {
            if s.starts_with("addr") { Ok(()) } else { Err("must start with 'addr'") }
        })
        .interact_text()
        .map_err(|e| anyhow!("payment address input failed: {e}"))?;
    validate_addr(s)
}

fn validate_addr(s: String) -> Result<String> {
    if s.starts_with("addr") {
        Ok(s)
    } else {
        Err(anyhow!("invalid payment address '{s}' — expected bech32 starting with 'addr'"))
    }
}

fn resolve_utxo(flag: Option<String>, label: &str) -> Result<String> {
    if let Some(u) = flag {
        return validate_utxo(u, label);
    }
    let prompt = format!("{label} (<64-hex-txhash>#<index>)");
    let s: String = Input::new()
        .with_prompt(prompt)
        .validate_with(|s: &String| validate_utxo_str(s))
        .interact_text()
        .map_err(|e| anyhow!("UTxO input failed: {e}"))?;
    Ok(s)
}

fn validate_utxo(s: String, label: &str) -> Result<String> {
    validate_utxo_str(&s).map_err(|e| anyhow!("invalid {label} '{s}': {e}"))?;
    Ok(s)
}

fn validate_utxo_str(s: &str) -> Result<(), &'static str> {
    let mut parts = s.splitn(2, '#');
    let hash = parts.next().unwrap_or("");
    let index = parts.next().unwrap_or("");
    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("tx hash must be 64 hex characters");
    }
    if index.parse::<u64>().is_err() {
        return Err("index must be a non-negative integer");
    }
    Ok(())
}

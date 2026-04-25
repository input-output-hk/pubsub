mod aiken;
mod blockfrost;
mod bootstrap;
mod create_topic;
mod publish_scripts;
mod tx;

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use dialoguer::Select;

use bootstrap::{BootstrapArgs, Network};
use create_topic::CreateTopicArgs;
use publish_scripts::PublishScriptsArgs;

#[derive(Parser)]
#[command(name = "pubsub-admin", about = "PubSub Cardano contract deployment and management")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Bootstrap the topic-registry contract on-chain (one-time per network).
    /// Consumes a UTxO as a one-shot parameter to derive a unique minting policy,
    /// then mints the registry-head NFT and writes a .env file with all addresses.
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

        /// UTxO to consume as the one-shot bootstrap parameter ("txhash#index").
        /// Any UTxO at your payment address works; ~5 ADA is sufficient.
        #[arg(long)]
        utxo: Option<String>,

        /// Directory containing compiled contract blueprints.
        #[arg(long, default_value = "../contracts")]
        contracts_dir: PathBuf,

        /// Directory to write the generated .env file.
        #[arg(long, default_value = ".")]
        output_dir: PathBuf,
    },

    /// Publish the three Plutus scripts as on-chain reference script UTxOs (CIP-33).
    /// Must be run after `bootstrap`. Reads the bootstrap UTxO ref from the .env file
    /// to re-derive parameterized blueprints, then publishes each script in its own tx.
    PublishScripts {
        /// Path to the .env file written by `bootstrap` (e.g. local/.env.preprod).
        #[arg(long)]
        env_file: Option<PathBuf>,

        /// Cardano network.
        #[arg(long)]
        network: Option<String>,

        /// Blockfrost project ID. Falls back to $BLOCKFROST_PROJECT_ID.
        #[arg(long)]
        blockfrost_project_id: Option<String>,

        /// Bech32 payment address that owns the funding UTxO.
        #[arg(long)]
        payment_addr: Option<String>,

        /// Path to the payment signing key JSON file.
        #[arg(long)]
        payment_skey: Option<PathBuf>,

        /// Directory containing compiled contract blueprints.
        #[arg(long, default_value = "../contracts")]
        contracts_dir: PathBuf,

        /// UTxO to fund the three script-publication outputs (~40 ADA needed).
        #[arg(long)]
        funding_utxo: Option<String>,
    },

    /// Register a new topic on-chain via the topic-registry contract.
    /// Must be run after `publish-scripts`. Reads the registry head UTxO,
    /// mints a topic token, and writes a TopicDatum to the topic validator address.
    CreateTopic {
        /// Path to the .env file written by `bootstrap` (e.g. local/.env.preprod).
        #[arg(long)]
        env_file: Option<PathBuf>,

        /// Cardano network.
        #[arg(long)]
        network: Option<String>,

        /// Blockfrost project ID. Falls back to $BLOCKFROST_PROJECT_ID.
        #[arg(long)]
        blockfrost_project_id: Option<String>,

        /// Bech32 payment address that will become the topic owner.
        #[arg(long)]
        payment_addr: Option<String>,

        /// Path to the payment signing key JSON file.
        #[arg(long)]
        payment_skey: Option<PathBuf>,

        /// Directory containing compiled contract blueprints.
        #[arg(long, default_value = "../contracts")]
        contracts_dir: PathBuf,

        /// UTxO to fund the transaction (~5 ADA sufficient).
        #[arg(long)]
        funding_utxo: Option<String>,

        /// Human-readable topic name (e.g. "iog/spo/alerts").
        #[arg(long)]
        name: String,

        /// Number of relay nodes that must cache each message (must be > 0).
        #[arg(long, default_value = "1")]
        replication_factor: u64,

        /// Message retention window in seconds (must be > 0).
        #[arg(long, default_value = "86400")]
        retention_period: u64,
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
            utxo,
            contracts_dir,
            output_dir,
        } => {
            let network = resolve_network(network)?;
            let blockfrost_project_id = resolve_blockfrost_id(blockfrost_project_id)?;
            let payment_skey_path = resolve_skey_path(payment_skey)?;
            let payment_addr = resolve_payment_addr(payment_addr)?;
            let bootstrap_utxo = resolve_utxo(utxo, "bootstrap UTxO")?;

            bootstrap::run(
                BootstrapArgs {
                    network,
                    blockfrost_project_id,
                    payment_skey_path,
                    payment_addr,
                    bootstrap_utxo,
                },
                &contracts_dir,
                &output_dir,
            )
            .await?;
        }

        Command::PublishScripts {
            env_file,
            network,
            blockfrost_project_id,
            payment_addr,
            payment_skey,
            contracts_dir,
            funding_utxo,
        } => {
            let network = resolve_network(network)?;
            let blockfrost_project_id = resolve_blockfrost_id(blockfrost_project_id)?;
            let payment_addr = resolve_payment_addr(payment_addr)?;
            let payment_skey_path = resolve_skey_path(payment_skey)?;
            let funding_utxo = resolve_utxo(funding_utxo, "funding UTxO")?;
            let env_file = resolve_env_file(env_file, &network)?;

            publish_scripts::run(PublishScriptsArgs {
                network,
                blockfrost_project_id,
                payment_addr,
                payment_skey_path,
                contracts_dir,
                env_file,
                funding_utxo,
            })
            .await?;
        }

        Command::CreateTopic {
            env_file,
            network,
            blockfrost_project_id,
            payment_addr,
            payment_skey,
            contracts_dir,
            funding_utxo,
            name,
            replication_factor,
            retention_period,
        } => {
            if replication_factor == 0 {
                return Err(anyhow!("--replication-factor must be > 0"));
            }
            if retention_period == 0 {
                return Err(anyhow!("--retention-period must be > 0"));
            }
            let network = resolve_network(network)?;
            let blockfrost_project_id = resolve_blockfrost_id(blockfrost_project_id)?;
            let payment_addr = resolve_payment_addr(payment_addr)?;
            let payment_skey_path = resolve_skey_path(payment_skey)?;
            let env_file = resolve_env_file(env_file, &network)?;
            let funding_utxo = resolve_utxo(funding_utxo, "funding UTxO")?;

            create_topic::run(CreateTopicArgs {
                network,
                blockfrost_project_id,
                payment_addr,
                payment_skey_path,
                contracts_dir,
                env_file,
                funding_utxo,
                name,
                replication_factor,
                retention_period,
            })
            .await?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Network (arrow-key Select — raw mode is fine here, no paste involved)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Text prompts — plain readline (no raw mode, paste-safe)
// ---------------------------------------------------------------------------

fn readline(prompt: &str) -> Result<String> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    write!(out, "{prompt}: ")?;
    out.flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

fn prompt_validated(prompt: &str, validate: impl Fn(&str) -> Result<(), &'static str>) -> Result<String> {
    loop {
        let val = readline(prompt)?;
        match validate(&val) {
            Ok(()) => return Ok(val),
            Err(msg) => eprintln!("  ✗ {msg}"),
        }
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
    prompt_validated("Blockfrost project ID", |s| {
        if s.is_empty() { Err("cannot be empty") } else { Ok(()) }
    })
}

fn resolve_skey_path(flag: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = flag {
        if !p.exists() {
            return Err(anyhow!("signing key file not found: {}", p.display()));
        }
        return Ok(p);
    }
    let s = prompt_validated("Payment signing key path (.skey)", |s| {
        if std::path::Path::new(s).exists() { Ok(()) } else { Err("file not found") }
    })?;
    Ok(PathBuf::from(s))
}

fn resolve_payment_addr(flag: Option<String>) -> Result<String> {
    if let Some(a) = flag {
        return load_addr(a);
    }
    let s = prompt_validated("Payment address or .addr file path", |s| {
        let resolved = if std::path::Path::new(s).exists() {
            std::fs::read_to_string(s).unwrap_or_default()
        } else {
            s.to_string()
        };
        if resolved.trim().starts_with("addr") { Ok(()) } else { Err("must be a bech32 addr or a path to a .addr file") }
    })?;
    load_addr(s)
}

fn load_addr(s: String) -> Result<String> {
    let path = std::path::Path::new(&s);
    let looks_like_path = s.contains('/') || s.contains('\\') || path.extension().is_some();

    let addr = if looks_like_path {
        std::fs::read_to_string(path)
            .with_context(|| format!("reading address file '{}' (cwd: {})", s, std::env::current_dir().unwrap_or_default().display()))?
            .trim()
            .to_string()
    } else {
        s
    };

    if addr.starts_with("addr") {
        Ok(addr)
    } else {
        Err(anyhow!("invalid payment address '{addr}' — expected bech32 starting with 'addr'"))
    }
}

fn resolve_utxo(flag: Option<String>, label: &str) -> Result<String> {
    if let Some(u) = flag {
        return validate_utxo(u, label);
    }
    let s = prompt_validated(
        &format!("{label} (<64-hex-txhash>#<index>)"),
        validate_utxo_str,
    )?;
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

fn resolve_env_file(flag: Option<PathBuf>, network: &Network) -> Result<PathBuf> {
    if let Some(p) = flag {
        if !p.exists() {
            return Err(anyhow!(".env file not found: {}", p.display()));
        }
        return Ok(p);
    }
    let default = PathBuf::from(format!("local/.env.{}", network.env_name()));
    if default.exists() {
        println!("Using .env file: {}", default.display());
        return Ok(default);
    }
    let s = prompt_validated(".env file path (written by bootstrap)", |s| {
        if std::path::Path::new(s).exists() { Ok(()) } else { Err("file not found") }
    })?;
    Ok(PathBuf::from(s))
}

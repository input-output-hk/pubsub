mod aiken;
mod blockfrost;
mod bootstrap;
mod cbor;
mod create_topic;
mod publish_scripts;
mod resolve;
mod tx;

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};

use bootstrap::BootstrapArgs;
use create_topic::CreateTopicArgs;
use publish_scripts::PublishScriptsArgs;
use resolve::{
    resolve_blockfrost_id, resolve_env_file, resolve_network, resolve_payment_addr,
    resolve_skey_path, resolve_utxo,
};

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

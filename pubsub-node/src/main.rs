use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use pubsub_node::{load_peer_list, InMemoryNetwork, Node, PeerId};

/// Minimal Cardano pub/sub node: registers on a shared (single-process)
/// in-memory network, loads its peer set from TOML, and waits for Ctrl-C.
#[derive(Parser)]
#[command(name = "pubsub-node", version, about, long_about = None)]
struct Args {
    /// This node's identifier (non-empty UTF-8, no internal NUL bytes).
    #[arg(long)]
    self_id: PeerId,

    /// Path to the TOML peer-list configuration file.
    #[arg(long)]
    config: PathBuf,

    /// Logging verbosity threshold (trace | debug | info | warn | error).
    #[arg(long, default_value = "info")]
    log_level: tracing::Level,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_max_level(args.log_level)
        .with_writer(std::io::stderr)
        .init();

    let cfg = load_peer_list(&args.config).unwrap_or_else(|e| {
        eprintln!("pubsub-node: {e}");
        std::process::exit(2);
    });

    let network = Arc::new(InMemoryNetwork::new());
    let node = Node::new(args.self_id, cfg, network)
        .await
        .unwrap_or_else(|e| {
            eprintln!("pubsub-node: {e}");
            std::process::exit(1);
        });

    if let Err(e) = tokio::signal::ctrl_c().await {
        eprintln!("pubsub-node: failed to install signal handler: {e}");
        std::process::exit(1);
    }

    drop(node);
}

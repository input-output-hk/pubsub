use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use pubsub_node::{
    load_node_config, AcceptFromAllCandidates, ConnectToAllCandidates, ConnectionStrategy,
    ConnectionStrategyKind, ForwardToAll, InMemoryNetwork, InMemorySubscriptionRegistry,
    InMemoryTopicRegistry, MockCryptoScheme, Node, PeerId, SeededBoundedSelection, Signer,
    TestVerifier, Verifier,
};

/// Minimal Cardano pub/sub node: registers on a shared (single-process)
/// in-memory network, loads its peer set from TOML, and waits for Ctrl-C.
#[derive(Parser)]
#[command(name = "pubsub-node", version, about, long_about = None)]
struct Args {
    /// This node's identifier (non-empty UTF-8, no internal NUL bytes).
    #[arg(long)]
    self_id: PeerId,

    /// Path to the TOML node-config file.
    #[arg(long)]
    config: PathBuf,

    /// Path to the TOML subscription-list file (the mock subscription registry
    /// the node reads its topics and peer membership from).
    #[arg(long)]
    subscription_list: PathBuf,

    /// Path to the TOML topic-registry file (the mock topic registry: which
    /// topics legitimately exist and their authorized publishers).
    #[arg(long)]
    topic_registry: PathBuf,

    /// Connection-selection strategy (case-insensitive): `connect-to-all` (full
    /// mesh, the default) or `seeded-bounded` (selects at most --out-degree
    /// upstream peers per topic, seeded by --seed).
    #[arg(long, default_value = "connect-to-all")]
    connection_strategy: ConnectionStrategyKind,

    /// Per-topic upstream bound (out-degree). Required for the `seeded-bounded`
    /// strategy; ignored otherwise.
    #[arg(long)]
    out_degree: Option<usize>,

    /// Network seed for deterministic bounded selection (default 0). Only has an
    /// effect for the `seeded-bounded` strategy; the same seed reproduces the
    /// same topology.
    #[arg(long, default_value_t = 0)]
    seed: u64,

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

    let cfg = load_node_config(&args.config).unwrap_or_else(|e| {
        eprintln!("pubsub-node: {e}");
        std::process::exit(2);
    });

    // The mock subscription registry, seeded from the subscription-list file
    // (the stand-in for the on-chain subscription list / operator registration).
    let registry = Arc::new(
        InMemorySubscriptionRegistry::from_file(&args.subscription_list).unwrap_or_else(|e| {
            eprintln!("pubsub-node: {e}");
            std::process::exit(2);
        }),
    );

    // The mock topic registry, seeded from the topic-registry file (the stand-in
    // for the on-chain topic registry: legitimate topics + authorized publishers).
    let topic_registry = Arc::new(
        InMemoryTopicRegistry::from_file(&args.topic_registry).unwrap_or_else(|e| {
            eprintln!("pubsub-node: {e}");
            std::process::exit(2);
        }),
    );

    let network = Arc::new(InMemoryNetwork::new());
    // Prototype-stage verifier: the mock accepts any correctly-bound mock
    // signature. A real verifier replaces this when authenticated crypto lands.
    let verifier: Arc<dyn Verifier> = Arc::new(TestVerifier);
    // Prototype-stage signing identity: the mock keypair for the node's alias
    // (the alias round-trips through `PeerId`'s display form), so it is coherent
    // with `self_id` by construction. Real key material replaces this at 011.
    let scheme = MockCryptoScheme::with_seed([0u8; 32]);
    let signer: Arc<dyn Signer> =
        Arc::new(scheme.signer(scheme.keypair_from_alias(&args.self_id.to_string()).private));

    // Map the named strategy + its parameters to a concrete policy (feature 005,
    // FR-012/FR-013). The full-mesh default is unchanged behaviour.
    let connection_strategy: Arc<dyn ConnectionStrategy> = match args.connection_strategy {
        ConnectionStrategyKind::ConnectToAll => Arc::new(ConnectToAllCandidates),
        ConnectionStrategyKind::SeededBounded => {
            let out_degree = args.out_degree.unwrap_or_else(|| {
                eprintln!(
                    "pubsub-node: --out-degree is required for --connection-strategy seeded-bounded"
                );
                std::process::exit(2);
            });
            Arc::new(SeededBoundedSelection::new(
                args.seed,
                args.self_id.clone(),
                out_degree,
            ))
        }
    };

    let node = Node::new(
        args.self_id,
        cfg,
        network,
        signer,
        verifier,
        registry,
        topic_registry,
        connection_strategy,
        Arc::new(ForwardToAll),
        Arc::new(AcceptFromAllCandidates),
    )
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

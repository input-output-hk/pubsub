use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use pubsub_node::{
    load_node_config, AcceptanceStrategyKind, ConnectionStrategyKind, ForwardToAll,
    InMemoryNetwork, InMemorySubscriptionRegistry, InMemoryTopicRegistry, MockCryptoScheme, Node,
    PeerId, Signer, StrategyParams, TestVerifier, Verifier,
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
    /// mesh, the default) or `seeded-bounded` (selects at most --upstream-degree
    /// upstream peers per topic, seeded by --seed).
    #[arg(long, default_value = "connect-to-all")]
    connection_strategy: ConnectionStrategyKind,

    /// Max upstream peers selected (dialed) per topic. Required for the
    /// `seeded-bounded` connection strategy; ignored otherwise.
    #[arg(long)]
    upstream_degree: Option<usize>,

    /// Network seed for deterministic bounded selection (default 0). Only has an
    /// effect for the `seeded-bounded` strategy; the same seed reproduces the
    /// same topology.
    #[arg(long, default_value_t = 0)]
    seed: u64,

    /// Inbound-acceptance strategy (case-insensitive): `accept-from-all` (the
    /// default) or `bounded` (accepts at most --downstream-degree downstream peers per
    /// topic, refusing the rest with an explicit rejection).
    #[arg(long, default_value = "accept-from-all")]
    acceptance_strategy: AcceptanceStrategyKind,

    /// Max downstream peers accepted per topic (inbound connections this node
    /// admits). Required for the `bounded` acceptance strategy; ignored otherwise.
    #[arg(long)]
    downstream_degree: Option<usize>,

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

    // Each strategy kind builds itself from the parsed params, validating the
    // parameters it requires; the edge stays lean and maps a StrategyConfigError
    // once (ADR 0028). The full-mesh / accept-from-all defaults are unchanged.
    let params = StrategyParams {
        self_id: args.self_id.clone(),
        seed: args.seed,
        upstream_degree: args.upstream_degree,
        downstream_degree: args.downstream_degree,
    };
    let connection_strategy = args.connection_strategy.build(&params).unwrap_or_else(|e| {
        eprintln!("pubsub-node: {e}");
        std::process::exit(2);
    });
    let acceptance_strategy = args.acceptance_strategy.build(&params).unwrap_or_else(|e| {
        eprintln!("pubsub-node: {e}");
        std::process::exit(2);
    });

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
        acceptance_strategy,
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

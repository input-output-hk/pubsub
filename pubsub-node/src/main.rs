use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use pubsub_node::{
    load_node_config, AcceptanceParams, AcceptanceStrategyKind, ConnectionParams,
    ConnectionStrategyKind, ForwardToAll, InMemoryNetwork, InMemorySubscriptionRegistry,
    InMemoryTopicRegistry, MockCryptoScheme, Node, NodeStrategies, PeerId, Signer, TestVerifier,
    Verifier,
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
    /// mesh, the default) or `hash-gated` (verifiable bucketed selection to ~--target-degree
    /// upstreams per topic, gated by the edge predicate over --genesis).
    #[arg(long, default_value = "connect-to-all")]
    connection_strategy: ConnectionStrategyKind,

    /// The fixed target connection degree `target_degree` — the target expected upstream degree per topic. Required
    /// for the `hash-gated` / `verifiable-bounded` strategies; ignored otherwise.
    /// The per-topic bucket count derives from it; small topics connect to all.
    #[arg(long)]
    target_degree: Option<usize>,

    /// Public genesis nonce folded into the verifiable edge predicate (default 0).
    /// Both peers use it; the same genesis reproduces the same topology.
    #[arg(long, default_value_t = 0)]
    genesis: u64,

    /// Inbound-acceptance strategy (case-insensitive): `accept-from-all` (the
    /// default) or `verifiable-bounded` (verifies the edge predicate + caps
    /// downstream at `⌈target_degree + c·√target_degree⌉` per topic, refusing over-capacity with `Rejected`).
    #[arg(long, default_value = "accept-from-all")]
    acceptance_strategy: AcceptanceStrategyKind,

    /// Accept-cap buffer `c` in `OC = ⌈target_degree + c·√target_degree⌉` (default 3). Only affects the
    /// `verifiable-bounded` acceptance strategy.
    #[arg(long, default_value_t = 3)]
    cap_buffer: usize,

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

    // Two-phase strategy construction (ADR 0028): phase 1 captures the resolved
    // strategy keys (clap already applied the seam defaults and rejected unknown
    // keys); phase 2 binds each seam's own params and builds them all, validating
    // the parameters each chosen strategy requires. The edge stays lean — it maps
    // a single StrategyConfigError. The full-mesh / accept-from-all defaults are
    // unchanged; fan-out stays `ForwardToAll`, injected separately below.
    let strategies = NodeStrategies::builder(args.connection_strategy, args.acceptance_strategy)
        .build(
            &ConnectionParams {
                self_id: args.self_id.clone(),
                genesis: args.genesis,
                target_degree: args.target_degree,
            },
            &AcceptanceParams {
                self_id: args.self_id.clone(),
                genesis: args.genesis,
                target_degree: args.target_degree,
                cap_buffer: args.cap_buffer,
            },
        )
        .unwrap_or_else(|e| {
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
        strategies.connection,
        Arc::new(ForwardToAll),
        strategies.acceptance,
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

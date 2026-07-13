use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use pubsub_node::{
    load_node_config, AcceptanceParams, AcceptanceStrategyKind, FanoutStrategyKind,
    InMemoryNetwork, InMemorySubscriptionRegistry, InMemoryTopicRegistry, LinkRole,
    LinkSelectionKind, MockCryptoScheme, Node, NodeStrategies, PeerId, SelectionParams, Signer,
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

    /// Relay link-selection strategy (case-insensitive): `connect-to-all` (full
    /// mesh, the default), `hash-gated` (verifiable bucketed selection to
    /// ~--relay-degree upstreams per topic, gated by the edge predicate over
    /// --genesis), or `none` (dial nobody — an accept-only node).
    #[arg(long, default_value = "connect-to-all")]
    connection_strategy: LinkSelectionKind,

    /// The fixed relay connection degree `relay_degree` — the target expected upstream (relay) degree per topic. Required
    /// for every strategy except `connect-to-all` / `accept-from-all`; ignored by those.
    /// The per-topic bucket count derives from it; with a derived bucket count,
    /// small topics connect to all (see --bucket-count for the pinned case).
    #[arg(long)]
    relay_degree: Option<usize>,

    /// Public genesis nonce (default 0): the node's initial **epoch nonce**, the
    /// randomness context the verifiable edge predicate hashes (the epoch-0
    /// stand-in for the chain-anchored beacon; an `Epoch` event replaces it).
    /// Both peers use it; the same genesis reproduces the same topology.
    #[arg(long, default_value_t = 0)]
    genesis: u64,

    /// Optional pinned bucket count `B` for the edge predicate. When unset, `B`
    /// is derived per topic from `--relay-degree`. When set, both peers use this
    /// exact value on both seams, so verification holds by construction (no
    /// dependence on the two ends having folded the same candidate set); a natural
    /// experiment axis. Applies to the hash-gated strategies; must be ≥ 1.
    /// Caution: pinning replaces the derived value INCLUDING the small-topic
    /// B=1 connect-to-all floor — a pinned B larger than a topic's candidate
    /// count can leave a node with zero upstreams on that topic (no retry).
    #[arg(long)]
    bucket_count: Option<usize>,

    /// Inbound-acceptance strategy (case-insensitive), the four one-dimensional
    /// baselines: `accept-from-all` (the default; membership only), `bounded`
    /// (caps downstream at `⌈relay_degree + c·√relay_degree⌉` per topic, refusing
    /// over-capacity with `Rejected`), `hash-gated` (verifies the edge predicate,
    /// no cap), or `hash-gated-bounded` (predicate + cap — the bucketed-pull compound).
    #[arg(long, default_value = "accept-from-all")]
    acceptance_strategy: AcceptanceStrategyKind,

    /// Accept-cap buffer `c` in `OC = ⌈relay_degree + c·√relay_degree⌉` (default 3). Only affects the
    /// `bounded` / `hash-gated-bounded` acceptance strategies (both slots).
    #[arg(long, default_value_t = 3)]
    cap_buffer: usize,

    /// Publish link-selection strategy (case-insensitive): `none` (the default
    /// — no standing initiation links), `hash-gated` (verifiable bucketed
    /// selection of ~--publish-degree initiation targets per topic, always
    /// established), or `connect-to-all`.
    #[arg(long, default_value = "none")]
    publish_strategy: LinkSelectionKind,

    /// The publish degree — the target standing initiation-link out-degree per
    /// topic (the M3 model's s−1), independent of --relay-degree. Required by
    /// the `hash-gated` publish strategy and by every publish-acceptance
    /// strategy except `accept-from-all`.
    #[arg(long)]
    publish_degree: Option<usize>,

    /// Fan-out strategy (case-insensitive) — the dissemination-model knob:
    /// `forward-to-all` (the default: relay downstream for every message, plus
    /// the initiation targets for the node's own publications) or
    /// `role-scoped` (the strict M3 partition: own publications over
    /// initiation links ONLY, relayed traffic over relay links only).
    #[arg(long, default_value = "forward-to-all")]
    fanout_strategy: FanoutStrategyKind,

    /// Inbound acceptance strategy for publish-intent requests
    /// (case-insensitive), the same four baselines as --acceptance-strategy,
    /// instantiated with --publish-degree and counted against inbound
    /// publishing links only (default: accept-from-all).
    #[arg(long, default_value = "accept-from-all")]
    publish_acceptance_strategy: AcceptanceStrategyKind,

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
    let strategies = NodeStrategies::builder(
        args.connection_strategy,
        args.acceptance_strategy,
        args.publish_strategy,
        args.publish_acceptance_strategy,
        args.fanout_strategy,
    )
    .build(
        &SelectionParams {
            self_id: args.self_id.clone(),
            role: LinkRole::Relay,
            degree: args.relay_degree,
            bucket_count: args.bucket_count,
        },
        &AcceptanceParams {
            self_id: args.self_id.clone(),
            role: LinkRole::Relay,
            degree: args.relay_degree,
            bucket_count: args.bucket_count,
            cap_buffer: args.cap_buffer,
        },
        &SelectionParams {
            self_id: args.self_id.clone(),
            role: LinkRole::Publisher,
            degree: args.publish_degree,
            bucket_count: args.bucket_count,
        },
        &AcceptanceParams {
            self_id: args.self_id.clone(),
            role: LinkRole::Publisher,
            degree: args.publish_degree,
            bucket_count: args.bucket_count,
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
        args.genesis,
        network,
        signer,
        verifier,
        registry,
        topic_registry,
        strategies.relay_selection,
        strategies.fanout,
        strategies.relay_acceptance,
        strategies.publish_selection,
        strategies.publish_acceptance,
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

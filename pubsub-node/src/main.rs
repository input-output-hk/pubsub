use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use pubsub_node::{
    load_node_config, AcceptanceParams, AcceptanceStrategyKind, ConnectionParams,
    ConnectionStrategyKind, FanoutStrategyKind, InMemoryNetwork, InMemorySubscriptionRegistry,
    InMemoryTopicRegistry, LinkKind, MockCryptoScheme, Node, NodeStrategies, PeerId,
    PublisherAdmission, Signer, TestVerifier, Verifier,
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

    /// Relay-link selection strategy (case-insensitive): `connect-to-all` (full
    /// mesh, the default) or `hash-gated` (verifiable bucketed selection to
    /// ~--relay-degree upstreams per topic, gated by the edge predicate over
    /// --genesis).
    #[arg(long, default_value = "connect-to-all")]
    relay_strategy: ConnectionStrategyKind,

    /// The fixed relay target degree — the target expected relay upstream degree
    /// per topic. Required for every relay strategy except `connect-to-all` /
    /// `accept-from-all`; ignored by those. The per-topic bucket count derives
    /// from it; with a derived bucket count, small topics connect to all (see
    /// --bucket-count for the pinned case).
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

    /// Relay-link acceptance strategy (case-insensitive), the four
    /// one-dimensional baselines: `accept-from-all` (the default; membership
    /// only), `bounded` (caps accepted relay downstreams per topic, refusing
    /// over-capacity with `Rejected`), `hash-gated` (verifies the edge
    /// predicate, no cap), or `hash-gated-bounded` (predicate + cap).
    #[arg(long, default_value = "accept-from-all")]
    relay_acceptance_strategy: AcceptanceStrategyKind,

    /// Publisher-link selection strategy (case-insensitive): `connect-to-all`
    /// or `hash-gated`. When absent (the default) the node never dials
    /// publisher links — the pre-publisher-links baseline.
    #[arg(long)]
    publisher_strategy: Option<ConnectionStrategyKind>,

    /// Publisher-link acceptance strategy (case-insensitive), same four kinds
    /// as --relay-acceptance-strategy. When absent (the default) inbound
    /// publisher requests are silently dropped.
    #[arg(long)]
    publisher_acceptance_strategy: Option<AcceptanceStrategyKind>,

    /// The fixed publisher target degree — the target expected number of
    /// standing publisher links per topic. Required by the publisher
    /// `hash-gated` / bounded kinds.
    #[arg(long)]
    publisher_degree: Option<usize>,

    /// Draw relay edges with the symmetric (unordered-pair) predicate: both
    /// ends of a valid edge dial each other and every link forms as a
    /// reciprocal pair. Applies to the relay selection AND acceptance
    /// hash-gated strategies together (the two sides must agree). The
    /// bidirectional model uses no publisher links (a publisher's own
    /// symmetric links carry its message out); publisher strategies, if
    /// configured anyway, are unaffected by this flag.
    #[arg(long)]
    symmetric_edges: bool,

    /// Accept-cap buffer `c` in the per-topic accept cap (default 3). Only
    /// affects the `bounded` / `hash-gated-bounded` acceptance strategies.
    #[arg(long, default_value_t = 3)]
    cap_buffer: usize,

    /// Fan-out strategy (case-insensitive): `forward-to-relays` (the default —
    /// held messages are forwarded to relay downstream only; publisher links
    /// carry just the node's own publications) or `forward-to-all` (every held
    /// message over both link classes).
    #[arg(long, default_value = "forward-to-relays")]
    fanout_strategy: FanoutStrategyKind,

    /// Receive-gate policy for inbound publisher links (case-insensitive):
    /// `owner-only` (the default — a publisher link admits only its owner's
    /// own publications) or `any-verified` (admits any verified message).
    /// Pair `any-verified` with `--fanout-strategy forward-to-all` network-wide.
    #[arg(long, default_value = "owner-only")]
    publisher_admission: PublisherAdmission,

    /// Logging verbosity threshold (trace | debug | info | warn | error).
    #[arg(long, default_value = "info")]
    log_level: tracing::Level,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    validate_flag_combinations(&args);

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
    // unchanged; fan-out stays `ForwardToRelays`, injected separately below.
    let strategies = NodeStrategies::builder(args.relay_strategy, args.relay_acceptance_strategy)
        .build(
            &ConnectionParams {
                self_id: args.self_id.clone(),
                kind: LinkKind::Relay,
                target_degree: args.relay_degree,
                bucket_count: args.bucket_count,
                symmetric: args.symmetric_edges,
            },
            &AcceptanceParams {
                self_id: args.self_id.clone(),
                kind: LinkKind::Relay,
                target_degree: args.relay_degree,
                bucket_count: args.bucket_count,
                cap_buffer: args.cap_buffer,
                symmetric: args.symmetric_edges,
            },
        )
        .unwrap_or_else(|e| {
            eprintln!("pubsub-node: {e}");
            std::process::exit(2);
        });

    // The optional publisher pair: second instances of the same seams, drawn
    // from the publisher hash domain with their own degree.
    let strategies = with_publisher_pair(strategies, &args);

    let node = Node::new(
        args.self_id,
        cfg,
        args.genesis,
        network,
        signer,
        verifier,
        registry,
        topic_registry,
        strategies,
        args.fanout_strategy.build(),
        args.publisher_admission,
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

/// Reject flag combinations that would silently do nothing — a
/// mis-parameterised experiment run must fail at startup, not produce
/// quietly-wrong topology data.
fn validate_flag_combinations(args: &Args) {
    let die = |msg: &str| {
        eprintln!("pubsub-node: {msg}");
        std::process::exit(2);
    };
    if args.publisher_degree.is_some()
        && args.publisher_strategy.is_none()
        && args.publisher_acceptance_strategy.is_none()
    {
        die(
            "--publisher-degree has no effect without --publisher-strategy or \
             --publisher-acceptance-strategy",
        );
    }
    let relay_selection_gated = args.relay_strategy == ConnectionStrategyKind::HashGated;
    let relay_acceptance_gated = matches!(
        args.relay_acceptance_strategy,
        AcceptanceStrategyKind::HashGated | AcceptanceStrategyKind::HashGatedBounded
    );
    if args.symmetric_edges && !relay_selection_gated && !relay_acceptance_gated {
        die(
            "--symmetric-edges has no effect: it requires a hash-gated relay strategy \
             (--relay-strategy hash-gated and/or a hash-gated --relay-acceptance-strategy)",
        );
    }
    if args.publisher_admission == PublisherAdmission::AnyVerified
        && args.publisher_acceptance_strategy.is_none()
    {
        die("--publisher-admission any-verified has no effect without \
             --publisher-acceptance-strategy (the node accepts no inbound publisher links)");
    }
}

/// Build the optional publisher selection/acceptance instances from the
/// publisher flags — second instances of the relay seams under the publisher
/// hash domain, with their own degree. Absent flags leave the pair `None`
/// (publisher links disabled).
fn with_publisher_pair(mut strategies: NodeStrategies, args: &Args) -> NodeStrategies {
    if let Some(kind) = args.publisher_strategy {
        strategies.publisher_connection = Some(
            kind.build(&ConnectionParams {
                self_id: args.self_id.clone(),
                kind: LinkKind::Publisher,
                target_degree: args.publisher_degree,
                bucket_count: args.bucket_count,
                symmetric: false,
            })
            .unwrap_or_else(|e| {
                eprintln!("pubsub-node: {e}");
                std::process::exit(2);
            }),
        );
    }
    if let Some(kind) = args.publisher_acceptance_strategy {
        strategies.publisher_acceptance = Some(
            kind.build(&AcceptanceParams {
                self_id: args.self_id.clone(),
                kind: LinkKind::Publisher,
                target_degree: args.publisher_degree,
                bucket_count: args.bucket_count,
                cap_buffer: args.cap_buffer,
                symmetric: false,
            })
            .unwrap_or_else(|e| {
                eprintln!("pubsub-node: {e}");
                std::process::exit(2);
            }),
        );
    }
    strategies
}

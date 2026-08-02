use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use pubsub_node::{
    selection_seed_bytes, AcceptanceParams, FanoutStrategyKind, InMemoryNetwork,
    InMemorySubscriptionRegistry, InMemoryTopicRegistry, LinkKind, MockCryptoScheme, Node,
    NodeStrategies, PeerId, SelectionParams, Signer, TestVerifier, Verifier,
};

/// Minimal Cardano pub/sub node: registers on a shared (single-process)
/// in-memory network, derives its peers from the subscription registry, and
/// waits for Ctrl-C.
// 017-FR-006: knob-only, per-seam, presence-activated selection surface; the
// strategy kind and degree flags are deleted with no aliases (pre-release).
#[derive(Parser)]
#[command(name = "pubsub-node", version, about, long_about = None)]
struct Args {
    /// This node's identifier (non-empty UTF-8, no internal NUL bytes).
    #[arg(long)]
    self_id: PeerId,

    /// Path to the TOML subscription-list file (the mock subscription registry
    /// the node reads its topics and peer membership from).
    #[arg(long)]
    subscription_list: PathBuf,

    /// Path to the TOML topic-registry file (the mock topic registry: which
    /// topics legitimately exist and their authorized publishers).
    #[arg(long)]
    topic_registry: PathBuf,

    /// Public genesis nonce (default 0): the node's initial **epoch nonce**, the
    /// randomness context the verifiable edge predicate hashes (the epoch-0
    /// stand-in for the chain-anchored beacon; an `Epoch` event replaces it).
    /// Both peers use it; the same genesis reproduces the same topology.
    #[arg(long, default_value_t = 0)]
    genesis: u64,

    /// Relay hash-gate bucket count B (at least 2). Present: relay dialing
    /// keeps only candidates passing the verifiable edge predicate at B, and
    /// relay acceptors verify inbound requests at the same B (one value, both
    /// sides). Absent: relay selection is ungated and acceptors do not verify.
    /// Caution: a B larger than a topic's candidate count can leave zero
    /// upstreams on that topic (no retry).
    #[arg(long)]
    relay_bucket_count: Option<usize>,

    /// Exact number of relay upstreams to select per topic: min(pick count,
    /// gate survivors) seeded uniform picks without replacement. Absent: dial
    /// every gate survivor. 0: dial no relay links (inbound acceptance still
    /// serves).
    #[arg(long)]
    relay_pick_count: Option<usize>,

    /// Absolute cap on accepted relay downstreams per topic; an over-capacity
    /// request is refused with an explicit rejection (the dialer abandons that
    /// edge). Absent: unbounded. 0: serve no relay downstreams.
    #[arg(long)]
    relay_accept_cap: Option<usize>,

    /// Establish relay links with the symmetric (bidirectional) handshake:
    /// edges are drawn for the unordered pair and one accept decision records
    /// each link in both directions on both ends. Composes with any relay
    /// knob combination; publisher links, if configured, stay directional.
    #[arg(long)]
    relay_symmetric: bool,

    /// Skip acceptor-side predicate verification on the relay seam (trusting
    /// acceptors): inbound relay requests are admitted without recomputing the
    /// edge predicate. Requires --relay-bucket-count (otherwise there is no
    /// gate to skip).
    #[arg(long)]
    relay_accept_unverified: bool,

    /// Publisher-seam mirror of --relay-bucket-count (the publisher hash
    /// domain). Any publisher flag activates the publisher seam; activation
    /// requires a publisher dial knob (--publisher-pick-count or
    /// --publisher-bucket-count).
    #[arg(long)]
    publisher_bucket_count: Option<usize>,

    /// Publisher-seam mirror of --relay-pick-count: the exact number of
    /// standing publisher links to select per topic. 0: dial no publisher
    /// links (an accept-only publisher seam).
    #[arg(long)]
    publisher_pick_count: Option<usize>,

    /// Publisher-seam mirror of --relay-accept-cap: the absolute cap on
    /// accepted inbound publisher links per topic.
    #[arg(long)]
    publisher_accept_cap: Option<usize>,

    /// Publisher-seam mirror of --relay-accept-unverified. Requires
    /// --publisher-bucket-count.
    #[arg(long)]
    publisher_accept_unverified: bool,

    /// Sampling seed for the seeded uniform picks. Required when any seam has
    /// a pick count of 1 or more; rejected as unused otherwise. The same
    /// configuration and seed reproduce the same topology; anyone who knows
    /// the seed can recompute the picks.
    #[arg(long)]
    selection_seed: Option<u64>,

    /// Fan-out strategy (case-insensitive): `forward-to-all` (the default —
    /// every held message over both link classes) or `forward-to-relays`
    /// (held messages to relay downstreams only; publisher links carry just
    /// the node's own publications). Caution: a node with publisher links
    /// and this flag omitted runs M5 semantics (relayed traffic rides its
    /// publisher links too); M3 exclusivity requires the explicit
    /// `forward-to-relays`.
    // 017-FR-009: the default flip's footgun stated in help.
    #[arg(long, default_value = "forward-to-all")]
    fanout_strategy: FanoutStrategyKind,

    /// Logging verbosity threshold (trace | debug | info | warn | error).
    #[arg(long, default_value = "info")]
    log_level: tracing::Level,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if let Err(e) = validate_flag_combinations(&args) {
        eprintln!("pubsub-node: {e}");
        std::process::exit(2);
    }

    tracing_subscriber::fmt()
        .with_max_level(args.log_level)
        .with_writer(std::io::stderr)
        .init();

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

    // Selection-plane construction: per-seam knobs map straight into the param
    // structs; the acceptance-side bucket count is the post-opt-out gate value
    // (None when --*-accept-unverified is set), so "verification follows the
    // seam's bucket count, with an explicit opt-out" is resolved right here at
    // the edge. The publisher pair goes through the same fallible call.
    // 017-FR-011 (opt-out at construction), 017-FR-008 (presence activation).
    let seed_bytes = args.selection_seed.map_or([0u8; 32], selection_seed_bytes);
    let publisher_active = args.publisher_bucket_count.is_some()
        || args.publisher_pick_count.is_some()
        || args.publisher_accept_cap.is_some()
        || args.publisher_accept_unverified;
    let strategies = NodeStrategies::new(
        SelectionParams {
            self_id: args.self_id.clone(),
            kind: LinkKind::Relay,
            symmetric: args.relay_symmetric,
            bucket_count: args.relay_bucket_count,
            pick_count: args.relay_pick_count,
            seed: seed_bytes,
        },
        AcceptanceParams {
            self_id: args.self_id.clone(),
            kind: LinkKind::Relay,
            symmetric: args.relay_symmetric,
            bucket_count: if args.relay_accept_unverified {
                None
            } else {
                args.relay_bucket_count
            },
            accept_cap: args.relay_accept_cap,
        },
        publisher_active.then(|| {
            (
                SelectionParams {
                    self_id: args.self_id.clone(),
                    kind: LinkKind::Publisher,
                    symmetric: false,
                    bucket_count: args.publisher_bucket_count,
                    pick_count: args.publisher_pick_count,
                    seed: seed_bytes,
                },
                AcceptanceParams {
                    self_id: args.self_id.clone(),
                    kind: LinkKind::Publisher,
                    symmetric: false,
                    bucket_count: if args.publisher_accept_unverified {
                        None
                    } else {
                        args.publisher_bucket_count
                    },
                    accept_cap: args.publisher_accept_cap,
                },
            )
        }),
    )
    .unwrap_or_else(|e| {
        eprintln!("pubsub-node: {e}");
        std::process::exit(2);
    });

    let node = Node::new(
        args.self_id,
        args.genesis,
        network,
        signer,
        verifier,
        registry,
        topic_registry,
        strategies,
        args.fanout_strategy.build(),
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

/// A rejected flag combination. Messages are operator-facing and actionable;
/// startup maps any of these to exit code 2.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
enum FlagError {
    /// A bucket count below the gating threshold.
    #[error("{flag} must be at least 2: gating is signalled by providing the flag, and a one-bucket gate admits everyone (omit the flag for ungated selection)")]
    BucketCountTooSmall {
        /// The offending flag.
        flag: &'static str,
    },
    /// A sampling seam was configured without a seed.
    #[error("--selection-seed is required when any seam has a pick count of 1 or more (seeded uniform picks need a sampling seed)")]
    MissingSelectionSeed,
    /// A seed was supplied but nothing samples.
    #[error("--selection-seed has no effect without a sampling seam (a pick count of 1 or more); remove the flag")]
    UnusedSelectionSeed,
    /// Publisher acceptance knobs alone cannot activate the publisher seam.
    #[error("publisher acceptance flags alone do not activate the publisher seam: add a publisher dial knob (--publisher-pick-count, 0 for an accept-only seam, or --publisher-bucket-count)")]
    PublisherSeamWithoutDialKnob,
    /// A verification opt-out with no gate to skip.
    #[error("{flag} has no effect without {bucket_flag}: there is no gate to skip")]
    UnusedAcceptUnverified {
        /// The opt-out flag.
        flag: &'static str,
        /// The seam's bucket-count flag that would give it effect.
        bucket_flag: &'static str,
    },
}

/// Reject flag combinations that would silently do nothing or silently mean
/// something else — a mis-parameterised experiment run must fail at startup,
/// not produce quietly-wrong topology data. Pure so the matrix is unit-tested
/// on values; `main` maps an `Err` to exit code 2.
// 017-FR-007 (knob domains + unconsumed flags), 017-FR-008 (publisher
// activation needs a dial knob), 017-FR-014 (seed required iff sampling);
// spec Clarifications 2026-07-31. The pre-017 symmetric-requires-hash-gated
// rule is gone: symmetric composes with every plane point.
fn validate_flag_combinations(args: &Args) -> Result<(), FlagError> {
    if matches!(args.relay_bucket_count, Some(b) if b < 2) {
        return Err(FlagError::BucketCountTooSmall {
            flag: "--relay-bucket-count",
        });
    }
    if matches!(args.publisher_bucket_count, Some(b) if b < 2) {
        return Err(FlagError::BucketCountTooSmall {
            flag: "--publisher-bucket-count",
        });
    }
    let publisher_dial_knob =
        args.publisher_pick_count.is_some() || args.publisher_bucket_count.is_some();
    let publisher_acceptance_knob =
        args.publisher_accept_cap.is_some() || args.publisher_accept_unverified;
    if publisher_acceptance_knob && !publisher_dial_knob {
        return Err(FlagError::PublisherSeamWithoutDialKnob);
    }
    let sampling = args.relay_pick_count.is_some_and(|k| k >= 1)
        || args.publisher_pick_count.is_some_and(|k| k >= 1);
    if sampling && args.selection_seed.is_none() {
        return Err(FlagError::MissingSelectionSeed);
    }
    if !sampling && args.selection_seed.is_some() {
        return Err(FlagError::UnusedSelectionSeed);
    }
    if args.relay_accept_unverified && args.relay_bucket_count.is_none() {
        return Err(FlagError::UnusedAcceptUnverified {
            flag: "--relay-accept-unverified",
            bucket_flag: "--relay-bucket-count",
        });
    }
    if args.publisher_accept_unverified && args.publisher_bucket_count.is_none() {
        return Err(FlagError::UnusedAcceptUnverified {
            flag: "--publisher-accept-unverified",
            bucket_flag: "--publisher-bucket-count",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::str::FromStr;

    use super::{validate_flag_combinations, Args, FlagError};
    use pubsub_node::{FanoutStrategyKind, PeerId};

    /// A valid no-knob baseline (the pre-017 default behaviour: ungated,
    /// uncapped, dial-all relay seam; publisher seam off).
    fn base_args() -> Args {
        Args {
            self_id: PeerId::from_str("self").expect("valid peer id"),
            subscription_list: PathBuf::from("subs.toml"),
            topic_registry: PathBuf::from("topics.toml"),
            genesis: 0,
            relay_bucket_count: None,
            relay_pick_count: None,
            relay_accept_cap: None,
            relay_symmetric: false,
            relay_accept_unverified: false,
            publisher_bucket_count: None,
            publisher_pick_count: None,
            publisher_accept_cap: None,
            publisher_accept_unverified: false,
            selection_seed: None,
            fanout_strategy: FanoutStrategyKind::ForwardToAll,
            log_level: tracing::Level::INFO,
        }
    }

    // 017-FR-007: the no-knob default and each single-knob point validate.
    #[test]
    fn plane_points_validate() {
        assert_eq!(validate_flag_combinations(&base_args()), Ok(()));

        let mut gated = base_args();
        gated.relay_bucket_count = Some(2);
        assert_eq!(validate_flag_combinations(&gated), Ok(()));

        let mut picks = base_args();
        picks.relay_pick_count = Some(8);
        picks.selection_seed = Some(7);
        assert_eq!(validate_flag_combinations(&picks), Ok(()));

        let mut capped = base_args();
        capped.relay_accept_cap = Some(17);
        assert_eq!(validate_flag_combinations(&capped), Ok(()));

        // The M1 boundary: pick count 0 samples nothing, so no seed is needed.
        let mut push_only = base_args();
        push_only.relay_pick_count = Some(0);
        push_only.publisher_pick_count = Some(4);
        push_only.selection_seed = Some(7);
        assert_eq!(validate_flag_combinations(&push_only), Ok(()));
    }

    // 017-FR-007: bucket counts 0 and 1 are rejected on either seam — gating
    // is signalled by the flag's presence and a one-bucket gate is vacuous.
    #[test]
    fn bucket_counts_below_two_are_rejected() {
        for value in [0, 1] {
            let mut relay = base_args();
            relay.relay_bucket_count = Some(value);
            assert_eq!(
                validate_flag_combinations(&relay),
                Err(FlagError::BucketCountTooSmall {
                    flag: "--relay-bucket-count"
                }),
            );

            let mut publisher = base_args();
            publisher.publisher_bucket_count = Some(value);
            assert_eq!(
                validate_flag_combinations(&publisher),
                Err(FlagError::BucketCountTooSmall {
                    flag: "--publisher-bucket-count"
                }),
            );
        }
    }

    // 017-FR-014: the seed is required iff a seam samples, and rejected as
    // unused otherwise (pick count 0 does not sample).
    #[test]
    fn seed_is_required_iff_sampling() {
        let mut sampling = base_args();
        sampling.relay_pick_count = Some(8);
        assert_eq!(
            validate_flag_combinations(&sampling),
            Err(FlagError::MissingSelectionSeed),
        );

        let mut publisher_sampling = base_args();
        publisher_sampling.publisher_pick_count = Some(4);
        assert_eq!(
            validate_flag_combinations(&publisher_sampling),
            Err(FlagError::MissingSelectionSeed),
        );

        let mut unused = base_args();
        unused.selection_seed = Some(7);
        assert_eq!(
            validate_flag_combinations(&unused),
            Err(FlagError::UnusedSelectionSeed),
        );

        let mut zero_picks = base_args();
        zero_picks.relay_pick_count = Some(0);
        zero_picks.selection_seed = Some(7);
        assert_eq!(
            validate_flag_combinations(&zero_picks),
            Err(FlagError::UnusedSelectionSeed),
        );
    }

    // 017-FR-008 / Clarifications 2026-07-31: acceptance knobs alone do not
    // activate the publisher seam — the error names the accept-only spelling.
    #[test]
    fn publisher_seam_needs_a_dial_knob() {
        let mut cap_only = base_args();
        cap_only.publisher_accept_cap = Some(4);
        assert_eq!(
            validate_flag_combinations(&cap_only),
            Err(FlagError::PublisherSeamWithoutDialKnob),
        );

        let mut unverified_only = base_args();
        unverified_only.publisher_accept_unverified = true;
        assert_eq!(
            validate_flag_combinations(&unverified_only),
            Err(FlagError::PublisherSeamWithoutDialKnob),
        );

        // The accept-only spelling: pick count 0 is a dial knob.
        let mut accept_only = base_args();
        accept_only.publisher_pick_count = Some(0);
        accept_only.publisher_accept_cap = Some(4);
        assert_eq!(validate_flag_combinations(&accept_only), Ok(()));
    }

    // 017-FR-011: the verification opt-out without the seam's bucket count is
    // rejected as unused (the gate is already vacuous).
    #[test]
    fn accept_unverified_requires_the_seams_bucket_count() {
        let mut relay = base_args();
        relay.relay_accept_unverified = true;
        assert_eq!(
            validate_flag_combinations(&relay),
            Err(FlagError::UnusedAcceptUnverified {
                flag: "--relay-accept-unverified",
                bucket_flag: "--relay-bucket-count",
            }),
        );

        let mut relay_gated = base_args();
        relay_gated.relay_accept_unverified = true;
        relay_gated.relay_bucket_count = Some(2);
        assert_eq!(validate_flag_combinations(&relay_gated), Ok(()));

        // Publisher mirror: the opt-out needs the PUBLISHER bucket count even
        // when the seam is otherwise active.
        let mut publisher = base_args();
        publisher.publisher_pick_count = Some(4);
        publisher.publisher_accept_unverified = true;
        publisher.selection_seed = Some(7);
        assert_eq!(
            validate_flag_combinations(&publisher),
            Err(FlagError::UnusedAcceptUnverified {
                flag: "--publisher-accept-unverified",
                bucket_flag: "--publisher-bucket-count",
            }),
        );
    }

    // The pre-017 symmetric-requires-hash-gated rule is deleted: symmetric
    // composes with every plane point (uniform + symmetric is the real M4).
    #[test]
    fn symmetric_composes_with_every_plane_point() {
        let mut uniform_symmetric = base_args();
        uniform_symmetric.relay_pick_count = Some(8);
        uniform_symmetric.relay_symmetric = true;
        uniform_symmetric.selection_seed = Some(7);
        assert_eq!(validate_flag_combinations(&uniform_symmetric), Ok(()));

        let mut bare_symmetric = base_args();
        bare_symmetric.relay_symmetric = true;
        assert_eq!(validate_flag_combinations(&bare_symmetric), Ok(()));
    }
}

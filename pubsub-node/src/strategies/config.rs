//! Two-phase strategy construction (ADR 0028).
//!
//! **Phase 1 — key → builder.** The edge parses each seam's strategy *key* into
//! its `*StrategyKind` (clap: absent → the seam default, unknown key → rejected
//! at CLI parse). [`NodeStrategies::builder`] holds the resolved kinds; nothing
//! is constructed yet.
//!
//! **Phase 2 — params → strategy.** [`NodeStrategiesBuilder::build`] takes one
//! per-seam params struct each ([`ConnectionParams`], [`AcceptanceParams`]) —
//! already-typed values, no `clap` in the core — and constructs every seam,
//! validating the parameters each chosen strategy requires. A required param
//! left `None` yields a [`StrategyConfigError`]; the edge maps it **once**.
//!
//! Each kind reads only the params for its own seam (no shared grab-bag), so
//! construction *and* required-parameter validation live with the strategy, not
//! scattered across the edge. (Fan-out stays `ForwardToRelays`, injected separately;
//! it is not built through this two-phase seam.)
//!
//! **Unified plane construction (017).** [`NodeStrategies::new`] builds the
//! whole set from per-seam plane knobs ([`SelectionParams`] /
//! [`UnifiedAcceptanceParams`]) in one fallible call — with no strategy kinds
//! to resolve, the key-resolution phase has nothing to do there, and the
//! publisher pair is built through the same call instead of bypassing
//! construction. The two-phase builder above coexists until the strategy
//! kinds are deleted (017-T016).

use std::sync::Arc;

use crate::connection_state::LinkKind;
use crate::peer::PeerId;
use crate::strategies::acceptance::{
    AcceptanceStrategyKind, ConnectionAcceptanceStrategy, UnifiedAcceptance,
};
use crate::strategies::connection::{ConnectionStrategy, ConnectionStrategyKind, Selection};

/// Already-parsed parameters for the connection (dial/upstream) seam. A field a
/// chosen kind requires but that is left `None` yields a [`StrategyConfigError`]
/// at build time.
#[derive(Clone, Debug)]
pub struct ConnectionParams {
    /// The node's own identity (folded into the verifiable edge predicate).
    pub self_id: PeerId,
    /// The link kind the built instance dials — selects the hash domain
    /// (`Relay` for the relay seam, `Publisher` for the publisher seam).
    pub kind: LinkKind,
    /// The fixed target connection degree `target_degree` — required by `hash-gated` (bucket count derives from it).
    pub target_degree: Option<usize>,
    /// Optional pinned bucket count `B`. When set, it overrides the per-topic
    /// count derived from `target_degree` on **both** seams, so the edge
    /// predicate is verifiable by construction (no dependence on the two ends
    /// having folded the same candidate set). Must be `≥ 1` if supplied.
    pub bucket_count: Option<usize>,
    /// Use the symmetric edge predicate (M4). One CLI flag sets this on the
    /// relay selection AND acceptance params together. Publisher params leave
    /// it `false`: M4 itself uses no publisher links at all ("no seeding
    /// mechanism" — `m4/README.md`; a publisher's own symmetric relay links
    /// carry its message out), and no published model defines symmetric
    /// publisher links, so a publisher instance configured alongside the flag
    /// stays directional.
    pub symmetric: bool,
}

/// Already-parsed parameters for the acceptance (inbound/downstream) seam.
#[derive(Clone, Debug)]
pub struct AcceptanceParams {
    /// The node's own identity (the candidate side of the verified edge).
    pub self_id: PeerId,
    /// The link kind the built instance admits — selects the hash domain and
    /// which accepted-link class its capacity counts.
    pub kind: LinkKind,
    /// The fixed target connection degree `target_degree` — required by `hash-gated-bounded`.
    pub target_degree: Option<usize>,
    /// Optional pinned bucket count `B` (see [`ConnectionParams::bucket_count`]);
    /// the acceptor must use the same value the dialer does. Must be `≥ 1` if
    /// supplied.
    pub bucket_count: Option<usize>,
    /// Accept-cap buffer `c` in `OC = ⌈target_degree + c·√target_degree⌉` (default 3).
    pub cap_buffer: usize,
    /// Use the symmetric edge predicate (M4) — set from the same CLI flag as
    /// the dial side. Reciprocity is constructed by the symmetric handshake
    /// (ADR 0034), so a predicate mismatch between the two seams costs
    /// dropped dials at worst, never one-way links.
    pub symmetric: bool,
}

/// The error a strategy kind raises when the configuration lacks a parameter
/// that kind requires.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum StrategyConfigError {
    /// The named strategy requires a parameter that was not supplied.
    #[error("the '{strategy}' strategy requires {parameter}")]
    MissingParameter {
        /// The strategy that needs the parameter (its config name).
        strategy: &'static str,
        /// The missing parameter, in operator-facing terms.
        parameter: &'static str,
    },
    /// The named strategy was supplied a parameter it cannot use.
    #[error("the '{strategy}' strategy requires {parameter} to be {constraint}")]
    InvalidParameter {
        /// The strategy that rejects the value (its config name).
        strategy: &'static str,
        /// The offending parameter, in operator-facing terms.
        parameter: &'static str,
        /// The constraint the value violated, in operator-facing terms.
        constraint: &'static str,
    },
}

/// Validate an optional pinned bucket count `B`: it must be `≥ 1` (a `B` of 0
/// would divide by zero in the edge predicate). Returns the value unchanged when
/// valid so a caller can thread it straight into a strategy.
pub(crate) fn validate_bucket_count(
    strategy: &'static str,
    bucket_count: Option<usize>,
) -> Result<Option<usize>, StrategyConfigError> {
    if bucket_count == Some(0) {
        return Err(StrategyConfigError::InvalidParameter {
            strategy,
            parameter: "the bucket count (--bucket-count)",
            constraint: "at least 1",
        });
    }
    Ok(bucket_count)
}

/// Validate the target connection degree a strategy requires: it must be
/// supplied and `≥ 1` (a degree of 0 degenerates the dial and accept seams in
/// opposite directions — dial-everything vs accept-nothing). Shared by both
/// seams' `build` arms so the two cannot drift on what a valid degree is.
pub(crate) fn require_target_degree(
    strategy: &'static str,
    kind: LinkKind,
    target_degree: Option<usize>,
) -> Result<usize, StrategyConfigError> {
    // The flag that supplies the degree differs per seam family; the error
    // must name the one the operator actually has to set.
    let (missing, invalid) = match kind {
        LinkKind::Relay => (
            "a relay degree (--relay-degree)",
            "the relay degree (--relay-degree)",
        ),
        LinkKind::Publisher => (
            "a publisher degree (--publisher-degree)",
            "the publisher degree (--publisher-degree)",
        ),
    };
    let target_degree = target_degree.ok_or(StrategyConfigError::MissingParameter {
        strategy,
        parameter: missing,
    })?;
    if target_degree == 0 {
        return Err(StrategyConfigError::InvalidParameter {
            strategy,
            parameter: invalid,
            constraint: "greater than 0",
        });
    }
    Ok(target_degree)
}

/// The concrete strategy set handed to [`Node::new`](crate::Node::new), produced
/// by [`NodeStrategies::new`] (the selection-plane construction), by
/// [`NodeStrategiesBuilder::build`], or by [`NodeStrategies::relay_only`] for
/// direct construction. Four link seams: the relay pair (required) and the
/// publisher pair (optional — `None` disables publisher links: no dials on the
/// selection side, inbound publisher requests dropped on the acceptance side).
/// Fan-out stays injected separately — it is not built through this two-phase
/// seam.
pub struct NodeStrategies {
    /// The relay-link selection (dial/upstream) strategy.
    pub relay_connection: Arc<dyn ConnectionStrategy>,
    /// The relay-link acceptance (downstream) strategy.
    pub relay_acceptance: Arc<dyn ConnectionAcceptanceStrategy>,
    /// The publisher-link selection strategy (standing initiation dials).
    pub publisher_connection: Option<Arc<dyn ConnectionStrategy>>,
    /// The publisher-link acceptance strategy (inbound initiation links).
    pub publisher_acceptance: Option<Arc<dyn ConnectionAcceptanceStrategy>>,
    /// Whether relay links are established with the **symmetric**
    /// (bidirectional) handshake — M4 (ADR 0034): the dial pass speaks the
    /// symmetric vocabulary and one accept decision records each link in both
    /// directions on both ends. `false` (the default) on every directional
    /// model; inbound symmetric handshakes are then dropped outright.
    pub symmetric_edges: bool,
}

/// Phase 1 of construction: the resolved per-seam strategy *kinds*, awaiting
/// their parameters. Create it with [`NodeStrategies::builder`].
pub struct NodeStrategiesBuilder {
    relay_connection: ConnectionStrategyKind,
    relay_acceptance: AcceptanceStrategyKind,
}

impl NodeStrategies {
    /// Phase 1: capture the resolved strategy keys for each seam. Nothing is
    /// constructed until [`NodeStrategiesBuilder::build`].
    #[must_use]
    pub fn builder(
        relay_connection: ConnectionStrategyKind,
        relay_acceptance: AcceptanceStrategyKind,
    ) -> NodeStrategiesBuilder {
        NodeStrategiesBuilder {
            relay_connection,
            relay_acceptance,
        }
    }

    /// A relay-only strategy set from already-constructed instances — the M2
    /// baseline shape (publisher links disabled), and the concise form for
    /// tests that inject concrete strategies directly.
    #[must_use]
    pub fn relay_only(
        relay_connection: Arc<dyn ConnectionStrategy>,
        relay_acceptance: Arc<dyn ConnectionAcceptanceStrategy>,
    ) -> Self {
        Self {
            relay_connection,
            relay_acceptance,
            publisher_connection: None,
            publisher_acceptance: None,
            symmetric_edges: false,
        }
    }

    /// Switch the set to the symmetric (bidirectional) relay handshake — M4.
    /// Pair it with relay strategies drawing the symmetric predicate.
    #[must_use]
    pub fn with_symmetric_edges(mut self, symmetric: bool) -> Self {
        self.symmetric_edges = symmetric;
        self
    }
}

impl NodeStrategiesBuilder {
    /// Phase 2: bind each seam's params, validate the parameters each chosen
    /// strategy requires, and construct the whole set — surfacing the first
    /// [`StrategyConfigError`] so the edge maps it once. The publisher pair is
    /// `None` here; the edge fills it when publisher flags are configured.
    pub fn build(
        self,
        relay_connection: &ConnectionParams,
        relay_acceptance: &AcceptanceParams,
    ) -> Result<NodeStrategies, StrategyConfigError> {
        Ok(NodeStrategies {
            relay_connection: self.relay_connection.build(relay_connection)?,
            relay_acceptance: self.relay_acceptance.build(relay_acceptance)?,
            publisher_connection: None,
            publisher_acceptance: None,
            // One flag configures the predicate on the relay params AND the
            // handshake vocabulary: a symmetric dial pass is what makes the
            // symmetric draws materialise as constructed pairs.
            symmetric_edges: relay_connection.symmetric,
        })
    }
}

/// Already-parsed parameters for one seam's [`Selection`] instance: the
/// selection-plane knobs plus the sampling seed.
// 017-FR-001/FR-002 knob domains; research R1/R5.
#[derive(Clone, Debug)]
pub struct SelectionParams {
    /// The node's own identity (the requester side of the edge predicate).
    pub self_id: PeerId,
    /// The link kind the built instance dials — selects the hash domain.
    pub kind: LinkKind,
    /// Use the symmetric edge predicate on the gate. One flag sets this on
    /// the relay selection AND acceptance params together; publisher params
    /// leave it `false` (no published model defines symmetric publisher
    /// links — ADR 0034's boundary).
    pub symmetric: bool,
    /// The bucket count (hash-gate width). `None` = ungated. `Some(0)` is
    /// rejected at construction; `Some(1)` is legal here (≡ ungated — the
    /// sweep config's axis point) even where the operator CLI rejects it.
    pub bucket_count: Option<usize>,
    /// The pick count: `None` = dial every gate survivor; `Some(0)` = dial
    /// none; `Some(k)` = exactly `min(k, survivors)` seeded uniform picks.
    pub pick_count: Option<usize>,
    /// The 32-byte sampling seed; read only when the pick count is `≥ 1`.
    pub seed: [u8; 32],
}

/// Already-parsed parameters for one seam's [`UnifiedAcceptance`] instance.
///
/// Takes the `AcceptanceParams` name once the two-phase builder and its
/// param structs are deleted (017-T016).
// 017-FR-010/FR-011/FR-012; research R1/R5.
#[derive(Clone, Debug)]
pub struct UnifiedAcceptanceParams {
    /// The node's own identity (the candidate side of the verified edge).
    pub self_id: PeerId,
    /// The link kind the built instance admits — selects the hash domain and
    /// which accepted-link class its capacity counts.
    pub kind: LinkKind,
    /// Verify with the symmetric edge predicate — set from the same flag as
    /// the dial side (both relay seams switch together).
    pub symmetric: bool,
    /// The bucket count the acceptor verifies at — the **post-opt-out gate
    /// value**: the edge passes the seam's bucket count so acceptors verify
    /// exactly the `B` the dialers use, or `None` when verification is
    /// opted out (or the seam is ungated). Domain as on
    /// [`SelectionParams::bucket_count`].
    pub bucket_count: Option<usize>,
    /// The absolute per-topic serving cap: `None` = unbounded; `Some(0)` =
    /// serve none (every new link refused with the explicit rejection).
    pub accept_cap: Option<usize>,
}

/// Validate a plane bucket count for construction: `Some(0)` is rejected (a
/// zero-bucket gate is meaningless); `Some(1)` is legal (≡ ungated — the
/// sweep config's boundary axis point; the operator CLI applies its own
/// stricter rule at the edge).
fn validate_plane_bucket_count(
    strategy: &'static str,
    kind: LinkKind,
    bucket_count: Option<usize>,
) -> Result<Option<usize>, StrategyConfigError> {
    if bucket_count == Some(0) {
        return Err(StrategyConfigError::InvalidParameter {
            strategy,
            parameter: match kind {
                LinkKind::Relay => "the relay bucket count",
                LinkKind::Publisher => "the publisher bucket count",
            },
            constraint: "at least 1",
        });
    }
    Ok(bucket_count)
}

/// Build one seam's [`Selection`] instance from its plane params.
fn build_selection(
    params: SelectionParams,
) -> Result<Arc<dyn ConnectionStrategy>, StrategyConfigError> {
    let bucket_count = validate_plane_bucket_count("selection", params.kind, params.bucket_count)?;
    Ok(Arc::new(
        Selection::new(params.self_id, params.seed)
            .for_kind(params.kind)
            .with_symmetric(params.symmetric)
            .with_bucket_count(bucket_count)
            .with_pick_count(params.pick_count),
    ))
}

/// Build one seam's [`UnifiedAcceptance`] instance from its plane params.
fn build_unified_acceptance(
    params: UnifiedAcceptanceParams,
) -> Result<Arc<dyn ConnectionAcceptanceStrategy>, StrategyConfigError> {
    let gate = validate_plane_bucket_count("acceptance", params.kind, params.bucket_count)?;
    Ok(Arc::new(
        UnifiedAcceptance::new(params.self_id)
            .for_kind(params.kind)
            .with_symmetric(params.symmetric)
            .with_gate(gate)
            .with_accept_cap(params.accept_cap),
    ))
}

impl NodeStrategies {
    /// Construct the full strategy set from selection-plane knobs — one
    /// fallible call building the relay pair always and the publisher pair
    /// when its params are supplied (`None` keeps the publisher seam off by
    /// construction: no dial pass, inbound publisher requests dropped). The
    /// first [`StrategyConfigError`] surfaces here, so the edge maps it
    /// once — the publisher pair no longer bypasses construction.
    // 017-FR-008 (seam off by construction), 017-FR-016 (seed threading);
    // research R5 (absorbs §1.2 item 6).
    pub fn new(
        relay_selection: SelectionParams,
        relay_acceptance: UnifiedAcceptanceParams,
        publisher: Option<(SelectionParams, UnifiedAcceptanceParams)>,
    ) -> Result<Self, StrategyConfigError> {
        // One flag configures the predicate on both relay params AND the
        // handshake vocabulary (a symmetric dial pass is what makes the
        // symmetric draws materialise as constructed pairs).
        let symmetric_edges = relay_selection.symmetric;
        let relay_connection = build_selection(relay_selection)?;
        let relay_acceptance = build_unified_acceptance(relay_acceptance)?;
        let (publisher_connection, publisher_acceptance) = match publisher {
            None => (None, None),
            Some((selection, acceptance)) => (
                Some(build_selection(selection)?),
                Some(build_unified_acceptance(acceptance)?),
            ),
        };
        Ok(Self {
            relay_connection,
            relay_acceptance,
            publisher_connection,
            publisher_acceptance,
            symmetric_edges,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{NodeStrategies, SelectionParams, StrategyConfigError, UnifiedAcceptanceParams};
    use crate::connection_state::LinkKind;
    use crate::strategies::test_support::peer;

    fn selection_params(kind: LinkKind) -> SelectionParams {
        SelectionParams {
            self_id: peer("self"),
            kind,
            symmetric: false,
            bucket_count: None,
            pick_count: None,
            seed: [0u8; 32],
        }
    }

    fn acceptance_params(kind: LinkKind) -> UnifiedAcceptanceParams {
        UnifiedAcceptanceParams {
            self_id: peer("self"),
            kind,
            symmetric: false,
            bucket_count: None,
            accept_cap: None,
        }
    }

    // 017-FR-008: no publisher params ⇒ the publisher seam stays off by
    // construction; the relay pair always builds.
    #[test]
    fn relay_only_construction_leaves_the_publisher_seam_off() {
        let strategies = NodeStrategies::new(
            selection_params(LinkKind::Relay),
            acceptance_params(LinkKind::Relay),
            None,
        )
        .expect("plane origin builds");
        assert!(strategies.publisher_connection.is_none());
        assert!(strategies.publisher_acceptance.is_none());
        assert!(!strategies.symmetric_edges);
    }

    // Publisher params supplied ⇒ both publisher seams are built through the
    // same call (no bypass — one error-map site).
    #[test]
    fn publisher_params_build_the_publisher_pair() {
        let strategies = NodeStrategies::new(
            selection_params(LinkKind::Relay),
            acceptance_params(LinkKind::Relay),
            Some((
                selection_params(LinkKind::Publisher),
                acceptance_params(LinkKind::Publisher),
            )),
        )
        .expect("publisher pair builds");
        assert!(strategies.publisher_connection.is_some());
        assert!(strategies.publisher_acceptance.is_some());
    }

    // The relay selection's symmetric knob drives the handshake vocabulary.
    #[test]
    fn symmetric_knob_threads_into_symmetric_edges() {
        let mut relay_selection = selection_params(LinkKind::Relay);
        relay_selection.symmetric = true;
        let mut relay_acceptance = acceptance_params(LinkKind::Relay);
        relay_acceptance.symmetric = true;
        let strategies = NodeStrategies::new(relay_selection, relay_acceptance, None)
            .expect("symmetric plane point builds");
        assert!(strategies.symmetric_edges);
    }

    // Core-domain validation: a bucket count of 0 is rejected on either
    // side of either seam; 1 is legal (≡ ungated — the sweep config's
    // boundary axis point, rejected only by the operator CLI's own rule).
    #[test]
    fn zero_bucket_count_is_rejected_and_one_is_legal() {
        let mut gated_zero = selection_params(LinkKind::Relay);
        gated_zero.bucket_count = Some(0);
        assert!(matches!(
            NodeStrategies::new(gated_zero, acceptance_params(LinkKind::Relay), None),
            Err(StrategyConfigError::InvalidParameter { .. }),
        ));

        let mut accept_zero = acceptance_params(LinkKind::Publisher);
        accept_zero.bucket_count = Some(0);
        assert!(matches!(
            NodeStrategies::new(
                selection_params(LinkKind::Relay),
                acceptance_params(LinkKind::Relay),
                Some((selection_params(LinkKind::Publisher), accept_zero)),
            ),
            Err(StrategyConfigError::InvalidParameter { .. }),
        ));

        let mut gated_one = selection_params(LinkKind::Relay);
        gated_one.bucket_count = Some(1);
        assert!(
            NodeStrategies::new(gated_one, acceptance_params(LinkKind::Relay), None).is_ok(),
            "bucket count 1 is the ungated axis point — legal in core construction",
        );
    }
}

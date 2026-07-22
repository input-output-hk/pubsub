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

use std::sync::Arc;

use crate::connection_state::LinkKind;
use crate::peer::PeerId;
use crate::strategies::acceptance::{AcceptanceStrategyKind, ConnectionAcceptanceStrategy};
use crate::strategies::connection::{ConnectionStrategy, ConnectionStrategyKind};

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
    /// Use the symmetric edge predicate (M4) — must match the dial side (one
    /// CLI flag sets both).
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
/// by [`NodeStrategiesBuilder::build`] (or [`NodeStrategies::relay_only`] for
/// direct construction). Four link seams: the relay pair (required) and the
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

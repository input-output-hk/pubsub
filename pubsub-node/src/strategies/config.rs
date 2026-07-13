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
//! scattered across the edge. (Fan-out stays `ForwardToAll`, injected separately;
//! it is not built through this two-phase seam.)

use std::sync::Arc;

use crate::connection_state::LinkRole;
use crate::peer::PeerId;
use crate::strategies::acceptance::{AcceptanceStrategyKind, ConnectionAcceptanceStrategy};
use crate::strategies::connection::{ConnectionStrategy, ConnectionStrategyKind};
use crate::strategies::publish::{PublishStrategy, PublishStrategyKind};

/// Already-parsed parameters for the connection (dial/upstream) seam. A field a
/// chosen kind requires but that is left `None` yields a [`StrategyConfigError`]
/// at build time.
#[derive(Clone, Debug)]
pub struct ConnectionParams {
    /// The node's own identity (folded into the verifiable edge predicate).
    pub self_id: PeerId,
    /// The fixed relay connection degree `relay_degree` — required by `hash-gated` (bucket count derives from it).
    pub relay_degree: Option<usize>,
    /// Optional pinned bucket count `B`. When set, it overrides the per-topic
    /// count derived from `relay_degree` on **both** seams, so the edge
    /// predicate is verifiable by construction (no dependence on the two ends
    /// having folded the same candidate set). Must be `≥ 1` if supplied.
    pub bucket_count: Option<usize>,
}

/// Already-parsed parameters for the relay acceptance (inbound) seam.
#[derive(Clone, Debug)]
pub struct AcceptanceParams {
    /// The node's own identity (the candidate side of the verified edge).
    pub self_id: PeerId,
    /// The fixed relay connection degree `relay_degree` — required by `hash-gated-bounded`.
    pub relay_degree: Option<usize>,
    /// Optional pinned bucket count `B` (see [`ConnectionParams::bucket_count`]);
    /// the acceptor must use the same value the dialer does. Must be `≥ 1` if
    /// supplied.
    pub bucket_count: Option<usize>,
    /// Accept-cap buffer `c` in `OC = ⌈relay_degree + c·√relay_degree⌉` (default 3).
    pub cap_buffer: usize,
}

/// Already-parsed parameters for the publish (publishing-link dial) seam
/// (feature 015, ADR 0033).
#[derive(Clone, Debug)]
pub struct PublishParams {
    /// The node's own identity (folded into the publish edge predicate).
    pub self_id: PeerId,
    /// The publish degree `publish_degree` — required by `hash-gated` (the
    /// publish bucket count derives from it). Independent of `relay_degree`.
    pub publish_degree: Option<usize>,
    /// The relay degree the M3 trigger recomputes the relay predicate with —
    /// required by `hash-gated`; must match the relay seam's degree.
    pub relay_degree: Option<usize>,
    /// Optional pinned publish bucket count `B_p`. Must be `≥ 1` if supplied.
    pub bucket_count: Option<usize>,
}

/// Already-parsed parameters for the publish acceptance (inbound
/// publishing-link) seam (feature 015, ADR 0033). The same acceptance kinds as
/// the relay slot, instantiated with the publish degree and the publish hash
/// domain; the cap `⌈publish_degree + c·√publish_degree⌉` counts inbound
/// publishing links only.
#[derive(Clone, Debug)]
pub struct PublishAcceptanceParams {
    /// The node's own identity (the candidate side of the verified publish edge).
    pub self_id: PeerId,
    /// The publish degree — required by every kind except `accept-from-all`.
    pub publish_degree: Option<usize>,
    /// Optional pinned publish bucket count `B_p`. Must be `≥ 1` if supplied.
    pub bucket_count: Option<usize>,
    /// Accept-cap buffer `c` (default 3), shared with the relay seam.
    pub cap_buffer: usize,
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

/// Validate a degree parameter a strategy requires: it must be supplied and
/// `≥ 1` (a degree of 0 degenerates the dial and accept seams in opposite
/// directions — dial-everything vs accept-nothing). Shared by every seam's
/// `build` arm so they cannot drift on what a valid degree is; the caller names
/// the parameter in operator-facing terms (`--relay-degree` /
/// `--publish-degree`).
pub(crate) fn require_degree(
    strategy: &'static str,
    degree: Option<usize>,
    missing: &'static str,
    invalid: &'static str,
) -> Result<usize, StrategyConfigError> {
    let degree = degree.ok_or(StrategyConfigError::MissingParameter {
        strategy,
        parameter: missing,
    })?;
    if degree == 0 {
        return Err(StrategyConfigError::InvalidParameter {
            strategy,
            parameter: invalid,
            constraint: "greater than 0",
        });
    }
    Ok(degree)
}

/// Validate the relay degree a relay-seam strategy requires (the
/// [`require_degree`] instance both relay seams share).
pub(crate) fn require_relay_degree(
    strategy: &'static str,
    relay_degree: Option<usize>,
) -> Result<usize, StrategyConfigError> {
    require_degree(
        strategy,
        relay_degree,
        "a relay degree (--relay-degree)",
        "the relay degree (--relay-degree)",
    )
}

/// The concrete strategy set handed to [`Node::new`](crate::Node::new), produced
/// by [`NodeStrategiesBuilder::build`]. (Fan-out stays `ForwardToAll`, injected
/// separately — it is not built through this two-phase seam.)
pub struct NodeStrategies {
    /// The relay connection (dial/upstream) strategy.
    pub connection: Arc<dyn ConnectionStrategy>,
    /// The relay inbound-acceptance strategy.
    pub acceptance: Arc<dyn ConnectionAcceptanceStrategy>,
    /// The publish-target (publishing-link dial) strategy (ADR 0033).
    pub publish: Arc<dyn PublishStrategy>,
    /// The publish inbound-acceptance strategy — the same kinds as the relay
    /// slot, instantiated with publish parameters.
    pub publish_acceptance: Arc<dyn ConnectionAcceptanceStrategy>,
}

/// Phase 1 of construction: the resolved per-seam strategy *kinds*, awaiting
/// their parameters. Create it with [`NodeStrategies::builder`].
pub struct NodeStrategiesBuilder {
    connection: ConnectionStrategyKind,
    acceptance: AcceptanceStrategyKind,
    publish: PublishStrategyKind,
    publish_acceptance: AcceptanceStrategyKind,
}

impl NodeStrategies {
    /// Phase 1: capture the resolved strategy keys for each seam. Nothing is
    /// constructed until [`NodeStrategiesBuilder::build`].
    #[must_use]
    pub fn builder(
        connection: ConnectionStrategyKind,
        acceptance: AcceptanceStrategyKind,
        publish: PublishStrategyKind,
        publish_acceptance: AcceptanceStrategyKind,
    ) -> NodeStrategiesBuilder {
        NodeStrategiesBuilder {
            connection,
            acceptance,
            publish,
            publish_acceptance,
        }
    }
}

impl NodeStrategiesBuilder {
    /// Phase 2: bind each seam's params, validate the parameters each chosen
    /// strategy requires, and construct the whole set — surfacing the first
    /// [`StrategyConfigError`] so the edge maps it once. The publish acceptance
    /// slot is built from the acceptance kinds with publish parameters and
    /// retargeted at the `Publisher` role (ADR 0033).
    pub fn build(
        self,
        connection: &ConnectionParams,
        acceptance: &AcceptanceParams,
        publish: &PublishParams,
        publish_acceptance: &PublishAcceptanceParams,
    ) -> Result<NodeStrategies, StrategyConfigError> {
        Ok(NodeStrategies {
            connection: self.connection.build(connection)?,
            acceptance: self.acceptance.build(acceptance)?,
            publish: self.publish.build(publish)?,
            publish_acceptance: self
                .publish_acceptance
                .build_for_role(LinkRole::Publisher, publish_acceptance)?,
        })
    }
}

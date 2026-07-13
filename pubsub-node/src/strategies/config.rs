//! Two-phase strategy construction (ADR 0028, extended by ADR 0034).
//!
//! **Phase 1 — key → builder.** The edge parses each seam's strategy *key* into
//! its kind enum (clap: absent → the seam default, unknown key → rejected at
//! CLI parse). [`NodeStrategies::builder`] holds the resolved kinds; nothing is
//! constructed yet.
//!
//! **Phase 2 — params → strategy.** [`NodeStrategiesBuilder::build`] takes one
//! params struct per slot — already-typed values, no `clap` in the core — and
//! constructs every seam, validating the parameters each chosen strategy
//! requires. Both link-selection slots share [`SelectionParams`] and both
//! acceptance slots share [`AcceptanceParams`]; the `role` field picks the hash
//! domain and the operator-facing flag names in errors, so one kind family
//! serves both roles (ADR 0034). A required param left `None` yields a
//! [`StrategyConfigError`]; the edge maps it **once**.

use std::sync::Arc;

use crate::connection_state::LinkRole;
use crate::peer::PeerId;
use crate::strategies::acceptance::{AcceptanceStrategyKind, ConnectionAcceptanceStrategy};
use crate::strategies::fanout::{FanoutStrategy, FanoutStrategyKind};
use crate::strategies::selection::{LinkSelectionKind, LinkSelectionStrategy};

/// Already-parsed parameters for one link-selection slot (relay or publish).
#[derive(Clone, Debug)]
pub struct SelectionParams {
    /// The node's own identity (folded into the role's edge predicate).
    pub self_id: PeerId,
    /// Which role slot these params configure — picks the hash domain and the
    /// flag names in validation errors.
    pub role: LinkRole,
    /// The slot's target out-degree (`relay_degree` / `publish_degree`) —
    /// required by `hash-gated` (the bucket count derives from it).
    pub degree: Option<usize>,
    /// Optional pinned bucket count `B`. When set, it overrides the per-topic
    /// derived count on both ends of the slot's handshake, so the edge
    /// predicate is verifiable by construction. Must be `≥ 1` if supplied.
    pub bucket_count: Option<usize>,
    /// Evaluate the **symmetric** edge predicate instead of the directional
    /// one (ADR 0035 — the M4 bidirectional mode). Applies to the `hash-gated`
    /// kind; one CLI flag wires selection and acceptance together so the two
    /// seams cannot disagree.
    pub symmetric: bool,
}

impl SelectionParams {
    pub(crate) fn missing_degree(&self) -> &'static str {
        match self.role {
            LinkRole::Relay => "a relay degree (--relay-degree)",
            LinkRole::Publisher => "a publish degree (--publish-degree)",
        }
    }

    pub(crate) fn invalid_degree(&self) -> &'static str {
        match self.role {
            LinkRole::Relay => "the relay degree (--relay-degree)",
            LinkRole::Publisher => "the publish degree (--publish-degree)",
        }
    }
}

/// Already-parsed parameters for one inbound-acceptance slot (relay or
/// publish). The same kind family serves both slots; the `role` scopes the
/// prelude scan, the cap, and the predicate domain (ADR 0033/0034).
#[derive(Clone, Debug)]
pub struct AcceptanceParams {
    /// The node's own identity (the candidate side of the verified edge).
    pub self_id: PeerId,
    /// Which role slot these params configure.
    pub role: LinkRole,
    /// The slot's degree (`relay_degree` / `publish_degree`) — required by
    /// every kind except `accept-from-all` (cap and bucket count derive from it).
    pub degree: Option<usize>,
    /// Optional pinned bucket count `B` (see [`SelectionParams::bucket_count`]);
    /// the acceptor must use the same value the dialer does. Must be `≥ 1` if
    /// supplied.
    pub bucket_count: Option<usize>,
    /// Accept-cap buffer `c` in `⌈degree + c·√degree⌉` (default 3).
    pub cap_buffer: usize,
    /// Verify the **symmetric** edge predicate instead of the directional one
    /// (ADR 0035) — must match the dialers' mode.
    pub symmetric: bool,
}

impl AcceptanceParams {
    pub(crate) fn missing_degree(&self) -> &'static str {
        match self.role {
            LinkRole::Relay => "a relay degree (--relay-degree)",
            LinkRole::Publisher => "a publish degree (--publish-degree)",
        }
    }

    pub(crate) fn invalid_degree(&self) -> &'static str {
        match self.role {
            LinkRole::Relay => "the relay degree (--relay-degree)",
            LinkRole::Publisher => "the publish degree (--publish-degree)",
        }
    }
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
/// directions — dial-everything vs accept-nothing). Shared by every slot's
/// `build` arm so they cannot drift on what a valid degree is; the caller
/// names the parameter in operator-facing terms via the params helpers.
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

/// The concrete strategy set handed to [`Node::new`](crate::Node::new), produced
/// by [`NodeStrategiesBuilder::build`].
pub struct NodeStrategies {
    /// The relay link-selection slot.
    pub relay_selection: Arc<dyn LinkSelectionStrategy>,
    /// The relay inbound-acceptance slot.
    pub relay_acceptance: Arc<dyn ConnectionAcceptanceStrategy>,
    /// The publish link-selection slot (standing initiation links, ADR 0033/0034).
    pub publish_selection: Arc<dyn LinkSelectionStrategy>,
    /// The publish inbound-acceptance slot.
    pub publish_acceptance: Arc<dyn ConnectionAcceptanceStrategy>,
    /// The origin-aware fan-out policy — the dissemination-model knob
    /// (ADR 0034: `forward-to-all` unions cells, `role-scoped` is the M3
    /// partition).
    pub fanout: Arc<dyn FanoutStrategy>,
}

/// Phase 1 of construction: the resolved per-slot strategy *kinds*, awaiting
/// their parameters. Create it with [`NodeStrategies::builder`].
pub struct NodeStrategiesBuilder {
    relay_selection: LinkSelectionKind,
    relay_acceptance: AcceptanceStrategyKind,
    publish_selection: LinkSelectionKind,
    publish_acceptance: AcceptanceStrategyKind,
    fanout: FanoutStrategyKind,
}

impl NodeStrategies {
    /// Phase 1: capture the resolved strategy keys for each slot. Nothing is
    /// constructed until [`NodeStrategiesBuilder::build`].
    #[must_use]
    pub fn builder(
        relay_selection: LinkSelectionKind,
        relay_acceptance: AcceptanceStrategyKind,
        publish_selection: LinkSelectionKind,
        publish_acceptance: AcceptanceStrategyKind,
        fanout: FanoutStrategyKind,
    ) -> NodeStrategiesBuilder {
        NodeStrategiesBuilder {
            relay_selection,
            relay_acceptance,
            publish_selection,
            publish_acceptance,
            fanout,
        }
    }
}

impl NodeStrategiesBuilder {
    /// Phase 2: bind each slot's params, validate the parameters each chosen
    /// strategy requires, and construct the whole set — surfacing the first
    /// [`StrategyConfigError`] so the edge maps it once. Callers pass the same
    /// param *shapes* for both roles; the `role` field inside each params
    /// struct is what differentiates the slots (ADR 0034).
    pub fn build(
        self,
        relay_selection: &SelectionParams,
        relay_acceptance: &AcceptanceParams,
        publish_selection: &SelectionParams,
        publish_acceptance: &AcceptanceParams,
    ) -> Result<NodeStrategies, StrategyConfigError> {
        Ok(NodeStrategies {
            relay_selection: self.relay_selection.build(relay_selection)?,
            relay_acceptance: self.relay_acceptance.build(relay_acceptance)?,
            publish_selection: self.publish_selection.build(publish_selection)?,
            publish_acceptance: self.publish_acceptance.build(publish_acceptance)?,
            fanout: self.fanout.build(),
        })
    }
}

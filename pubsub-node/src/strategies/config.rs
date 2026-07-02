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
    /// Public genesis nonce for the edge predicate (default 0).
    pub genesis: u64,
    /// The fixed target connection degree `target_degree` — required by `hash-gated` (bucket count derives from it).
    pub target_degree: Option<usize>,
}

/// Already-parsed parameters for the acceptance (inbound/downstream) seam.
#[derive(Clone, Debug)]
pub struct AcceptanceParams {
    /// The node's own identity (the candidate side of the verified edge).
    pub self_id: PeerId,
    /// Public genesis nonce for the edge predicate (default 0).
    pub genesis: u64,
    /// The fixed target connection degree `target_degree` — required by `verifiable-bounded`.
    pub target_degree: Option<usize>,
    /// Accept-cap buffer `c` in `OC = ⌈target_degree + c·√target_degree⌉` (default 3).
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
}

/// The concrete strategy set handed to [`Node::new`](crate::Node::new), produced
/// by [`NodeStrategiesBuilder::build`]. (Fan-out stays `ForwardToAll`, injected
/// separately — it is not built through this two-phase seam.)
pub struct NodeStrategies {
    /// The connection (dial/upstream) strategy.
    pub connection: Arc<dyn ConnectionStrategy>,
    /// The inbound-acceptance (downstream) strategy.
    pub acceptance: Arc<dyn ConnectionAcceptanceStrategy>,
}

/// Phase 1 of construction: the resolved per-seam strategy *kinds*, awaiting
/// their parameters. Create it with [`NodeStrategies::builder`].
pub struct NodeStrategiesBuilder {
    connection: ConnectionStrategyKind,
    acceptance: AcceptanceStrategyKind,
}

impl NodeStrategies {
    /// Phase 1: capture the resolved strategy keys for each seam. Nothing is
    /// constructed until [`NodeStrategiesBuilder::build`].
    #[must_use]
    pub fn builder(
        connection: ConnectionStrategyKind,
        acceptance: AcceptanceStrategyKind,
    ) -> NodeStrategiesBuilder {
        NodeStrategiesBuilder {
            connection,
            acceptance,
        }
    }
}

impl NodeStrategiesBuilder {
    /// Phase 2: bind each seam's params, validate the parameters each chosen
    /// strategy requires, and construct the whole set — surfacing the first
    /// [`StrategyConfigError`] so the edge maps it once.
    pub fn build(
        self,
        connection: &ConnectionParams,
        acceptance: &AcceptanceParams,
    ) -> Result<NodeStrategies, StrategyConfigError> {
        Ok(NodeStrategies {
            connection: self.connection.build(connection)?,
            acceptance: self.acceptance.build(acceptance)?,
        })
    }
}

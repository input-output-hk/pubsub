//! Parsed strategy-construction parameters and the error a strategy kind raises
//! when a required one is missing (ADR 0028).
//!
//! Parse-at-the-edge: the CLI/loader parses raw arguments into [`StrategyParams`]
//! (already-typed values — no `clap` in the core), and each strategy kind's
//! `build` consumes only the params it needs, validating the required ones. So
//! construction *and* required-parameter validation live with the strategy, not
//! scattered across the edge; the edge just maps a [`StrategyConfigError`] once.

use crate::peer::PeerId;

/// Already-parsed parameters a strategy kind may draw on to build its concrete
/// strategy. Each kind reads only the fields relevant to it; a field left
/// `None` that a chosen kind requires yields a [`StrategyConfigError`].
#[derive(Clone, Debug)]
pub struct StrategyParams {
    /// The node's own identity (folded into seeded selection).
    pub self_id: PeerId,
    /// Network seed for the deterministic seeded strategies.
    pub seed: u64,
    /// Max upstream peers dialed per topic — required by the seeded-bounded
    /// connection-selection strategy.
    pub upstream_degree: Option<usize>,
    /// Max downstream peers accepted per topic — required by the bounded
    /// acceptance strategy.
    pub downstream_degree: Option<usize>,
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

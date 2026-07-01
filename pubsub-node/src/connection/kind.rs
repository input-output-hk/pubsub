//! The selectable connection-selection strategies, named for configuration.
//!
//! [`ConnectionStrategyKind`] is the config-facing enum: it parses
//! case-insensitively from a readable name (`connect-to-all`, `seeded-bounded`)
//! and carries a stable, unique byte-string [`tag`](ConnectionStrategyKind::tag)
//! per variant, used as the domain separator in any keyed hashing the strategy
//! performs (so distinct strategies never share a hash domain). The edge
//! (CLI/loader) maps a kind plus its parameters to a concrete
//! [`ConnectionStrategy`](super::ConnectionStrategy) instance — the kind itself
//! constructs nothing.

use std::str::FromStr;
use std::sync::Arc;

use super::{ConnectToAllCandidates, ConnectionStrategy, SeededBoundedSelection};
use crate::strategy_config::{StrategyConfigError, StrategyParams};

/// A selectable connection-selection strategy, identified by a readable name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionStrategyKind {
    /// The full-mesh policy ([`ConnectToAllCandidates`](super::ConnectToAllCandidates)).
    ConnectToAll,
    /// The seeded, bounded policy ([`SeededBoundedSelection`](super::SeededBoundedSelection)).
    SeededBounded,
}

impl ConnectionStrategyKind {
    /// The canonical, lower-case configuration name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ConnectToAll => "connect-to-all",
            Self::SeededBounded => "seeded-bounded",
        }
    }

    /// A stable, unique byte-string identifying this strategy — used as the
    /// domain-separation tag in keyed hashing so distinct strategies never share
    /// a hash domain.
    #[must_use]
    pub const fn tag(self) -> &'static [u8] {
        match self {
            Self::ConnectToAll => b"pubsub/connection-strategy/connect-to-all",
            Self::SeededBounded => b"pubsub/connection-strategy/seeded-bounded",
        }
    }

    /// Build the concrete connection-selection strategy from parsed params,
    /// validating the parameters this kind requires (ADR 0028). The edge maps a
    /// returned [`StrategyConfigError`] once — it holds no per-strategy logic.
    pub fn build(
        self,
        params: &StrategyParams,
    ) -> Result<Arc<dyn ConnectionStrategy>, StrategyConfigError> {
        match self {
            Self::ConnectToAll => Ok(Arc::new(ConnectToAllCandidates)),
            Self::SeededBounded => {
                let upstream_degree =
                    params
                        .upstream_degree
                        .ok_or(StrategyConfigError::MissingParameter {
                            strategy: self.name(),
                            parameter: "an upstream degree (--upstream-degree)",
                        })?;
                Ok(Arc::new(SeededBoundedSelection::new(
                    params.seed,
                    params.self_id.clone(),
                    upstream_degree,
                )))
            }
        }
    }
}

/// The error returned when a configuration string names no known connection
/// strategy.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("unknown connection strategy '{0}' (expected one of: connect-to-all, seeded-bounded)")]
pub struct UnknownConnectionStrategy(pub String);

impl FromStr for ConnectionStrategyKind {
    type Err = UnknownConnectionStrategy;

    /// Parse a strategy name case-insensitively.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "connect-to-all" => Ok(Self::ConnectToAll),
            "seeded-bounded" => Ok(Self::SeededBounded),
            _ => Err(UnknownConnectionStrategy(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ConnectionStrategyKind;
    use crate::peer::PeerId;
    use crate::strategy_config::{StrategyConfigError, StrategyParams};
    use std::str::FromStr;

    fn params(upstream_degree: Option<usize>) -> StrategyParams {
        StrategyParams {
            self_id: PeerId::from_str("self").expect("valid peer id"),
            seed: 0,
            upstream_degree,
            downstream_degree: None,
        }
    }

    // ADR 0028: connect-to-all needs no params; build succeeds regardless.
    #[test]
    fn connect_to_all_builds_without_params() {
        assert!(ConnectionStrategyKind::ConnectToAll
            .build(&params(None))
            .is_ok());
    }

    // ADR 0028: seeded-bounded validates its required upstream degree in build —
    // missing → typed error, present → Ok.
    #[test]
    fn seeded_bounded_requires_upstream_degree() {
        assert!(matches!(
            ConnectionStrategyKind::SeededBounded.build(&params(None)),
            Err(StrategyConfigError::MissingParameter { .. }),
        ));
        assert!(ConnectionStrategyKind::SeededBounded
            .build(&params(Some(3)))
            .is_ok());
    }

    #[test]
    fn parses_case_insensitively() {
        assert_eq!(
            ConnectionStrategyKind::from_str("connect-to-all").unwrap(),
            ConnectionStrategyKind::ConnectToAll,
        );
        assert_eq!(
            ConnectionStrategyKind::from_str("Seeded-Bounded").unwrap(),
            ConnectionStrategyKind::SeededBounded,
        );
        assert_eq!(
            ConnectionStrategyKind::from_str("SEEDED-BOUNDED").unwrap(),
            ConnectionStrategyKind::SeededBounded,
        );
    }

    #[test]
    fn every_name_round_trips() {
        for kind in [
            ConnectionStrategyKind::ConnectToAll,
            ConnectionStrategyKind::SeededBounded,
        ] {
            assert_eq!(ConnectionStrategyKind::from_str(kind.name()).unwrap(), kind);
        }
    }

    #[test]
    fn unknown_name_is_rejected() {
        assert!(ConnectionStrategyKind::from_str("nope").is_err());
    }

    #[test]
    fn tags_are_unique_per_strategy() {
        assert_ne!(
            ConnectionStrategyKind::ConnectToAll.tag(),
            ConnectionStrategyKind::SeededBounded.tag(),
        );
    }
}

//! The selectable connection-selection strategies, named for configuration.
//!
//! [`ConnectionStrategyKind`] is the config-facing enum: it parses
//! case-insensitively from a readable name (`connect-to-all`, `hash-gated`) and
//! carries a stable, unique byte-string [`tag`](ConnectionStrategyKind::tag) per
//! variant. The edge (CLI/loader) maps a kind plus its parameters to a concrete
//! [`ConnectionStrategy`](super::ConnectionStrategy) instance — the kind itself
//! constructs nothing.

use std::str::FromStr;
use std::sync::Arc;

use super::{ConnectToAllCandidates, ConnectionStrategy, HashGatedConnection};
use crate::strategies::config::{ConnectionParams, StrategyConfigError};

/// A selectable connection-selection strategy, identified by a readable name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionStrategyKind {
    /// The full-mesh policy ([`ConnectToAllCandidates`](super::ConnectToAllCandidates)).
    ConnectToAll,
    /// The verifiable hash-gated policy ([`HashGatedConnection`](super::HashGatedConnection)).
    HashGated,
}

impl ConnectionStrategyKind {
    /// The canonical, lower-case configuration name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ConnectToAll => "connect-to-all",
            Self::HashGated => "hash-gated",
        }
    }

    /// A stable, unique byte-string identifying this strategy.
    #[must_use]
    pub const fn tag(self) -> &'static [u8] {
        match self {
            Self::ConnectToAll => b"pubsub/connection-strategy/connect-to-all",
            Self::HashGated => b"pubsub/connection-strategy/hash-gated",
        }
    }

    /// Build the concrete connection-selection strategy from the connection
    /// seam's params, validating the parameters this kind requires (ADR 0028).
    /// The edge maps a returned [`StrategyConfigError`] once.
    pub fn build(
        self,
        params: &ConnectionParams,
    ) -> Result<Arc<dyn ConnectionStrategy>, StrategyConfigError> {
        match self {
            Self::ConnectToAll => Ok(Arc::new(ConnectToAllCandidates)),
            Self::HashGated => {
                let rf = params.rf.ok_or(StrategyConfigError::MissingParameter {
                    strategy: self.name(),
                    parameter: "a fanout (--rf)",
                })?;
                Ok(Arc::new(HashGatedConnection::new(
                    params.genesis,
                    params.self_id.clone(),
                    rf,
                )))
            }
        }
    }
}

/// The error returned when a configuration string names no known connection
/// strategy.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("unknown connection strategy '{0}' (expected one of: connect-to-all, hash-gated)")]
pub struct UnknownConnectionStrategy(pub String);

impl FromStr for ConnectionStrategyKind {
    type Err = UnknownConnectionStrategy;

    /// Parse a strategy name case-insensitively.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "connect-to-all" => Ok(Self::ConnectToAll),
            "hash-gated" => Ok(Self::HashGated),
            _ => Err(UnknownConnectionStrategy(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ConnectionStrategyKind;
    use crate::peer::PeerId;
    use crate::strategies::config::{ConnectionParams, StrategyConfigError};
    use std::str::FromStr;

    fn params(rf: Option<usize>) -> ConnectionParams {
        ConnectionParams {
            self_id: PeerId::from_str("self").expect("valid peer id"),
            genesis: 0,
            rf,
        }
    }

    // ADR 0028: connect-to-all needs no params; build succeeds regardless.
    #[test]
    fn connect_to_all_builds_without_params() {
        assert!(ConnectionStrategyKind::ConnectToAll
            .build(&params(None))
            .is_ok());
    }

    // ADR 0028: hash-gated validates its required fanout in build.
    #[test]
    fn hash_gated_requires_rf() {
        assert!(matches!(
            ConnectionStrategyKind::HashGated.build(&params(None)),
            Err(StrategyConfigError::MissingParameter { .. }),
        ));
        assert!(ConnectionStrategyKind::HashGated
            .build(&params(Some(8)))
            .is_ok());
    }

    #[test]
    fn parses_case_insensitively() {
        assert_eq!(
            ConnectionStrategyKind::from_str("connect-to-all").unwrap(),
            ConnectionStrategyKind::ConnectToAll,
        );
        assert_eq!(
            ConnectionStrategyKind::from_str("Hash-Gated").unwrap(),
            ConnectionStrategyKind::HashGated,
        );
        assert_eq!(
            ConnectionStrategyKind::from_str("HASH-GATED").unwrap(),
            ConnectionStrategyKind::HashGated,
        );
    }

    #[test]
    fn every_name_round_trips() {
        for kind in [
            ConnectionStrategyKind::ConnectToAll,
            ConnectionStrategyKind::HashGated,
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
            ConnectionStrategyKind::HashGated.tag(),
        );
    }
}

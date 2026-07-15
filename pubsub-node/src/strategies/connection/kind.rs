//! The selectable connection-selection strategies, named for configuration.
//!
//! [`ConnectionStrategyKind`] is the config-facing enum: it parses
//! case-insensitively from a readable name (`connect-to-all`, `hash-gated`).
//! The edge (CLI/loader) maps a kind plus its parameters to a concrete
//! [`ConnectionStrategy`](super::ConnectionStrategy) instance — the kind itself
//! constructs nothing.

use std::str::FromStr;
use std::sync::Arc;

use super::{ConnectToAllCandidates, ConnectionStrategy, HashGatedConnection};
use crate::strategies::config::{
    require_target_degree, validate_bucket_count, ConnectionParams, StrategyConfigError,
};

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
                let target_degree = require_target_degree(self.name(), params.target_degree)?;
                let bucket_override = validate_bucket_count(self.name(), params.bucket_count)?;
                Ok(Arc::new(
                    HashGatedConnection::new(params.self_id.clone(), target_degree)
                        .with_bucket_override(bucket_override)
                        .for_kind(params.kind),
                ))
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
    use crate::connection_state::LinkKind;
    use crate::peer::PeerId;
    use crate::strategies::config::{ConnectionParams, StrategyConfigError};
    use std::str::FromStr;

    fn params(target_degree: Option<usize>) -> ConnectionParams {
        ConnectionParams {
            self_id: PeerId::from_str("self").expect("valid peer id"),
            kind: LinkKind::Relay,
            target_degree,
            bucket_count: None,
        }
    }

    // ADR 0028: connect-to-all needs no params; build succeeds regardless.
    #[test]
    fn connect_to_all_builds_without_params() {
        assert!(ConnectionStrategyKind::ConnectToAll
            .build(&params(None))
            .is_ok());
    }

    // ADR 0028: hash-gated validates its required target degree in build.
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

    // A target degree of 0 degenerates the seam (connect-to-all) and is rejected
    // at build rather than booting into an asymmetric topology.
    #[test]
    fn hash_gated_rejects_zero_target_degree() {
        assert!(matches!(
            ConnectionStrategyKind::HashGated.build(&params(Some(0))),
            Err(StrategyConfigError::InvalidParameter { .. }),
        ));
    }

    // A pinned bucket count of 0 would divide by zero in the predicate; reject it.
    #[test]
    fn hash_gated_rejects_zero_bucket_count() {
        let mut p = params(Some(8));
        p.bucket_count = Some(0);
        assert!(matches!(
            ConnectionStrategyKind::HashGated.build(&p),
            Err(StrategyConfigError::InvalidParameter { .. }),
        ));
        // A pinned count ≥ 1 builds.
        p.bucket_count = Some(4);
        assert!(ConnectionStrategyKind::HashGated.build(&p).is_ok());
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
}

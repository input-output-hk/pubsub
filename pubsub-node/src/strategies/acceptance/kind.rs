//! The selectable inbound-acceptance strategies, named for configuration.
//!
//! The inbound mirror of `connection::ConnectionStrategyKind`: a config-facing
//! enum parsed case-insensitively from a readable name. The edge maps a kind
//! plus its parameters to a concrete
//! [`ConnectionAcceptanceStrategy`](super::ConnectionAcceptanceStrategy).

use std::str::FromStr;
use std::sync::Arc;

use super::{AcceptFromAllCandidates, ConnectionAcceptanceStrategy, VerifiableBoundedAcceptance};
use crate::strategies::config::{
    require_target_degree, validate_bucket_count, AcceptanceParams, StrategyConfigError,
};

/// A selectable inbound-acceptance strategy, identified by a readable name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptanceStrategyKind {
    /// Accept every membership-valid request ([`AcceptFromAllCandidates`](super::AcceptFromAllCandidates)).
    AcceptFromAll,
    /// Verifiable, bucketed, bounded acceptance ([`VerifiableBoundedAcceptance`](super::VerifiableBoundedAcceptance)).
    VerifiableBounded,
}

impl AcceptanceStrategyKind {
    /// The canonical, lower-case configuration name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::AcceptFromAll => "accept-from-all",
            Self::VerifiableBounded => "verifiable-bounded",
        }
    }

    /// Build the concrete inbound-acceptance strategy from the acceptance seam's
    /// params, validating the parameters this kind requires (ADR 0028). The edge
    /// maps a returned [`StrategyConfigError`] once.
    pub fn build(
        self,
        params: &AcceptanceParams,
    ) -> Result<Arc<dyn ConnectionAcceptanceStrategy>, StrategyConfigError> {
        match self {
            Self::AcceptFromAll => Ok(Arc::new(AcceptFromAllCandidates)),
            Self::VerifiableBounded => {
                let target_degree = require_target_degree(self.name(), params.target_degree)?;
                let bucket_override = validate_bucket_count(self.name(), params.bucket_count)?;
                Ok(Arc::new(
                    VerifiableBoundedAcceptance::new(
                        params.genesis,
                        params.self_id.clone(),
                        target_degree,
                        params.cap_buffer,
                    )
                    .with_bucket_override(bucket_override),
                ))
            }
        }
    }
}

/// The error returned when a configuration string names no known acceptance
/// strategy.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("unknown acceptance strategy '{0}' (expected one of: accept-from-all, verifiable-bounded)")]
pub struct UnknownAcceptanceStrategy(pub String);

impl FromStr for AcceptanceStrategyKind {
    type Err = UnknownAcceptanceStrategy;

    /// Parse a strategy name case-insensitively.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "accept-from-all" => Ok(Self::AcceptFromAll),
            "verifiable-bounded" => Ok(Self::VerifiableBounded),
            _ => Err(UnknownAcceptanceStrategy(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AcceptanceStrategyKind;
    use crate::peer::PeerId;
    use crate::strategies::config::{AcceptanceParams, StrategyConfigError};
    use std::str::FromStr;

    fn params(target_degree: Option<usize>) -> AcceptanceParams {
        AcceptanceParams {
            self_id: PeerId::from_str("self").expect("valid peer id"),
            genesis: 0,
            target_degree,
            bucket_count: None,
            cap_buffer: 3,
        }
    }

    // ADR 0028: accept-from-all needs no params; build succeeds regardless.
    #[test]
    fn accept_from_all_builds_without_params() {
        assert!(AcceptanceStrategyKind::AcceptFromAll
            .build(&params(None))
            .is_ok());
    }

    // ADR 0028: verifiable-bounded validates its required target degree in build.
    #[test]
    fn verifiable_bounded_requires_rf() {
        assert!(matches!(
            AcceptanceStrategyKind::VerifiableBounded.build(&params(None)),
            Err(StrategyConfigError::MissingParameter { .. }),
        ));
        assert!(AcceptanceStrategyKind::VerifiableBounded
            .build(&params(Some(8)))
            .is_ok());
    }

    // A target degree of 0 makes the accept cap 0 (reject everything); reject at
    // build so the two seams cannot degenerate in opposite directions.
    #[test]
    fn verifiable_bounded_rejects_zero_target_degree() {
        assert!(matches!(
            AcceptanceStrategyKind::VerifiableBounded.build(&params(Some(0))),
            Err(StrategyConfigError::InvalidParameter { .. }),
        ));
    }

    // A pinned bucket count of 0 would divide by zero in the predicate; reject it.
    #[test]
    fn verifiable_bounded_rejects_zero_bucket_count() {
        let mut p = params(Some(8));
        p.bucket_count = Some(0);
        assert!(matches!(
            AcceptanceStrategyKind::VerifiableBounded.build(&p),
            Err(StrategyConfigError::InvalidParameter { .. }),
        ));
        p.bucket_count = Some(4);
        assert!(AcceptanceStrategyKind::VerifiableBounded.build(&p).is_ok());
    }

    #[test]
    fn parses_case_insensitively() {
        assert_eq!(
            AcceptanceStrategyKind::from_str("Accept-From-All").unwrap(),
            AcceptanceStrategyKind::AcceptFromAll,
        );
        assert_eq!(
            AcceptanceStrategyKind::from_str("VERIFIABLE-BOUNDED").unwrap(),
            AcceptanceStrategyKind::VerifiableBounded,
        );
    }

    #[test]
    fn every_name_round_trips() {
        for kind in [
            AcceptanceStrategyKind::AcceptFromAll,
            AcceptanceStrategyKind::VerifiableBounded,
        ] {
            assert_eq!(AcceptanceStrategyKind::from_str(kind.name()).unwrap(), kind);
        }
    }

    #[test]
    fn unknown_name_is_rejected() {
        assert!(AcceptanceStrategyKind::from_str("nope").is_err());
    }
}

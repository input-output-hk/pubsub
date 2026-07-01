//! The selectable inbound-acceptance strategies, named for configuration.
//!
//! The inbound mirror of `connection::ConnectionStrategyKind`: a config-facing
//! enum parsed case-insensitively from a readable name, with a stable, unique
//! byte-string [`tag`](AcceptanceStrategyKind::tag) per variant. The edge maps a
//! kind plus its parameters to a concrete
//! [`ConnectionAcceptanceStrategy`](super::ConnectionAcceptanceStrategy).

use std::str::FromStr;
use std::sync::Arc;

use super::{AcceptFromAllCandidates, BoundedAcceptance, ConnectionAcceptanceStrategy};
use crate::strategies::config::{AcceptanceParams, StrategyConfigError};

/// A selectable inbound-acceptance strategy, identified by a readable name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptanceStrategyKind {
    /// Accept every membership-valid request ([`AcceptFromAllCandidates`](super::AcceptFromAllCandidates)).
    AcceptFromAll,
    /// Bound the inbound degree per topic ([`BoundedAcceptance`](super::BoundedAcceptance)).
    Bounded,
}

impl AcceptanceStrategyKind {
    /// The canonical, lower-case configuration name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::AcceptFromAll => "accept-from-all",
            Self::Bounded => "bounded",
        }
    }

    /// A stable, unique byte-string identifying this strategy.
    #[must_use]
    pub const fn tag(self) -> &'static [u8] {
        match self {
            Self::AcceptFromAll => b"pubsub/acceptance-strategy/accept-from-all",
            Self::Bounded => b"pubsub/acceptance-strategy/bounded",
        }
    }

    /// Build the concrete inbound-acceptance strategy from the acceptance seam's
    /// params, validating the parameters this kind requires (ADR 0028). The edge
    /// maps a returned [`StrategyConfigError`] once — it holds no per-strategy
    /// logic.
    pub fn build(
        self,
        params: &AcceptanceParams,
    ) -> Result<Arc<dyn ConnectionAcceptanceStrategy>, StrategyConfigError> {
        match self {
            Self::AcceptFromAll => Ok(Arc::new(AcceptFromAllCandidates)),
            Self::Bounded => {
                let downstream_degree =
                    params
                        .downstream_degree
                        .ok_or(StrategyConfigError::MissingParameter {
                            strategy: self.name(),
                            parameter: "a downstream degree (--downstream-degree)",
                        })?;
                Ok(Arc::new(BoundedAcceptance::new(downstream_degree)))
            }
        }
    }
}

/// The error returned when a configuration string names no known acceptance
/// strategy.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("unknown acceptance strategy '{0}' (expected one of: accept-from-all, bounded)")]
pub struct UnknownAcceptanceStrategy(pub String);

impl FromStr for AcceptanceStrategyKind {
    type Err = UnknownAcceptanceStrategy;

    /// Parse a strategy name case-insensitively.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "accept-from-all" => Ok(Self::AcceptFromAll),
            "bounded" => Ok(Self::Bounded),
            _ => Err(UnknownAcceptanceStrategy(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AcceptanceStrategyKind;
    use crate::strategies::config::{AcceptanceParams, StrategyConfigError};
    use std::str::FromStr;

    fn params(downstream_degree: Option<usize>) -> AcceptanceParams {
        AcceptanceParams { downstream_degree }
    }

    // ADR 0028: accept-from-all needs no params; build succeeds regardless.
    #[test]
    fn accept_from_all_builds_without_params() {
        assert!(AcceptanceStrategyKind::AcceptFromAll
            .build(&params(None))
            .is_ok());
    }

    // ADR 0028: bounded validates its required downstream degree in build.
    #[test]
    fn bounded_requires_downstream_degree() {
        assert!(matches!(
            AcceptanceStrategyKind::Bounded.build(&params(None)),
            Err(StrategyConfigError::MissingParameter { .. }),
        ));
        assert!(AcceptanceStrategyKind::Bounded
            .build(&params(Some(2)))
            .is_ok());
    }

    #[test]
    fn parses_case_insensitively() {
        assert_eq!(
            AcceptanceStrategyKind::from_str("Accept-From-All").unwrap(),
            AcceptanceStrategyKind::AcceptFromAll,
        );
        assert_eq!(
            AcceptanceStrategyKind::from_str("BOUNDED").unwrap(),
            AcceptanceStrategyKind::Bounded,
        );
    }

    #[test]
    fn every_name_round_trips() {
        for kind in [
            AcceptanceStrategyKind::AcceptFromAll,
            AcceptanceStrategyKind::Bounded,
        ] {
            assert_eq!(AcceptanceStrategyKind::from_str(kind.name()).unwrap(), kind);
        }
    }

    #[test]
    fn unknown_name_is_rejected() {
        assert!(AcceptanceStrategyKind::from_str("nope").is_err());
    }

    #[test]
    fn tags_are_unique_per_strategy() {
        assert_ne!(
            AcceptanceStrategyKind::AcceptFromAll.tag(),
            AcceptanceStrategyKind::Bounded.tag(),
        );
    }
}

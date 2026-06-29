//! The selectable inbound-acceptance strategies, named for configuration.
//!
//! The inbound mirror of `connection::ConnectionStrategyKind`: a config-facing
//! enum parsed case-insensitively from a readable name, with a stable, unique
//! byte-string [`tag`](AcceptanceStrategyKind::tag) per variant. The edge maps a
//! kind plus its parameters to a concrete
//! [`ConnectionAcceptanceStrategy`](super::ConnectionAcceptanceStrategy).

use std::str::FromStr;

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
    use std::str::FromStr;

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

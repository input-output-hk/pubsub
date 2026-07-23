//! The selectable fan-out strategies, named for configuration.
//!
//! The fan-out sibling of the connection/acceptance kind enums: a config-facing
//! enum parsed case-insensitively from a readable name. Fan-out strategies take
//! no parameters, so the kind builds the instance directly (no two-phase step).

use std::str::FromStr;
use std::sync::Arc;

use super::{FanoutStrategy, ForwardToAll, ForwardToRelays};

/// A selectable fan-out strategy, identified by a readable name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FanoutStrategyKind {
    /// Relay downstream always; `Active` publisher links only for
    /// locally-published messages ([`ForwardToRelays`] — the default, M3).
    ForwardToRelays,
    /// Every held message over relay downstream ∪ `Active` publisher links,
    /// any origin ([`ForwardToAll`] — M5's send side).
    ForwardToAll,
}

impl FanoutStrategyKind {
    /// The canonical, lower-case configuration name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ForwardToRelays => "forward-to-relays",
            Self::ForwardToAll => "forward-to-all",
        }
    }

    /// Build the concrete fan-out strategy (parameterless).
    #[must_use]
    pub fn build(self) -> Arc<dyn FanoutStrategy> {
        match self {
            Self::ForwardToRelays => Arc::new(ForwardToRelays),
            Self::ForwardToAll => Arc::new(ForwardToAll),
        }
    }
}

/// The error returned when a configuration string names no known fan-out
/// strategy.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("unknown fanout strategy '{0}' (expected one of: forward-to-relays, forward-to-all)")]
pub struct UnknownFanoutStrategy(pub String);

impl FromStr for FanoutStrategyKind {
    type Err = UnknownFanoutStrategy;

    /// Parse a strategy name case-insensitively.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "forward-to-relays" => Ok(Self::ForwardToRelays),
            "forward-to-all" => Ok(Self::ForwardToAll),
            _ => Err(UnknownFanoutStrategy(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FanoutStrategyKind;
    use std::str::FromStr;

    #[test]
    fn every_name_round_trips() {
        for kind in [
            FanoutStrategyKind::ForwardToRelays,
            FanoutStrategyKind::ForwardToAll,
        ] {
            assert_eq!(FanoutStrategyKind::from_str(kind.name()).unwrap(), kind);
        }
    }

    #[test]
    fn parses_case_insensitively_and_rejects_unknown() {
        assert_eq!(
            FanoutStrategyKind::from_str("Forward-To-Relays").unwrap(),
            FanoutStrategyKind::ForwardToRelays,
        );
        assert!(FanoutStrategyKind::from_str("nope").is_err());
    }
}

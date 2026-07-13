//! The selectable fan-out strategies, named for configuration — the
//! dissemination-model knob (ADR 0034).

use std::str::FromStr;
use std::sync::Arc;

use super::{FanoutStrategy, ForwardToAll, RoleScopedFanout};

/// A selectable fan-out strategy, identified by a readable name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FanoutStrategyKind {
    /// Union: relay downstream for every message, plus the initiation targets
    /// for a local origin ([`ForwardToAll`](super::ForwardToAll)) — the default.
    ForwardToAll,
    /// The strict M3 partition: local publications over initiation links only,
    /// relayed traffic over relay links only
    /// ([`RoleScopedFanout`](super::RoleScopedFanout)).
    RoleScoped,
}

impl FanoutStrategyKind {
    /// The canonical, lower-case configuration name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ForwardToAll => "forward-to-all",
            Self::RoleScoped => "role-scoped",
        }
    }

    /// Build the concrete fan-out strategy (no parameters — the policies are
    /// pure cell selections).
    #[must_use]
    pub fn build(self) -> Arc<dyn FanoutStrategy> {
        match self {
            Self::ForwardToAll => Arc::new(ForwardToAll),
            Self::RoleScoped => Arc::new(RoleScopedFanout),
        }
    }
}

/// The error returned when a configuration string names no known fan-out
/// strategy.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("unknown fanout strategy '{0}' (expected one of: forward-to-all, role-scoped)")]
pub struct UnknownFanoutStrategy(pub String);

impl FromStr for FanoutStrategyKind {
    type Err = UnknownFanoutStrategy;

    /// Parse a strategy name case-insensitively.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "forward-to-all" => Ok(Self::ForwardToAll),
            "role-scoped" => Ok(Self::RoleScoped),
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
            FanoutStrategyKind::ForwardToAll,
            FanoutStrategyKind::RoleScoped,
        ] {
            assert_eq!(FanoutStrategyKind::from_str(kind.name()).unwrap(), kind);
        }
    }

    #[test]
    fn unknown_name_is_rejected() {
        assert!(FanoutStrategyKind::from_str("nope").is_err());
    }
}

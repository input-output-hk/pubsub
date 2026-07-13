//! The selectable link-selection strategies, named for configuration.
//!
//! One config-facing enum serves **both** role slots (ADR 0034): the edge maps
//! a kind plus the slot's [`SelectionParams`] to a concrete
//! [`LinkSelectionStrategy`](super::LinkSelectionStrategy); the params carry
//! the slot's role, which picks the hash domain and the operator-facing flag
//! names in errors.

use std::str::FromStr;
use std::sync::Arc;

use super::{ConnectToAllCandidates, HashGatedSelection, LinkSelectionStrategy, NoLinks};
use crate::strategies::config::{
    require_degree, validate_bucket_count, SelectionParams, StrategyConfigError,
};

/// A selectable link-selection strategy, identified by a readable name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkSelectionKind {
    /// Select nothing ([`NoLinks`](super::NoLinks)) — the publish-slot default;
    /// on the relay slot, an accept-only node.
    None,
    /// The full-mesh policy ([`ConnectToAllCandidates`](super::ConnectToAllCandidates))
    /// — the relay-slot default.
    ConnectToAll,
    /// The verifiable hash-gated policy ([`HashGatedSelection`](super::HashGatedSelection))
    /// under the slot's role domain.
    HashGated,
}

impl LinkSelectionKind {
    /// The canonical, lower-case configuration name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ConnectToAll => "connect-to-all",
            Self::HashGated => "hash-gated",
        }
    }

    /// Build the concrete selection strategy from the slot's params, validating
    /// the parameters this kind requires (ADR 0028/0034). The edge maps a
    /// returned [`StrategyConfigError`] once.
    pub fn build(
        self,
        params: &SelectionParams,
    ) -> Result<Arc<dyn LinkSelectionStrategy>, StrategyConfigError> {
        match self {
            Self::None => Ok(Arc::new(NoLinks)),
            Self::ConnectToAll => Ok(Arc::new(ConnectToAllCandidates)),
            Self::HashGated => {
                let degree = require_degree(
                    self.name(),
                    params.degree,
                    params.missing_degree(),
                    params.invalid_degree(),
                )?;
                let bucket_override = validate_bucket_count(self.name(), params.bucket_count)?;
                Ok(Arc::new(
                    HashGatedSelection::new(params.role, params.self_id.clone(), degree)
                        .with_bucket_override(bucket_override)
                        .with_symmetric(params.symmetric),
                ))
            }
        }
    }
}

/// The error returned when a configuration string names no known selection
/// strategy.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error(
    "unknown link-selection strategy '{0}' (expected one of: none, connect-to-all, hash-gated)"
)]
pub struct UnknownLinkSelection(pub String);

impl FromStr for LinkSelectionKind {
    type Err = UnknownLinkSelection;

    /// Parse a strategy name case-insensitively.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "connect-to-all" => Ok(Self::ConnectToAll),
            "hash-gated" => Ok(Self::HashGated),
            _ => Err(UnknownLinkSelection(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LinkSelectionKind;
    use crate::connection_state::LinkRole;
    use crate::peer::PeerId;
    use crate::strategies::config::{SelectionParams, StrategyConfigError};
    use std::str::FromStr;

    fn params(role: LinkRole, degree: Option<usize>) -> SelectionParams {
        SelectionParams {
            self_id: PeerId::from_str("self").expect("valid peer id"),
            role,
            degree,
            bucket_count: None,
            symmetric: false,
        }
    }

    // ADR 0028: none / connect-to-all need no params; build succeeds regardless.
    #[test]
    fn parameterless_kinds_build_without_degree() {
        for kind in [LinkSelectionKind::None, LinkSelectionKind::ConnectToAll] {
            assert!(kind.build(&params(LinkRole::Relay, None)).is_ok());
        }
    }

    // ADR 0028/0034: hash-gated validates the slot's degree in build; the error
    // names the slot's flag.
    #[test]
    fn hash_gated_requires_the_slot_degree() {
        let Err(err) = LinkSelectionKind::HashGated.build(&params(LinkRole::Publisher, None))
        else {
            panic!("missing degree must fail")
        };
        assert!(
            matches!(err, StrategyConfigError::MissingParameter { .. }),
            "missing-parameter error"
        );
        assert!(
            err.to_string().contains("--publish-degree"),
            "the error names the publish slot's flag: {err}",
        );
        assert!(LinkSelectionKind::HashGated
            .build(&params(LinkRole::Relay, Some(8)))
            .is_ok());
    }

    // A degree of 0 degenerates the seam; rejected at build.
    #[test]
    fn hash_gated_rejects_zero_degree() {
        assert!(matches!(
            LinkSelectionKind::HashGated.build(&params(LinkRole::Relay, Some(0))),
            Err(StrategyConfigError::InvalidParameter { .. }),
        ));
    }

    #[test]
    fn every_name_round_trips() {
        for kind in [
            LinkSelectionKind::None,
            LinkSelectionKind::ConnectToAll,
            LinkSelectionKind::HashGated,
        ] {
            assert_eq!(LinkSelectionKind::from_str(kind.name()).unwrap(), kind);
        }
    }

    #[test]
    fn unknown_name_is_rejected() {
        assert!(LinkSelectionKind::from_str("nope").is_err());
    }
}

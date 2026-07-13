//! The selectable inbound-acceptance strategies, named for configuration.
//!
//! The inbound mirror of `connection::ConnectionStrategyKind`: a config-facing
//! enum parsed case-insensitively from a readable name. The edge maps a kind
//! plus its parameters to a concrete
//! [`ConnectionAcceptanceStrategy`](super::ConnectionAcceptanceStrategy).

use std::str::FromStr;
use std::sync::Arc;

use super::{
    AcceptFromAllCandidates, BoundedAcceptance, ConnectionAcceptanceStrategy, HashGatedAcceptance,
    HashGatedBoundedAcceptance,
};
use crate::connection_state::LinkRole;
use crate::strategies::config::{
    require_degree, require_relay_degree, validate_bucket_count, AcceptanceParams,
    PublishAcceptanceParams, StrategyConfigError,
};

/// A selectable inbound-acceptance strategy, identified by a readable name —
/// the four one-dimensional baselines of the empirical approach (ADR 0031):
/// neither check, cap only, gate only, both.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptanceStrategyKind {
    /// Accept every membership-valid request ([`AcceptFromAllCandidates`](super::AcceptFromAllCandidates)).
    AcceptFromAll,
    /// Cap-only acceptance, no hash gate ([`BoundedAcceptance`](super::BoundedAcceptance)).
    Bounded,
    /// Hash-gate-only acceptance, no cap ([`HashGatedAcceptance`](super::HashGatedAcceptance)).
    HashGated,
    /// Verifiable, bucketed, bounded acceptance — gate **and** cap
    /// ([`HashGatedBoundedAcceptance`](super::HashGatedBoundedAcceptance)).
    HashGatedBounded,
}

impl AcceptanceStrategyKind {
    /// The canonical, lower-case configuration name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::AcceptFromAll => "accept-from-all",
            Self::Bounded => "bounded",
            Self::HashGated => "hash-gated",
            Self::HashGatedBounded => "hash-gated-bounded",
        }
    }

    /// Build the concrete **relay-slot** inbound-acceptance strategy from the
    /// acceptance seam's params, validating the parameters this kind requires
    /// (ADR 0028). The edge maps a returned [`StrategyConfigError`] once.
    pub fn build(
        self,
        params: &AcceptanceParams,
    ) -> Result<Arc<dyn ConnectionAcceptanceStrategy>, StrategyConfigError> {
        match self {
            Self::AcceptFromAll => Ok(Arc::new(AcceptFromAllCandidates)),
            Self::Bounded => {
                let relay_degree = require_relay_degree(self.name(), params.relay_degree)?;
                Ok(Arc::new(BoundedAcceptance::new(
                    relay_degree,
                    params.cap_buffer,
                )))
            }
            Self::HashGated => {
                let relay_degree = require_relay_degree(self.name(), params.relay_degree)?;
                let bucket_override = validate_bucket_count(self.name(), params.bucket_count)?;
                Ok(Arc::new(
                    HashGatedAcceptance::new(params.self_id.clone(), relay_degree)
                        .with_bucket_override(bucket_override),
                ))
            }
            Self::HashGatedBounded => {
                let relay_degree = require_relay_degree(self.name(), params.relay_degree)?;
                let bucket_override = validate_bucket_count(self.name(), params.bucket_count)?;
                Ok(Arc::new(
                    HashGatedBoundedAcceptance::new(
                        params.self_id.clone(),
                        relay_degree,
                        params.cap_buffer,
                    )
                    .with_bucket_override(bucket_override),
                ))
            }
        }
    }

    /// Build the concrete inbound-acceptance strategy for a **non-relay slot**
    /// (feature 015, ADR 0033): the same four kinds, instantiated with that
    /// role's degree parameters and retargeted so the prelude scan, the cap,
    /// and the predicate domain are role-scoped. Used by the publish slot.
    pub fn build_for_role(
        self,
        role: LinkRole,
        params: &PublishAcceptanceParams,
    ) -> Result<Arc<dyn ConnectionAcceptanceStrategy>, StrategyConfigError> {
        match self {
            Self::AcceptFromAll => Ok(Arc::new(AcceptFromAllCandidates)),
            Self::Bounded => {
                let degree = require_degree(
                    self.name(),
                    params.publish_degree,
                    "a publish degree (--publish-degree)",
                    "the publish degree (--publish-degree)",
                )?;
                Ok(Arc::new(
                    BoundedAcceptance::new(degree, params.cap_buffer).for_role(role),
                ))
            }
            Self::HashGated => {
                let degree = require_degree(
                    self.name(),
                    params.publish_degree,
                    "a publish degree (--publish-degree)",
                    "the publish degree (--publish-degree)",
                )?;
                let bucket_override = validate_bucket_count(self.name(), params.bucket_count)?;
                Ok(Arc::new(
                    HashGatedAcceptance::new(params.self_id.clone(), degree)
                        .with_bucket_override(bucket_override)
                        .for_role(role),
                ))
            }
            Self::HashGatedBounded => {
                let degree = require_degree(
                    self.name(),
                    params.publish_degree,
                    "a publish degree (--publish-degree)",
                    "the publish degree (--publish-degree)",
                )?;
                let bucket_override = validate_bucket_count(self.name(), params.bucket_count)?;
                Ok(Arc::new(
                    HashGatedBoundedAcceptance::new(
                        params.self_id.clone(),
                        degree,
                        params.cap_buffer,
                    )
                    .with_bucket_override(bucket_override)
                    .for_role(role),
                ))
            }
        }
    }
}

/// The error returned when a configuration string names no known acceptance
/// strategy.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("unknown acceptance strategy '{0}' (expected one of: accept-from-all, bounded, hash-gated, hash-gated-bounded)")]
pub struct UnknownAcceptanceStrategy(pub String);

impl FromStr for AcceptanceStrategyKind {
    type Err = UnknownAcceptanceStrategy;

    /// Parse a strategy name case-insensitively.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "accept-from-all" => Ok(Self::AcceptFromAll),
            "bounded" => Ok(Self::Bounded),
            "hash-gated" => Ok(Self::HashGated),
            "hash-gated-bounded" => Ok(Self::HashGatedBounded),
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

    fn params(relay_degree: Option<usize>) -> AcceptanceParams {
        AcceptanceParams {
            self_id: PeerId::from_str("self").expect("valid peer id"),
            relay_degree,
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

    // ADR 0028: every parameterised kind validates its required relay degree
    // in build; each builds cleanly once it is supplied.
    #[test]
    fn parameterised_kinds_require_a_relay_degree() {
        for kind in [
            AcceptanceStrategyKind::Bounded,
            AcceptanceStrategyKind::HashGated,
            AcceptanceStrategyKind::HashGatedBounded,
        ] {
            assert!(
                matches!(
                    kind.build(&params(None)),
                    Err(StrategyConfigError::MissingParameter { .. }),
                ),
                "{} must require a relay degree",
                kind.name(),
            );
            assert!(
                kind.build(&params(Some(8))).is_ok(),
                "{} must build with a relay degree",
                kind.name(),
            );
        }
    }

    // A relay degree of 0 makes the accept cap 0 (reject everything); reject at
    // build so the two seams cannot degenerate in opposite directions.
    #[test]
    fn parameterised_kinds_reject_zero_relay_degree() {
        for kind in [
            AcceptanceStrategyKind::Bounded,
            AcceptanceStrategyKind::HashGated,
            AcceptanceStrategyKind::HashGatedBounded,
        ] {
            assert!(
                matches!(
                    kind.build(&params(Some(0))),
                    Err(StrategyConfigError::InvalidParameter { .. }),
                ),
                "{} must reject a zero relay degree",
                kind.name(),
            );
        }
    }

    // A pinned bucket count of 0 would divide by zero in the predicate; reject it.
    #[test]
    fn hash_gated_bounded_rejects_zero_bucket_count() {
        let mut p = params(Some(8));
        p.bucket_count = Some(0);
        assert!(matches!(
            AcceptanceStrategyKind::HashGatedBounded.build(&p),
            Err(StrategyConfigError::InvalidParameter { .. }),
        ));
        p.bucket_count = Some(4);
        assert!(AcceptanceStrategyKind::HashGatedBounded.build(&p).is_ok());
    }

    #[test]
    fn parses_case_insensitively() {
        assert_eq!(
            AcceptanceStrategyKind::from_str("Accept-From-All").unwrap(),
            AcceptanceStrategyKind::AcceptFromAll,
        );
        assert_eq!(
            AcceptanceStrategyKind::from_str("HASH-GATED-BOUNDED").unwrap(),
            AcceptanceStrategyKind::HashGatedBounded,
        );
    }

    #[test]
    fn every_name_round_trips() {
        for kind in [
            AcceptanceStrategyKind::AcceptFromAll,
            AcceptanceStrategyKind::Bounded,
            AcceptanceStrategyKind::HashGated,
            AcceptanceStrategyKind::HashGatedBounded,
        ] {
            assert_eq!(AcceptanceStrategyKind::from_str(kind.name()).unwrap(), kind);
        }
    }

    #[test]
    fn unknown_name_is_rejected() {
        assert!(AcceptanceStrategyKind::from_str("nope").is_err());
    }
}

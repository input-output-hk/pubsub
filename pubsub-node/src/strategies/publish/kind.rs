//! The selectable publish-target strategies, named for configuration.
//!
//! The publish mirror of `connection::ConnectionStrategyKind`: a config-facing
//! enum parsed case-insensitively from a readable name. The edge maps a kind
//! plus its parameters to a concrete [`PublishStrategy`](super::PublishStrategy).

use std::str::FromStr;
use std::sync::Arc;

use super::{HashGatedPublish, NoPublishLinks, PublishStrategy};
use crate::strategies::config::{
    require_degree, validate_bucket_count, PublishParams, StrategyConfigError,
};

/// A selectable publish-target strategy, identified by a readable name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublishStrategyKind {
    /// No publishing links ([`NoPublishLinks`](super::NoPublishLinks)) — the default.
    None,
    /// The verifiable hash-gated policy ([`HashGatedPublish`](super::HashGatedPublish)).
    HashGated,
}

impl PublishStrategyKind {
    /// The canonical, lower-case configuration name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::HashGated => "hash-gated",
        }
    }

    /// Build the concrete publish-target strategy from the publish seam's
    /// params, validating the parameters this kind requires (ADR 0028/0033).
    /// The edge maps a returned [`StrategyConfigError`] once.
    pub fn build(
        self,
        params: &PublishParams,
    ) -> Result<Arc<dyn PublishStrategy>, StrategyConfigError> {
        match self {
            Self::None => Ok(Arc::new(NoPublishLinks)),
            Self::HashGated => {
                let publish_degree = require_degree(
                    self.name(),
                    params.publish_degree,
                    "a publish degree (--publish-degree)",
                    "the publish degree (--publish-degree)",
                )?;
                let relay_degree = require_degree(
                    self.name(),
                    params.relay_degree,
                    "a relay degree (--relay-degree) for the trigger",
                    "the relay degree (--relay-degree)",
                )?;
                let bucket_override = validate_bucket_count(self.name(), params.bucket_count)?;
                Ok(Arc::new(
                    HashGatedPublish::new(params.self_id.clone(), publish_degree, relay_degree)
                        .with_bucket_override(bucket_override),
                ))
            }
        }
    }
}

/// The error returned when a configuration string names no known publish
/// strategy.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("unknown publish strategy '{0}' (expected one of: none, hash-gated)")]
pub struct UnknownPublishStrategy(pub String);

impl FromStr for PublishStrategyKind {
    type Err = UnknownPublishStrategy;

    /// Parse a strategy name case-insensitively.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "hash-gated" => Ok(Self::HashGated),
            _ => Err(UnknownPublishStrategy(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PublishStrategyKind;
    use crate::peer::PeerId;
    use crate::strategies::config::{PublishParams, StrategyConfigError};
    use std::str::FromStr;

    fn params(publish_degree: Option<usize>, relay_degree: Option<usize>) -> PublishParams {
        PublishParams {
            self_id: PeerId::from_str("self").expect("valid peer id"),
            publish_degree,
            relay_degree,
            bucket_count: None,
        }
    }

    // ADR 0028: none needs no params; build succeeds regardless.
    #[test]
    fn none_builds_without_params() {
        assert!(PublishStrategyKind::None.build(&params(None, None)).is_ok());
    }

    // ADR 0028/0033: hash-gated validates both degrees in build.
    #[test]
    fn hash_gated_requires_both_degrees() {
        assert!(matches!(
            PublishStrategyKind::HashGated.build(&params(None, Some(8))),
            Err(StrategyConfigError::MissingParameter { .. }),
        ));
        assert!(matches!(
            PublishStrategyKind::HashGated.build(&params(Some(3), None)),
            Err(StrategyConfigError::MissingParameter { .. }),
        ));
        assert!(PublishStrategyKind::HashGated
            .build(&params(Some(3), Some(8)))
            .is_ok());
    }

    #[test]
    fn every_name_round_trips() {
        for kind in [PublishStrategyKind::None, PublishStrategyKind::HashGated] {
            assert_eq!(PublishStrategyKind::from_str(kind.name()).unwrap(), kind);
        }
    }

    #[test]
    fn unknown_name_is_rejected() {
        assert!(PublishStrategyKind::from_str("nope").is_err());
    }
}

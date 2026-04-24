use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::PubSubError;
use crate::message::{PublisherId, TopicId};

/// Topic configuration as stored in the on-chain Topic Registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicConfig {
    /// Unique topic identifier
    pub topic_id: TopicId,

    /// Human-readable name (e.g., "ops/emergency/critical")
    pub name: String,

    /// Optional description
    pub description: Option<String>,

    /// Public keys authorized to publish. Empty = open topic (anyone can publish).
    pub authorized_publishers: Vec<PublisherId>,

    /// How long messages should be retained in the hot cache
    pub retention_period: Duration,

    /// Phase 1: ignored. Reserved for the future clique-DHT persistence layer (D2 Ch.4),
    /// where replication servers cache messages for topics proportionally.
    /// Relay nodes cache all messages in the hot cache regardless of this value.
    pub replication_factor: u32,
}

impl TopicConfig {
    /// Validated constructor. Enforces retention_period > 0 (messages with no
    /// retention window would be immediately evictable, which is nonsensical).
    pub fn try_new(
        topic_id: TopicId,
        name: String,
        description: Option<String>,
        authorized_publishers: Vec<PublisherId>,
        retention_period: Duration,
        replication_factor: u32,
    ) -> Result<Self, PubSubError> {
        if retention_period.is_zero() {
            return Err(PubSubError::InvalidConfig(
                "retention_period must be > 0".into(),
            ));
        }
        Ok(Self {
            topic_id,
            name,
            description,
            authorized_publishers,
            retention_period,
            replication_factor,
        })
    }

    /// Whether this topic restricts who can publish (moderated) or is open
    pub fn is_moderated(&self) -> bool {
        !self.authorized_publishers.is_empty()
    }

    /// Check if a given publisher is authorized for this topic
    pub fn is_authorized(&self, publisher: &PublisherId) -> bool {
        if self.authorized_publishers.is_empty() {
            true // open topic
        } else {
            self.authorized_publishers.contains(publisher)
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::error::PubSubError;
    use crate::message::TopicId;

    use super::TopicConfig;

    fn tid() -> TopicId {
        TopicId([0u8; 32])
    }

    #[test]
    fn try_new_valid() {
        assert!(TopicConfig::try_new(
            tid(), "t".into(), None, vec![],
            Duration::from_secs(60), 1,
        ).is_ok());
    }

    #[test]
    fn try_new_rejects_zero_retention() {
        let err = TopicConfig::try_new(
            tid(), "t".into(), None, vec![],
            Duration::ZERO, 1,
        ).unwrap_err();
        assert!(matches!(err, PubSubError::InvalidConfig(_)));
    }
}

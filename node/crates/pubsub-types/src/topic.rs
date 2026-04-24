use std::time::Duration;

use serde::{Deserialize, Serialize};

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

    /// Replication factor for persistence layer (Phase 1: ignored, all relay nodes cache)
    pub replication_factor: u32,
}

impl TopicConfig {
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

//! Test-only declarative constructors for topic-registry events.
//!
//! Tests that drive registry-derived node state through event sequences read
//! better as a script of one-line steps than as a list of inline struct
//! literals. [`TopicRegistryEvent`] gains compact constructors taking a plain
//! string topic id, and [`TopicRegistryScript`] chains them into an ordered
//! event sequence (mirrors `subscription_registry::MembershipScript`):
//!
//! ```ignore
//! let script = TopicRegistryScript::new()
//!     .registered("weather", [k1])
//!     .publishers_changed("weather", [k4], [k1])
//!     .removed("weather");
//! ```

use std::str::FromStr;

use crate::crypto::PublicKey;
use crate::topic::TopicId;

use super::TopicRegistryEvent;

fn topic(s: &str) -> TopicId {
    TopicId::from_str(s).expect("valid topic id")
}

impl TopicRegistryEvent {
    /// A [`TopicRegistryEvent::Registered`] for `topic` with `publishers`
    /// (empty ⇒ open).
    pub(crate) fn registered(
        topic_id: &str,
        publishers: impl IntoIterator<Item = PublicKey>,
    ) -> Self {
        Self::Registered {
            topic: topic(topic_id),
            publishers: publishers.into_iter().collect(),
        }
    }

    /// A [`TopicRegistryEvent::PublishersChanged`] for `topic` with the given
    /// `added`/`removed` keys.
    pub(crate) fn publishers_changed(
        topic_id: &str,
        added: impl IntoIterator<Item = PublicKey>,
        removed: impl IntoIterator<Item = PublicKey>,
    ) -> Self {
        Self::PublishersChanged {
            topic: topic(topic_id),
            added: added.into_iter().collect(),
            removed: removed.into_iter().collect(),
        }
    }

    /// A [`TopicRegistryEvent::Removed`] for `topic`.
    pub(crate) fn removed(topic_id: &str) -> Self {
        Self::Removed {
            topic: topic(topic_id),
        }
    }
}

/// An ordered topic-registry-event script, built one step per line.
///
/// Iterate it to feed the events into a transition under test.
pub(crate) struct TopicRegistryScript(Vec<TopicRegistryEvent>);

impl TopicRegistryScript {
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }

    /// Append a `Registered` step.
    pub(crate) fn registered(
        mut self,
        topic: &str,
        publishers: impl IntoIterator<Item = PublicKey>,
    ) -> Self {
        self.0
            .push(TopicRegistryEvent::registered(topic, publishers));
        self
    }

    /// Append a `PublishersChanged` step.
    pub(crate) fn publishers_changed(
        mut self,
        topic: &str,
        added: impl IntoIterator<Item = PublicKey>,
        removed: impl IntoIterator<Item = PublicKey>,
    ) -> Self {
        self.0.push(TopicRegistryEvent::publishers_changed(
            topic, added, removed,
        ));
        self
    }

    /// Append a `Removed` step.
    pub(crate) fn removed(mut self, topic: &str) -> Self {
        self.0.push(TopicRegistryEvent::removed(topic));
        self
    }
}

impl IntoIterator for TopicRegistryScript {
    type Item = TopicRegistryEvent;
    type IntoIter = std::vec::IntoIter<TopicRegistryEvent>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

//! The per-topic registry entry, as a declarative type.
//!
//! A registered topic's node-facing facts are "who may publish to it". This
//! wraps that authorized-publisher set behind intention-revealing predicates —
//! [`is_open`](TopicEntry::is_open) and
//! [`is_publisher_authorized`](TopicEntry::is_publisher_authorized) — so the
//! message-accept path reads as `entry.is_publisher_authorized(key)` rather than
//! an inline `set.is_empty() || set.contains(key)` idiom whose "empty ⇒ open"
//! meaning is easy to misread. It is the home future per-topic governance fields
//! (owners, admins, …) attach to without reshaping call sites (the 012
//! consumer). Crate-internal: the node's projection representation, distinct
//! from the public `TopicRegistryEvent`, which keeps carrying a bare
//! `BTreeSet<PublicKey>`.

use std::collections::BTreeSet;

use crate::crypto::PublicKey;

/// A registered topic's authorized publishers (empty ⇒ open: any publisher
/// accepted).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct TopicEntry {
    publishers: BTreeSet<PublicKey>,
}

impl TopicEntry {
    /// Build an entry from an authorized-publisher set (as carried on a
    /// `TopicRegistryEvent::Registered`). An empty set is an open topic.
    pub(crate) fn from_publishers(publishers: BTreeSet<PublicKey>) -> Self {
        Self { publishers }
    }

    /// Whether the topic is **open** — no publisher restriction, any publisher
    /// accepted.
    pub(crate) fn is_open(&self) -> bool {
        self.publishers.is_empty()
    }

    /// Whether `key` may publish to this topic: true on an open topic, otherwise
    /// only when `key` is in the authorized set.
    pub(crate) fn is_publisher_authorized(&self, key: &PublicKey) -> bool {
        self.is_open() || self.publishers.contains(key)
    }

    /// Apply a `PublishersChanged` diff to the authorized set.
    pub(crate) fn apply_publishers_diff(
        &mut self,
        added: BTreeSet<PublicKey>,
        removed: &BTreeSet<PublicKey>,
    ) {
        for key in added {
            self.publishers.insert(key);
        }
        for key in removed {
            self.publishers.remove(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TopicEntry;
    use crate::crypto::PublicKey;
    use std::collections::BTreeSet;

    fn pk(bytes: &[u8]) -> PublicKey {
        PublicKey::new(bytes.to_vec())
    }

    #[test]
    fn empty_set_is_open_and_authorizes_anyone() {
        let entry = TopicEntry::from_publishers(BTreeSet::new());
        assert!(entry.is_open());
        assert!(entry.is_publisher_authorized(&pk(b"anyone")));
    }

    #[test]
    fn restricted_authorizes_only_listed_keys() {
        let entry = TopicEntry::from_publishers(BTreeSet::from([pk(b"k1")]));
        assert!(!entry.is_open());
        assert!(entry.is_publisher_authorized(&pk(b"k1")));
        assert!(!entry.is_publisher_authorized(&pk(b"k2")));
    }

    #[test]
    fn diff_adds_removes_and_can_reopen() {
        let mut entry = TopicEntry::from_publishers(BTreeSet::from([pk(b"k1")]));
        entry.apply_publishers_diff(BTreeSet::from([pk(b"k2")]), &BTreeSet::new());
        assert!(entry.is_publisher_authorized(&pk(b"k2")));
        // Remove every key → open again.
        entry.apply_publishers_diff(BTreeSet::new(), &BTreeSet::from([pk(b"k1"), pk(b"k2")]));
        assert!(entry.is_open());
    }
}

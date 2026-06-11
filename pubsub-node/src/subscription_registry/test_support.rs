//! Test-only declarative constructors for membership events.
//!
//! Tests that drive registry-derived state through event sequences read
//! better as a script of one-line steps than as a list of inline struct
//! literals. [`MembershipEvent`] gains compact constructors taking plain
//! string ids, and [`MembershipScript`] chains them into an ordered event
//! sequence:
//!
//! ```ignore
//! let script = MembershipScript::new()
//!     .joined("a", ["t1"])
//!     .topics_changed("a", ["t2"], ["t1"])
//!     .left("a");
//! ```

use std::collections::BTreeSet;
use std::str::FromStr;

use crate::peer::PeerId;
use crate::topic::TopicId;

use super::MembershipEvent;

fn peer(s: &str) -> PeerId {
    PeerId::from_str(s).expect("valid peer id")
}

fn topics<const N: usize>(ts: [&str; N]) -> BTreeSet<TopicId> {
    ts.iter()
        .map(|t| TopicId::from_str(t).expect("valid topic id"))
        .collect()
}

impl MembershipEvent {
    /// A [`MembershipEvent::Joined`] for `node` on `topics`.
    pub(crate) fn joined<const N: usize>(node: &str, topics_list: [&str; N]) -> Self {
        Self::Joined {
            node: peer(node),
            topics: topics(topics_list),
        }
    }

    /// A [`MembershipEvent::TopicsChanged`] for `node` with `added`/`removed`.
    pub(crate) fn topics_changed<const A: usize, const R: usize>(
        node: &str,
        added: [&str; A],
        removed: [&str; R],
    ) -> Self {
        Self::TopicsChanged {
            node: peer(node),
            added: topics(added),
            removed: topics(removed),
        }
    }

    /// A [`MembershipEvent::Left`] for `node`.
    pub(crate) fn left(node: &str) -> Self {
        Self::Left { node: peer(node) }
    }
}

/// An ordered membership-event script, built one step per line.
///
/// Iterate it to feed the events into a transition under test.
pub(crate) struct MembershipScript(Vec<MembershipEvent>);

impl MembershipScript {
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }

    /// Append a `Joined` step.
    pub(crate) fn joined<const N: usize>(mut self, node: &str, topics: [&str; N]) -> Self {
        self.0.push(MembershipEvent::joined(node, topics));
        self
    }

    /// Append a `TopicsChanged` step.
    pub(crate) fn topics_changed<const A: usize, const R: usize>(
        mut self,
        node: &str,
        added: [&str; A],
        removed: [&str; R],
    ) -> Self {
        self.0
            .push(MembershipEvent::topics_changed(node, added, removed));
        self
    }

    /// Append a `Left` step.
    pub(crate) fn left(mut self, node: &str) -> Self {
        self.0.push(MembershipEvent::left(node));
        self
    }
}

impl IntoIterator for MembershipScript {
    type Item = MembershipEvent;
    type IntoIter = std::vec::IntoIter<MembershipEvent>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

//! [`NodeView`] — the read-only view of node state a strategy reads when making a
//! decision (ADR 0030).
//!
//! Grouping the recurring node-state parameters (`subscriptions`, the candidate
//! sets, the two link maps, `epoch_nonce`) into one borrowed view keeps the seam
//! signatures lean and makes the "read-only context in → decision out" contract
//! explicit: a strategy reads a `NodeView` and returns a decision; it never
//! mutates node state (the transition applies the decision). This is the shape
//! the planned strategies-as-`apply`-arguments refactor generalises.
//!
//! It is a borrow of the current [`NodeState`](crate::state) fields, constructed
//! once per call by the transition — no copying of the candidate/link sets.
//!
//! Candidate access goes through the **self-excluding accessors**
//! ([`candidates_for`](NodeView::candidates_for),
//! [`candidates_len`](NodeView::candidates_len),
//! [`is_candidate`](NodeView::is_candidate)) rather than a raw field: the
//! stored per-topic sets hold a topic's **full membership including the node
//! itself** (ADR 0038 — identical across subscribers, so driver-owned cores
//! can share them), and the exclusion of the node's own id lives here, once,
//! for every reader.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::connection_state::{LinkKey, LinkState};
use crate::peer::PeerId;
use crate::topic::TopicId;

/// A read-only view of the node's current state, passed to a strategy decision.
pub struct NodeView<'a> {
    /// The node's membership-derived topic set (the topics it has joined).
    pub subscriptions: &'a BTreeSet<TopicId>,
    /// The node's own id — what the candidate accessors exclude.
    pub(crate) self_id: &'a PeerId,
    /// Per-topic members of each topic: the **full** membership sets as stored
    /// (the node's own id included when it is a member — ADR 0038). Strategies
    /// read them through the self-excluding accessors below, never raw.
    pub(crate) candidates: &'a BTreeMap<TopicId, Arc<BTreeSet<PeerId>>>,
    /// The node's **upstream** links — peers it receives from (relay pull dials
    /// with their lifecycle; accepted inbound publisher links, always `Active`).
    /// Read by a publisher acceptance instance (its cap counts publisher
    /// upstreams); the relay seams ignore it.
    pub upstream: &'a BTreeMap<LinkKey, LinkState>,
    /// The node's **downstream** links — peers it sends to (accepted relay
    /// peers, always `Active`; publisher dials with their lifecycle). Read by
    /// the relay acceptance seam (re-accept short-circuit + cap count); the
    /// selection seam ignores it.
    pub downstream: &'a BTreeMap<LinkKey, LinkState>,
    /// The current epoch nonce — the randomness context feeding the edge
    /// predicate (ADR 0024/0030/0031). Folded from the last `Epoch` event;
    /// stable across `Heartbeat` dial ticks within an epoch.
    pub epoch_nonce: u64,
}

impl<'a> NodeView<'a> {
    /// The candidate peers on `topic`, with the node's own id excluded.
    ///
    /// Iterates in the stored (sorted) order minus the node itself — exactly
    /// the peers a selection strategy may draw from. Empty if the topic has no
    /// members.
    pub fn candidates_for(&self, topic: &TopicId) -> impl Iterator<Item = &'a PeerId> {
        let self_id = self.self_id;
        self.candidates
            .get(topic)
            .into_iter()
            .flat_map(|peers| peers.iter())
            .filter(move |peer| *peer != self_id)
    }

    /// The number of candidate peers on `topic`, with the node's own id
    /// excluded — the count the bucket derivation reads. Zero if the topic has
    /// no members.
    #[must_use]
    pub fn candidates_len(&self, topic: &TopicId) -> usize {
        self.candidates.get(topic).map_or(0, |peers| {
            peers.len() - usize::from(peers.contains(self.self_id))
        })
    }

    /// Whether `peer` is a candidate (known member) on `topic`. The node's own
    /// id is never a candidate.
    #[must_use]
    pub fn is_candidate(&self, topic: &TopicId, peer: &PeerId) -> bool {
        peer != self.self_id
            && self
                .candidates
                .get(topic)
                .is_some_and(|peers| peers.contains(peer))
    }
}

#[cfg(test)]
mod tests {
    use crate::strategies::test_support::{candidates, peer, subscriptions, topic, view};
    use std::collections::BTreeMap;

    // ADR 0038: the stored sets hold full membership including self; the
    // accessors exclude the node's own id ("self" — the builders' view id).
    #[test]
    fn accessors_exclude_the_nodes_own_id() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a", "self", "z"])]);
        let down = BTreeMap::new();
        let v = view(&subs, &cands, &down);

        let listed: Vec<_> = v.candidates_for(&topic("t1")).cloned().collect();
        assert_eq!(
            listed,
            vec![peer("a"), peer("z")],
            "self skipped, order kept"
        );
        assert_eq!(v.candidates_len(&topic("t1")), 2, "self not counted");
        assert!(v.is_candidate(&topic("t1"), &peer("a")));
        assert!(
            !v.is_candidate(&topic("t1"), &peer("self")),
            "the node's own id is never a candidate",
        );
    }

    // A set without self behaves identically: exclusion is a no-op, not an
    // off-by-one (the count subtracts self only when present).
    #[test]
    fn absent_self_is_not_subtracted() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a", "b"])]);
        let down = BTreeMap::new();
        let v = view(&subs, &cands, &down);
        assert_eq!(v.candidates_len(&topic("t1")), 2);
        assert_eq!(v.candidates_for(&topic("t1")).count(), 2);
    }

    // An unknown topic reads as empty through every accessor.
    #[test]
    fn unknown_topic_reads_empty() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[]);
        let down = BTreeMap::new();
        let v = view(&subs, &cands, &down);
        assert_eq!(v.candidates_len(&topic("t1")), 0);
        assert_eq!(v.candidates_for(&topic("t1")).count(), 0);
        assert!(!v.is_candidate(&topic("t1"), &peer("a")));
    }
}

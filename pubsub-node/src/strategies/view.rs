//! [`NodeView`] — the read-only view of node state a strategy reads when making a
//! decision (ADR 0030/0032).
//!
//! Grouping the recurring node-state parameters (`subscriptions`, `candidates`,
//! `links`, `epoch_nonce`) into one borrowed view keeps the seam signatures
//! lean and makes the "read-only context in → decision out" contract explicit:
//! a strategy reads a `NodeView` and returns a decision; it never mutates node
//! state (the transition applies the decision). This is the shape the planned
//! strategies-as-`apply`-arguments refactor generalises.
//!
//! It is a borrow of the current [`NodeState`](crate::state) fields, constructed
//! once per call by the transition — no copying of the candidate or link sets.

use std::collections::{BTreeMap, BTreeSet};

use crate::connection_state::{LinkDirection, LinkRole, Links};
use crate::peer::PeerId;
use crate::topic::TopicId;

/// A read-only view of the node's current state, passed to a strategy decision.
pub struct NodeView<'a> {
    /// The node's membership-derived topic set (the topics it has joined).
    pub subscriptions: &'a BTreeSet<TopicId>,
    /// Per-topic peers discovered on each topic (the node's own id never present).
    pub candidates: &'a BTreeMap<TopicId, BTreeSet<PeerId>>,
    /// The node's unified link store, keyed by `(peer, topic, role, direction)`
    /// (ADR 0032). Read role-scoped by the seams — acceptance counts a role's
    /// inbound links via [`inbound_scan`](Self::inbound_scan); fan-out selects
    /// its targets from the role × direction cells directly.
    pub links: &'a Links,
    /// The current epoch nonce — the randomness context feeding the edge
    /// predicate (ADR 0024/0030/0031). Folded from the last `Epoch` event;
    /// stable across `Heartbeat` dial ticks within an epoch.
    pub epoch_nonce: u64,
}

impl NodeView<'_> {
    /// One pass over the inbound (`In`) links of `role` for the two facts a
    /// bounding acceptance policy needs: whether `emitter` is already an
    /// accepted inbound link on `topic`, and how many inbound links of that
    /// role the topic holds. Role-scoped, so the relay cap and the publish cap
    /// count disjoint sets (ADR 0033).
    #[must_use]
    pub fn inbound_scan(&self, role: LinkRole, emitter: &PeerId, topic: &TopicId) -> (bool, usize) {
        let mut already_in = false;
        let mut on_topic = 0;
        for (peer, t, r, direction) in self.links.keys() {
            if *r == role && *direction == LinkDirection::In && t == topic {
                on_topic += 1;
                if peer == emitter {
                    already_in = true;
                }
            }
        }
        (already_in, on_topic)
    }
}

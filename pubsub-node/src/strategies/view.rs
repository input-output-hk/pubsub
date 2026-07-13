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

use crate::connection_state::{LinkRole, LinkStore};
use crate::peer::PeerId;
use crate::topic::TopicId;

/// A read-only view of the node's current state, passed to a strategy decision.
pub struct NodeView<'a> {
    /// The node's membership-derived topic set (the topics it has joined).
    pub subscriptions: &'a BTreeSet<TopicId>,
    /// Per-topic peers discovered on each topic (the node's own id never present).
    pub candidates: &'a BTreeMap<TopicId, BTreeSet<PeerId>>,
    /// The node's unified link store, cell-structured by role × direction
    /// (ADR 0032/0034). A strategy reads exactly the cells its model needs —
    /// M3 partitions by role, M4/M5 union cells; acceptance counts a role's
    /// inbound links via [`inbound_scan`](Self::inbound_scan).
    pub links: &'a LinkStore,
    /// The current epoch nonce — the randomness context feeding the edge
    /// predicate (ADR 0024/0030/0031). Folded from the last `Epoch` event;
    /// stable across `Heartbeat` dial ticks within an epoch.
    pub epoch_nonce: u64,
}

impl NodeView<'_> {
    /// One pass over the inbound cell of `role` for the two facts a bounding
    /// acceptance policy needs — see [`LinkStore::inbound_scan`].
    #[must_use]
    pub fn inbound_scan(&self, role: LinkRole, emitter: &PeerId, topic: &TopicId) -> (bool, usize) {
        self.links.inbound_scan(role, emitter, topic)
    }
}

//! [`NodeView`] — the read-only view of node state a strategy reads when making a
//! decision (ADR 0030).
//!
//! Grouping the recurring node-state parameters (`subscriptions`, `candidates`,
//! `downstream`, `interval`) into one borrowed view keeps the seam signatures
//! lean and makes the "read-only context in → decision out" contract explicit:
//! a strategy reads a `NodeView` and returns a decision; it never mutates node
//! state (the transition applies the decision). This is the shape the planned
//! strategies-as-`apply`-arguments refactor generalises.
//!
//! It is a borrow of the current [`NodeState`](crate::state) fields, constructed
//! once per call by the transition — no copying of the candidate/downstream sets.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::peer::PeerId;
use crate::topic::TopicId;

/// A read-only view of the node's current state, passed to a strategy decision.
pub struct NodeView<'a> {
    /// The node's membership-derived topic set (the topics it has joined).
    pub subscriptions: &'a BTreeSet<TopicId>,
    /// Per-topic peers discovered on each topic (the node's own id never present).
    pub candidates: &'a BTreeMap<TopicId, BTreeSet<PeerId>>,
    /// The node's current accepted-inbound (downstream) set.
    pub downstream: &'a HashSet<(PeerId, TopicId)>,
    /// The current heartbeat interval — the round counter feeding the edge
    /// predicate (ADR 0024/0030).
    pub interval: u64,
}

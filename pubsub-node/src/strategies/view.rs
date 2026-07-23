//! [`NodeView`] — the read-only view of node state a strategy reads when making a
//! decision (ADR 0030).
//!
//! Grouping the recurring node-state parameters (`subscriptions`, `candidates`,
//! the two link maps, `epoch_nonce`) into one borrowed view keeps the seam
//! signatures lean and makes the "read-only context in → decision out" contract
//! explicit: a strategy reads a `NodeView` and returns a decision; it never
//! mutates node state (the transition applies the decision). This is the shape
//! the planned strategies-as-`apply`-arguments refactor generalises.
//!
//! It is a borrow of the current [`NodeState`](crate::state) fields, constructed
//! once per call by the transition — no copying of the candidate/link sets.

use std::collections::{BTreeMap, BTreeSet};

use crate::connection_state::{LinkKey, LinkState};
use crate::peer::PeerId;
use crate::topic::TopicId;

/// A read-only view of the node's current state, passed to a strategy decision.
pub struct NodeView<'a> {
    /// The node's membership-derived topic set (the topics it has joined).
    pub subscriptions: &'a BTreeSet<TopicId>,
    /// Per-topic peers discovered on each topic (the node's own id never present).
    pub candidates: &'a BTreeMap<TopicId, BTreeSet<PeerId>>,
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

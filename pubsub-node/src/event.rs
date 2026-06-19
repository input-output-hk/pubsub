//! The node's event queue — the single input a [`Node`](crate::Node) drains.
//!
//! A node has exactly one consumer loop and many producers. Each producer (the
//! network adapter today; a registry reader in feature 008; connection loops in
//! feature 004) holds a cloned [`EventQueue`] and calls [`EventQueue::push`];
//! the node drains the events linearly, one at a time.
//!
//! See `specs/event-loop-and-registry-contract.md` for the cross-feature
//! contract this enables.

use tokio::sync::mpsc::UnboundedSender;

use crate::message::{Message, SignedMessage};
use crate::peer::PeerId;
use crate::subscription_registry::MembershipEvent;
use crate::topic_registry::TopicRegistryEvent;

/// An input to a [`Node`](crate::Node)'s event loop.
///
/// `#[non_exhaustive]`: later features add variants (a registry-update event in
/// feature 008, connection events in feature 004) without breaking external
/// match sites. The node's consumer loop matches every variant explicitly, so
/// adding one is a compile error until its handling is wired in.
#[non_exhaustive]
#[derive(Debug)]
pub enum Event {
    /// A message arrived from the network: the forwarding peer's id and the
    /// message it delivered.
    MessageReceived {
        /// The peer that forwarded the message.
        from: PeerId,
        /// The delivered message.
        message: Message,
    },
    /// A request to publish a message originated on this node, pushed by
    /// [`Node::publish`](crate::Node::publish). The node validates it (the
    /// receive-path checks minus the connection gate and severance; the
    /// publisher need not be the node itself), records it with a local origin,
    /// and fans it out to its downstream peers on the message's topic (ADR
    /// 0021).
    Publish(SignedMessage),
    /// A membership delta from the subscription registry, drained from a
    /// `MembershipWatch` by the node-owned registry reader.
    MembershipUpdate(MembershipEvent),
    /// A topic-registry delta (a topic registered, its publishers changed, or
    /// removed), drained from a `TopicRegistryWatch` by the node-owned
    /// topic-registry reader.
    TopicRegistryUpdate(TopicRegistryEvent),
    /// The node has **synced**: both registries' initial snapshots have been
    /// applied, so the node is at/near the chain tip (ADR 0020). Folding it
    /// transitions the node from `Syncing` to `Synced` and establishes
    /// connections. Pushed once by the registry indexer after it has folded both
    /// snapshots; it is the single readiness signal (the per-registry markers are
    /// gone). Idempotent — a redundant `Synced` after the transition is a no-op.
    Synced,
    /// The connection-establishment **action**: the node consults its
    /// connection-selection strategy and dials the expected upstreams it does
    /// not already hold (ADR 0018). Invoked by the [`Synced`](Event::Synced)
    /// transition, and also available directly through the public event intake
    /// (tests, operator injection, a future epochal re-dial).
    ConnectionSetup,
    /// The graceful-teardown trigger, pushed by
    /// [`Node::shutdown`](crate::Node::shutdown). The node notifies every
    /// connection counterpart, and this event doubles as the event loop's
    /// terminal marker — the loop executes its effects and then stops (ADR
    /// 0019).
    Shutdown,
}

/// A cloneable handle for pushing [`Event`]s onto a node's event queue.
///
/// Obtain one from [`Node::events`](crate::Node::events), or receive one inside
/// a producer registered via [`Node::spawn_producer`](crate::Node::spawn_producer).
/// The node owns the single consumer; producers only push.
#[derive(Clone)]
pub struct EventQueue(UnboundedSender<Event>);

impl EventQueue {
    pub(crate) fn new(tx: UnboundedSender<Event>) -> Self {
        Self(tx)
    }

    /// Push an event onto the queue.
    ///
    /// Silently drops the event if the node's event loop has already shut down
    /// (e.g. the node is being torn down); producers are not expected to treat
    /// that as an error.
    pub fn push(&self, event: Event) {
        let _ = self.0.send(event);
    }
}

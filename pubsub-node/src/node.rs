use std::collections::HashSet;
use std::future::Future;
use std::sync::{Arc, Mutex};

use tokio::task::JoinHandle;

use crate::config::NodeConfig;
use crate::crypto::Verifier;
use crate::error::NodeError;
use crate::event::{Event, EventQueue};
use crate::message::Message;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::network::{Network, NetworkHandle, RoutingFrame};
use crate::peer::{BasicPeerDescriptor, PeerId};
use crate::received::ReceivedDelivery;
use crate::state::{apply, NodeState};
use crate::topic::TopicId;

/// Outcome of a [`Node::subscribe`] call.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SubscribeOutcome {
    /// The topic was not previously in the subscription set; the call added it.
    Added,
    /// The topic was already in the subscription set; the call was a no-op.
    AlreadyPresent,
}

/// Outcome of a [`Node::unsubscribe`] call.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum UnsubscribeOutcome {
    /// The topic was in the subscription set; the call removed it.
    Removed,
    /// The topic was not in the subscription set; the call was a no-op.
    NotSubscribed,
}

/// A network participant.
///
/// Constructed via [`Node::new`], which registers the node on a
/// [`Network`], spawns its event loop and the network producer, and returns
/// once the node is ready to send and observe messages. Inbound messages and
/// any other inputs flow through a single event queue drained by one loop (see
/// [`Event`]); additional producers can be attached via
/// [`spawn_producer`](Node::spawn_producer). The event loop and every producer
/// are aborted when the [`Node`] is dropped.
///
/// A node carries:
/// - its own [`PeerId`],
/// - a static peer set (no peer-set mutation API at this stage),
/// - a mutable subscription set queryable via
///   [`subscriptions`](Node::subscriptions) and mutable through
///   [`subscribe`](Node::subscribe) / [`unsubscribe`](Node::unsubscribe),
/// - a queryable record of received messages accessible via
///   [`received_messages`](Node::received_messages). A delivery enters this
///   record only if its topic is in the subscription set at receive time and
///   its signature verifies; messages failing either check are silently
///   dropped (with an info-level `message_dropped` tracing event carrying a
///   `cause`).
pub struct Node {
    handle: NetworkHandle,
    peers: Vec<BasicPeerDescriptor>,
    // The node's full mutable state as one value (see `crate::state`). The
    // event loop is the sole event-driven writer; the public getters and
    // subscription mutators take the same lock. The verifier's canonical
    // owner is `NodeState`.
    state: Arc<Mutex<NodeState>>,
    events: EventQueue,
    event_loop: JoinHandle<()>,
    // Producer tasks the node owns (the network adapter, plus any attached via
    // `spawn_producer`); all aborted on drop.
    producers: Vec<JoinHandle<()>>,
}

impl Node {
    /// Construct a node, registering on `network` under `self_id` and
    /// spawning its event loop and network producer. A failed registration
    /// returns the error before any background task is spawned.
    ///
    /// `initial_subscriptions` is the set of topics this node will accept on
    /// receive. An empty set yields a node that drops every inbound
    /// message; the set may be mutated at runtime via
    /// [`subscribe`](Self::subscribe) / [`unsubscribe`](Self::unsubscribe).
    ///
    /// `verifier` checks each inbound message's signature; messages whose
    /// signature does not verify are dropped. It is consulted on the receive
    /// path only — a node does not sign.
    ///
    /// Returns [`NodeError`] if registration fails (e.g. the id is already
    /// taken on this network instance).
    pub async fn new<N: Network>(
        self_id: PeerId,
        config: NodeConfig,
        initial_subscriptions: HashSet<TopicId>,
        network: Arc<N>,
        verifier: Arc<dyn Verifier>,
    ) -> Result<Self, NodeError> {
        let mut handle = network.register(self_id).await?;
        let rx = handle.take_receiver();

        // Registration precedes every spawn: a failed construction returns
        // before any background task exists, so nothing leaks on the error
        // path (FR-016).
        let state: Arc<Mutex<NodeState>> = Arc::new(Mutex::new(NodeState::new(
            handle.id().clone(),
            initial_subscriptions,
            verifier,
        )));
        let state_for_task = Arc::clone(&state);

        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
        let events = EventQueue::new(event_tx);

        // The single consumer: drain the event queue and run each event in
        // arrival order through the pure transition, then execute whatever
        // effects it returns. New event variants get their handling in
        // `state::apply`, not here.
        let event_loop = tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                let effects = {
                    let mut guard = state_for_task
                        .lock()
                        .expect("event loop: state mutex poisoned");
                    apply(&mut guard, event)
                };
                // `Effect` is uninhabited pre-connection, so this executor is
                // vacuous; the connection model populates it. The lint is
                // right that the loop never loops — that is the point: the
                // empty match is the compile-time proof that every future
                // variant must be handled here.
                #[allow(clippy::never_loop)]
                for effect in effects {
                    match effect {}
                }
            }
        });

        let peers = config
            .peers
            .into_iter()
            .map(|entry| BasicPeerDescriptor { id: entry.id })
            .collect();

        let mut node = Self {
            handle,
            peers,
            state,
            events,
            event_loop,
            producers: Vec::new(),
        };

        // The network mailbox is the node's first producer: see
        // `network_mailbox_loop`. Future producers (a registry reader,
        // per-connection receive loops) attach the same way, as named
        // async fns handed to `spawn_producer`.
        node.spawn_producer(move |queue| network_mailbox_loop(queue, rx));

        Ok(node)
    }

    /// Return a cloneable handle for pushing [`Event`]s onto this node's event
    /// queue.
    ///
    /// Intended for ad-hoc injection and integration tests. Long-lived
    /// producers should be attached via [`spawn_producer`](Self::spawn_producer)
    /// so the node owns and tears down their task.
    #[must_use]
    pub fn events(&self) -> EventQueue {
        self.events.clone()
    }

    /// Attach a node-owned producer task.
    ///
    /// `producer` receives a clone of this node's [`EventQueue`] and runs until
    /// the node is dropped, at which point its task is aborted. The network
    /// adapter is registered this way at construction; later features attach a
    /// registry reader and per-connection receive loops identically.
    pub fn spawn_producer<F, Fut>(&mut self, producer: F)
    where
        F: FnOnce(EventQueue) -> Fut,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.producers
            .push(tokio::spawn(producer(self.events.clone())));
    }

    /// Dispatch `message` to the peer registered under `to`.
    ///
    /// Resolves once the network has accepted the message for delivery; the
    /// recipient may surface it via [`received_messages`](Self::received_messages)
    /// subsequently if the message's topic is in the recipient's subscription
    /// set at receive time. Sending to an unregistered id is silently
    /// dropped (with a warn-level log entry); senders never observe a
    /// synchronous error for that case. Sending is decoupled from the
    /// sender's own subscription set — a node may emit on a topic it is
    /// not itself subscribed to.
    pub async fn send(&self, to: &PeerId, message: Message) -> Result<(), NodeError> {
        self.handle.send(to, message).await.map_err(NodeError::from)
    }

    /// Return this node's identifier.
    #[must_use]
    pub fn id(&self) -> &PeerId {
        self.handle.id()
    }

    /// Return the node's configured peer set in declaration order.
    ///
    /// The set is static for the node's lifetime; there is no peer-set
    /// mutation API at this stage.
    #[must_use]
    pub fn peers(&self) -> &[BasicPeerDescriptor] {
        &self.peers
    }

    /// Return a snapshot of every delivery observed by this node so far,
    /// in receive order.
    ///
    /// The returned `Vec` is a clone of the node's internal record — it is
    /// stable for the caller and unaffected by subsequent receptions. This
    /// is the observability surface acceptance tests assert against.
    #[must_use]
    pub fn received_messages(&self) -> Vec<ReceivedDelivery> {
        self.state
            .lock()
            .expect("received_messages: state mutex poisoned")
            .received_snapshot()
    }

    /// Add `topic` to this node's subscription set.
    ///
    /// Synchronous; returns [`SubscribeOutcome::Added`] when the topic was
    /// newly inserted, or [`SubscribeOutcome::AlreadyPresent`] when the
    /// topic was already present (idempotent no-op). Emits an info-level
    /// `topic_subscribed` tracing event on `Added`; emits a debug-level
    /// `topic_subscribe_noop` event on `AlreadyPresent`.
    // Thin lock-taker: outcome logic and its log events live on `NodeState`
    // (the pure core), where they are synchronously testable.
    // Not `#[must_use]`: this is a mutator whose outcome is informational;
    // callers that don't care whether the topic was already present
    // legitimately ignore it (unchanged contract from 002).
    #[allow(clippy::must_use_candidate)]
    pub fn subscribe(&self, topic: TopicId) -> SubscribeOutcome {
        self.state
            .lock()
            .expect("subscribe: state mutex poisoned")
            .subscribe(topic)
    }

    /// Remove `topic` from this node's subscription set.
    ///
    /// Synchronous; returns [`UnsubscribeOutcome::Removed`] when the topic
    /// was present and removed, or [`UnsubscribeOutcome::NotSubscribed`]
    /// when the topic was absent (idempotent no-op). Emits an info-level
    /// `topic_unsubscribed` tracing event on `Removed`; emits a debug-level
    /// `topic_unsubscribe_noop` event on `NotSubscribed`.
    // Thin lock-taker; see `subscribe` (including the must_use rationale).
    #[allow(clippy::must_use_candidate)]
    pub fn unsubscribe(&self, topic: TopicId) -> UnsubscribeOutcome {
        self.state
            .lock()
            .expect("unsubscribe: state mutex poisoned")
            .unsubscribe(topic)
    }

    /// Return a snapshot of this node's subscription set.
    ///
    /// The returned `Vec` is built by cloning the subscription set's
    /// contents under the internal lock; entry order is unspecified
    /// (set semantics). The snapshot is stable for the caller and
    /// unaffected by subsequent [`subscribe`](Self::subscribe) /
    /// [`unsubscribe`](Self::unsubscribe) calls on the same node.
    #[must_use]
    pub fn subscriptions(&self) -> Vec<TopicId> {
        self.state
            .lock()
            .expect("subscriptions: state mutex poisoned")
            .subscriptions_snapshot()
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        self.event_loop.abort();
        for producer in &self.producers {
            producer.abort();
        }
    }
}

/// The network mailbox producer: forwards each inbound frame from the
/// network receiver onto the node's event queue.
///
/// The node's first producer, registered through
/// [`spawn_producer`](Node::spawn_producer) at construction; future
/// producers (a registry reader, per-connection receive loops) follow the
/// same named-async-fn shape.
async fn network_mailbox_loop(queue: EventQueue, mut rx: UnboundedReceiver<RoutingFrame>) {
    while let Some(frame) = rx.recv().await {
        queue.push(Event::MessageReceived {
            from: frame.from,
            message: frame.message,
        });
    }
}

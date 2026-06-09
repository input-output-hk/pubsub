use std::collections::HashSet;
use std::future::Future;
use std::sync::{Arc, Mutex};

use tokio::task::JoinHandle;

use crate::config::NodeConfig;
use crate::crypto::Verifier;
use crate::error::NodeError;
use crate::event::{Event, EventQueue};
use crate::message::Message;
use crate::network::{Network, NetworkHandle};
use crate::peer::{BasicPeerDescriptor, PeerId};
use crate::received::ReceivedDelivery;
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
    received: Arc<Mutex<Vec<ReceivedDelivery>>>,
    subscriptions: Arc<Mutex<HashSet<TopicId>>>,
    // Canonical owner of the verifier for the node's lifetime. The event loop
    // consults its own `Arc` clone; this field is retained so the verifier
    // outlives any future non-task use (e.g. a synchronous verify API).
    #[allow(dead_code)]
    verifier: Arc<dyn Verifier>,
    events: EventQueue,
    event_loop: JoinHandle<()>,
    // Producer tasks the node owns (the network adapter, plus any attached via
    // `spawn_producer`); all aborted on drop.
    producers: Vec<JoinHandle<()>>,
}

impl Node {
    /// Construct a node, registering on `network` under `self_id` and
    /// spawning its background receive task.
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
        let self_id_for_task = handle.id().clone();

        let received: Arc<Mutex<Vec<ReceivedDelivery>>> = Arc::new(Mutex::new(Vec::new()));
        let received_for_task = Arc::clone(&received);

        let subscriptions: Arc<Mutex<HashSet<TopicId>>> =
            Arc::new(Mutex::new(initial_subscriptions));
        let subscriptions_for_task = Arc::clone(&subscriptions);

        let verifier_for_task = Arc::clone(&verifier);

        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
        let events = EventQueue::new(event_tx);

        // The single consumer: drain the event queue and apply each event in
        // arrival order. New event variants get their own arm here.
        let event_loop = tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                match event {
                    Event::MessageReceived { from, message } => {
                        tracing::debug!(
                            target: "pubsub_node::node",
                            from = %from,
                            "recv",
                        );

                        match message {
                            Message::Signed(signed) => {
                                // Topic filter first (cheap), then signature
                                // verification — off-topic traffic never pays
                                // the verification cost.
                                let is_subscribed = {
                                    let guard = subscriptions_for_task
                                        .lock()
                                        .expect("event loop: subscriptions mutex poisoned");
                                    guard.contains(&signed.plain.topic)
                                };

                                if !is_subscribed {
                                    tracing::info!(
                                        target: "pubsub_node::node",
                                        event = "message_dropped",
                                        cause = "topic_not_subscribed",
                                        self_id = %self_id_for_task,
                                        from = %from,
                                        topic = %signed.plain.topic,
                                    );
                                    continue;
                                }

                                let verify_outcome = verifier_for_task.verify(
                                    signed.plain.publisher_id.as_public_key(),
                                    &signed.plain.signed_bytes(),
                                    &signed.signature,
                                );
                                if verify_outcome.is_err() {
                                    tracing::info!(
                                        target: "pubsub_node::node",
                                        event = "message_dropped",
                                        cause = "invalid_signature",
                                        self_id = %self_id_for_task,
                                        from = %from,
                                        topic = %signed.plain.topic,
                                        publisher_id = %signed.plain.publisher_id,
                                    );
                                    continue;
                                }

                                let delivery = ReceivedDelivery {
                                    from,
                                    message: Message::Signed(signed),
                                };
                                let mut guard = received_for_task
                                    .lock()
                                    .expect("event loop: received mutex poisoned");
                                guard.push(delivery);
                            }
                        }
                    }
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
            received,
            subscriptions,
            verifier,
            events,
            event_loop,
            producers: Vec::new(),
        };

        // The network mailbox is the node's first producer: it forwards each
        // inbound frame onto the event queue. Future producers (a registry
        // reader, per-connection receive loops) attach the same way.
        node.spawn_producer(move |queue| async move {
            let mut rx = rx;
            while let Some(frame) = rx.recv().await {
                queue.push(Event::MessageReceived {
                    from: frame.from,
                    message: frame.message,
                });
            }
        });

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
        let guard = self
            .received
            .lock()
            .expect("received_messages: received mutex poisoned");
        guard.clone()
    }

    /// Add `topic` to this node's subscription set.
    ///
    /// Synchronous; returns [`SubscribeOutcome::Added`] when the topic was
    /// newly inserted, or [`SubscribeOutcome::AlreadyPresent`] when the
    /// topic was already present (idempotent no-op). Emits an info-level
    /// `topic_subscribed` tracing event on `Added`; emits a debug-level
    /// `topic_subscribe_noop` event on `AlreadyPresent`.
    // Owned `TopicId` matches `HashSet::insert`'s consuming shape and the
    // public-API contract; the lint-flagged "needless pass by value" is the
    // contract choice, not an accident.
    #[allow(clippy::needless_pass_by_value)]
    pub fn subscribe(&self, topic: TopicId) -> SubscribeOutcome {
        let mut guard = self
            .subscriptions
            .lock()
            .expect("subscribe: subscriptions mutex poisoned");
        let was_inserted = guard.insert(topic.clone());
        drop(guard);

        if was_inserted {
            tracing::info!(
                target: "pubsub_node::node",
                event = "topic_subscribed",
                self_id = %self.id(),
                topic = %topic,
            );
            SubscribeOutcome::Added
        } else {
            tracing::debug!(
                target: "pubsub_node::node",
                event = "topic_subscribe_noop",
                self_id = %self.id(),
                topic = %topic,
                reason = "already_present",
            );
            SubscribeOutcome::AlreadyPresent
        }
    }

    /// Remove `topic` from this node's subscription set.
    ///
    /// Synchronous; returns [`UnsubscribeOutcome::Removed`] when the topic
    /// was present and removed, or [`UnsubscribeOutcome::NotSubscribed`]
    /// when the topic was absent (idempotent no-op). Emits an info-level
    /// `topic_unsubscribed` tracing event on `Removed`; emits a debug-level
    /// `topic_unsubscribe_noop` event on `NotSubscribed`.
    // Owned `TopicId` for API symmetry with `subscribe`; see the analogous
    // allow there.
    #[allow(clippy::needless_pass_by_value)]
    pub fn unsubscribe(&self, topic: TopicId) -> UnsubscribeOutcome {
        let mut guard = self
            .subscriptions
            .lock()
            .expect("unsubscribe: subscriptions mutex poisoned");
        let was_removed = guard.remove(&topic);
        drop(guard);

        if was_removed {
            tracing::info!(
                target: "pubsub_node::node",
                event = "topic_unsubscribed",
                self_id = %self.id(),
                topic = %topic,
            );
            UnsubscribeOutcome::Removed
        } else {
            tracing::debug!(
                target: "pubsub_node::node",
                event = "topic_unsubscribe_noop",
                self_id = %self.id(),
                topic = %topic,
                reason = "not_subscribed",
            );
            UnsubscribeOutcome::NotSubscribed
        }
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
        let guard = self
            .subscriptions
            .lock()
            .expect("subscriptions: subscriptions mutex poisoned");
        guard.iter().cloned().collect()
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

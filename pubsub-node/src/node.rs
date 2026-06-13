use std::collections::HashSet;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::task::JoinHandle;

use crate::config::NodeConfig;
use crate::connection::{ConnectionStrategy, UpstreamState};
use crate::crypto::{Signer, Verifier};
use crate::error::NodeError;
use crate::event::{Event, EventQueue};
use crate::message::Message;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::network::{Network, NetworkHandle, NetworkSender, RoutingFrame};
use crate::peer::{BasicPeerDescriptor, PeerId};
use crate::received::ReceivedDelivery;
use crate::state::{apply, Effect, NodeState};
use crate::subscription_registry::SubscriptionRegistry;
use crate::topic::TopicId;
use crate::topic_registry::TopicRegistry;

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
/// - a registry-derived subscription set — the topics it accepts on — queryable
///   via [`subscriptions`](Node::subscriptions) (the topics it both declared in
///   its subscription-list entry **and** that are registered in the topic
///   registry); the node holds no API to mutate it (it is folded from the two
///   registry streams),
/// - a queryable record of received messages accessible via
///   [`received_messages`](Node::received_messages). A delivery enters this
///   record only if its topic is subscribed (declared **and** a registered
///   topic), its publisher is authorized for that topic (or the
///   topic is open), and its signature verifies; messages failing any check are
///   silently dropped (with an info-level `message_dropped` tracing event
///   carrying a `cause`).
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
    /// Construct a node, registering on `network` under `self_id` and spawning
    /// its event loop, the network producer, the subscription-registry reader,
    /// and the topic-registry reader. A failed network registration returns the
    /// error before any background task is spawned.
    ///
    /// The node derives **all** of its registry state from two read-only watch
    /// streams (it is read-only toward both registries — it performs no writes):
    /// - `registry` ([`SubscriptionRegistry`]), node-keyed: its own entry
    ///   resolves its declared subscription set; other nodes' entries build the
    ///   per-topic candidate set ([`candidates`](Self::candidates)).
    /// - `topic_registry` ([`TopicRegistry`]), global: which topics are
    ///   legitimately registered and who may publish to each.
    ///
    /// A message is accepted only if its topic is in the node's effective
    /// subscription set (declared **and** registered, see
    /// [`subscriptions`](Self::subscriptions)), its publisher
    /// is authorized for the topic (or the topic is open), and its signature
    /// verifies. The node starts with empty derived state and converges as the
    /// cold-start bursts drain; topics do not come from config, and a node with
    /// no registry entries simply stays empty (no construction error).
    ///
    /// `verifier` checks each inbound message's signature; messages whose
    /// signature does not verify are dropped. `signer` is the node's signing
    /// identity — it signs the connection-control messages the node emits
    /// (`Request`/`Accepted`/`Terminated`); `strategy` is the
    /// connection-selection policy (v1: `ConnectToAllCandidates`) consulted on
    /// a setup event.
    ///
    /// Construction validates **identity/signer coherence before** registering
    /// on the network: if `self_id` does not match `signer`'s public key it
    /// returns [`NodeError::IdentityMismatch`] with no background activity
    /// started (a mismatch would make every control message the node emits
    /// verifiably invalid and silently dropped by its peers).
    ///
    /// Returns [`NodeError`] if the identity/signer check fails or network
    /// registration fails.
    // The parameter list is the feature's specified construction contract
    // (contracts §4): network + signer + verifier + two registries + strategy
    // are each a distinct collaborator, not incidental sprawl. A config/builder
    // struct is the natural future refactor if it grows further.
    #[allow(clippy::too_many_arguments)]
    pub async fn new<N: Network, R: SubscriptionRegistry, T: TopicRegistry>(
        self_id: PeerId,
        config: NodeConfig,
        network: Arc<N>,
        signer: Arc<dyn Signer>,
        verifier: Arc<dyn Verifier>,
        subscription_registry: Arc<R>,
        topic_registry: Arc<T>,
        strategy: Arc<dyn ConnectionStrategy>,
    ) -> Result<Self, NodeError> {
        // Identity/signer coherence, checked before registration so a mismatch
        // leaks nothing — no handle, no tasks (FR-024).
        if *self_id.as_public_key() != signer.public_key() {
            return Err(NodeError::IdentityMismatch(self_id));
        }

        let connection_setup_delay = config.connection_setup_delay;

        let mut handle = network.register(self_id).await?;
        let node_id = handle.id().clone();
        let rx = handle.take_receiver();
        // The effect executor needs the network send half and the node's own
        // id inside the loop task (to stamp outbound frames as `from`).
        let sender = handle.sender();
        let loop_self_id = node_id.clone();

        // The node starts with an empty subscription set and derives it — and
        // its candidate sets — by folding the subscription-registry `watch` stream (ADR
        // 0013/0014). Registration precedes the spawns so nothing leaks on the
        // error path (FR-016).
        let state: Arc<Mutex<NodeState>> = Arc::new(Mutex::new(NodeState::new(
            node_id.clone(),
            HashSet::new(),
            verifier,
            signer,
            strategy,
        )));
        let state_for_task = Arc::clone(&state);

        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
        let events = EventQueue::new(event_tx);

        // The single consumer: drain the event queue and run each event in
        // arrival order through the pure transition, then execute whatever
        // effects it returns. New event variants get their handling in
        // `state::apply`, not here. The state lock is held only across `apply`
        // and released before effects execute — effects do I/O (`await`) and
        // must not run under the lock.
        let event_loop = tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                let effects = {
                    let mut guard = state_for_task
                        .lock()
                        .expect("event loop: state mutex poisoned");
                    apply(&mut guard, event)
                };
                for effect in effects {
                    execute_effect(&sender, &loop_self_id, effect).await;
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

        // Three node-owned producers, all named async fns handed to
        // `spawn_producer` and aborted on drop: the network mailbox, the
        // subscription-registry reader (node-keyed `watch`), and the
        // topic-registry reader (global `watch`). Each reader owns its registry
        // `Arc` so its watch stays live for the node's lifetime.
        node.spawn_producer(move |queue| network_mailbox_loop(queue, rx));
        node.spawn_producer(move |queue| {
            subscription_registry_reader_loop(queue, subscription_registry, node_id)
        });
        node.spawn_producer(move |queue| topic_registry_reader_loop(queue, topic_registry));

        // The optional fourth producer: the one-shot setup timer, spawned only
        // when a delay is configured. It fires a single `ConnectionSetup` event
        // after the delay (giving the registry view time to converge), then
        // returns; it is drop-aborted like every producer (ADR 0018). Unset by
        // default, so autonomy is opt-in and scripted tests never race a timer.
        if let Some(delay) = connection_setup_delay {
            node.spawn_producer(move |queue| setup_timer_producer(queue, delay));
        }

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

    /// Return the candidate peers for `topic` — the topic-derived membership
    /// the node folded from the subscription registry, with the node's own id
    /// excluded. Order is unspecified; empty if the topic has no members.
    ///
    /// This is distinct from [`peers`](Self::peers) (the static config
    /// bootstrap list); the candidate set is what a future sampler/dialer
    /// draws from.
    #[must_use]
    pub fn candidates(&self, topic: &TopicId) -> Vec<PeerId> {
        self.state
            .lock()
            .expect("candidates: state mutex poisoned")
            .candidates_snapshot(topic)
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

    /// Return a snapshot of this node's subscription set — the topics it
    /// actually accepts messages on (the accept-filter).
    ///
    /// This is the topics the node both **declared** (its own subscription-list
    /// entry, folded from the subscription-registry `watch` stream) **and** that
    /// are **registered** (legitimate) in the topic registry — i.e. the
    /// intersection. A declared topic that is not a registered topic is excluded
    /// (the node drops traffic on it). The node holds no API to mutate this; it
    /// is derived from the two registry streams, and later updates on either
    /// stream may supersede the snapshot. Entry order is unspecified.
    #[must_use]
    pub fn subscriptions(&self) -> Vec<TopicId> {
        self.state
            .lock()
            .expect("subscriptions: state mutex poisoned")
            .subscriptions_snapshot()
    }

    /// Return a snapshot of this node's **upstream** connections — the
    /// `(peer, topic, state)` triples it requested as message sources, each
    /// either [`AwaitingAccept`](crate::UpstreamState::AwaitingAccept) or
    /// [`Active`](crate::UpstreamState::Active).
    ///
    /// A stable clone of the node's record, unaffected by subsequent events;
    /// entry order is unspecified. Pending (`AwaitingAccept`) entries are a
    /// visible diagnostic — a request awaiting an answer that may never come.
    #[must_use]
    pub fn upstream_connections(&self) -> Vec<(PeerId, TopicId, UpstreamState)> {
        self.state
            .lock()
            .expect("upstream_connections: state mutex poisoned")
            .upstream_snapshot()
    }

    /// Return a snapshot of this node's **downstream** connections — the
    /// `(peer, topic)` pairs it accepted as fan-out destinations.
    ///
    /// A stable clone of the node's record, unaffected by subsequent events;
    /// entry order is unspecified.
    #[must_use]
    pub fn downstream_connections(&self) -> Vec<(PeerId, TopicId)> {
        self.state
            .lock()
            .expect("downstream_connections: state mutex poisoned")
            .downstream_snapshot()
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

/// Execute one [`Effect`] the transition returned, outside the state lock.
///
/// `Send` failures are logged and otherwise ignored — the network drops sends
/// to unregistered ids without surfacing an error, mirroring [`Node::send`].
/// `Misbehaved` becomes the operator-facing `connection_severed` warn event
/// and nothing else at this stage. No `apply` arm produces effects yet, so
/// this executor is wired but not exercised until the connection transitions
/// land.
async fn execute_effect(sender: &NetworkSender, self_id: &PeerId, effect: Effect) {
    match effect {
        Effect::Send { to, message } => {
            if let Err(error) = sender.send(self_id, &to, message).await {
                tracing::warn!(
                    target: "pubsub_node::node",
                    %error,
                    to = %to,
                    "connection send failed",
                );
            }
        }
        Effect::Misbehaved { peer, topic, cause } => {
            tracing::warn!(
                target: "pubsub_node::node",
                event = "connection_severed",
                peer = %peer,
                topic = %topic,
                cause,
                "connection severed",
            );
        }
    }
}

/// The optional one-shot setup-timer producer: sleeps for the configured
/// delay, pushes a single [`Event::ConnectionSetup`], and returns.
///
/// Spawned via [`spawn_producer`](Node::spawn_producer) only when a delay is
/// configured (so it is node-owned and drop-aborted); the delay lets the
/// node's registry view converge before it dials (ADR 0018). It fires at most
/// once per node lifetime — the autonomous topology is the single setup
/// event's diff.
async fn setup_timer_producer(queue: EventQueue, delay: Duration) {
    tokio::time::sleep(delay).await;
    queue.push(Event::ConnectionSetup);
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

/// The subscription-registry reader producer: opens the node-keyed
/// [`watch`](SubscriptionRegistry::watch) and drains its [`MembershipWatch`]
/// onto the node's event queue as `MembershipUpdate` events (the node folds
/// them into its subscriptions + candidate sets). Holds the registry `Arc` so
/// the watch's sender side stays alive — and thus the subscription stays live —
/// for the node's lifetime; the task is aborted on drop.
async fn subscription_registry_reader_loop<R: SubscriptionRegistry>(
    queue: EventQueue,
    subscription_registry: Arc<R>,
    node_id: PeerId,
) {
    let mut watch = match subscription_registry.watch(node_id).await {
        Ok(watch) => watch,
        Err(error) => {
            tracing::error!(
                target: "pubsub_node::node",
                %error,
                "subscription-registry watch failed; node has no topics",
            );
            return;
        }
    };
    while let Some(event) = watch.recv().await {
        queue.push(Event::MembershipUpdate(event));
    }
    // `subscription_registry` is owned by this task so the watch's sender side
    // stays alive for the loop; drop it explicitly when the task ends.
    drop(subscription_registry);
}

/// The topic-registry reader producer: opens the global
/// [`watch`](TopicRegistry::watch) and drains its [`TopicRegistryWatch`] onto
/// the node's event queue as `TopicRegistryUpdate` events (the node folds them
/// into its registered-topics projection, which gates the message accept-path).
/// Holds the topic-registry `Arc` so the watch's sender side stays alive for the
/// node's lifetime; the task is aborted on drop.
async fn topic_registry_reader_loop<T: TopicRegistry>(queue: EventQueue, topic_registry: Arc<T>) {
    let mut watch = match topic_registry.watch().await {
        Ok(watch) => watch,
        Err(error) => {
            tracing::error!(
                target: "pubsub_node::node",
                %error,
                "topic-registry watch failed; node has no registered topics",
            );
            return;
        }
    };
    while let Some(event) = watch.recv().await {
        queue.push(Event::TopicRegistryUpdate(event));
    }
    // `topic_registry` is owned by this task so the watch's sender side stays
    // alive for the loop; drop it explicitly when the task ends.
    drop(topic_registry);
}

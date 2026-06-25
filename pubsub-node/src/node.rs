use std::collections::HashSet;
use std::future::Future;
use std::sync::{Arc, Mutex};

use tokio::task::JoinHandle;

use crate::acceptance::ConnectionAcceptanceStrategy;
use crate::config::NodeConfig;
use crate::connection::{ConnectionStrategy, UpstreamState};
use crate::crypto::{Signer, Verifier};
use crate::error::NodeError;
use crate::event::{Event, EventQueue};
use crate::fanout::FanoutStrategy;
use crate::message::{Message, SignedMessage};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::network::{Network, NetworkHandle, NetworkSender, RoutingFrame};
use crate::peer::{BasicPeerDescriptor, PeerId};
use crate::received::ReceivedDelivery;
use crate::state::{apply, Effect, NodeState};
use crate::subscription_registry::{MembershipEvent, SubscriptionRegistry};
use crate::topic::TopicId;
use crate::topic_registry::{TopicRegistry, TopicRegistryEvent};

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
/// - its own [`PeerId`] (a public key) and a signing identity that signs the
///   connection-control messages it emits,
/// - a static peer set (no peer-set mutation API at this stage),
/// - a registry-derived subscription set — the topics it accepts on — queryable
///   via [`subscriptions`](Node::subscriptions) (the topics it both declared in
///   its subscription-list entry **and** that are registered in the topic
///   registry); the node holds no API to mutate it (it is folded from the two
///   registry streams),
/// - **logical connections** to peers, one per `(peer, topic)`, in two roles:
///   *upstream* connections it requested (its message sources, each
///   [`AwaitingAccept`](crate::UpstreamState::AwaitingAccept) or
///   [`Active`](crate::UpstreamState::Active)) and *downstream* connections it
///   accepted (its fan-out destinations). They are established autonomously by
///   an injected connection-selection strategy on a setup event, exchanged as
///   signed control messages over the network, and observable through
///   [`upstream_connections`](Node::upstream_connections) /
///   [`downstream_connections`](Node::downstream_connections). There is no
///   manual connect/disconnect API — only construction, [`send`](Node::send),
///   [`shutdown`](Node::shutdown), and the read-only snapshot getters.
/// - a queryable record of received messages accessible via
///   [`received_messages`](Node::received_messages). The receive path is
///   **connection-gated**: a delivery enters the record only if the delivering
///   peer holds an Active upstream with the node for the message's topic, the
///   topic is subscribed (declared **and** a registered topic), the publisher
///   is authorized for that topic (or the topic is open), and the signature
///   verifies. Messages failing a check are silently dropped (an info-level
///   `message_dropped` event with a `cause`); a signature failure over an
///   otherwise-admissible Active upstream additionally **severs** that
///   connection (a warn-level `connection_severed`, no notice sent).
///
/// Teardown has two paths: [`shutdown`](Node::shutdown) (consuming, awaitable)
/// notifies every connection counterpart before releasing the node; a plain
/// drop is the abrupt, no-notice path.
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
    /// (`Request`/`Accepted`/`Terminated`); `connection_strategy` is the
    /// connection-selection policy (v1: `ConnectToAllCandidates`) consulted on
    /// a setup event; `fanout_strategy` is the fan-out policy (v1:
    /// `ForwardToAll`) consulted at the record point to choose which downstream
    /// peers a recorded message is forwarded to; `acceptance_strategy` is the
    /// inbound-acceptance policy (v1: `AcceptFromAllCandidates`) consulted on a
    /// verified connection `Request` to decide whether to accept the emitter as
    /// downstream.
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
    // (contracts §4): network + signer + verifier + two registries + connection
    // strategy + fan-out strategy + acceptance strategy are each a distinct
    // collaborator, not incidental sprawl. A config/builder struct is the natural
    // future refactor if it grows further.
    #[allow(clippy::too_many_arguments)]
    pub async fn new<N: Network, R: SubscriptionRegistry, T: TopicRegistry>(
        self_id: PeerId,
        config: NodeConfig,
        network: Arc<N>,
        signer: Arc<dyn Signer>,
        verifier: Arc<dyn Verifier>,
        subscription_registry: Arc<R>,
        topic_registry: Arc<T>,
        connection_strategy: Arc<dyn ConnectionStrategy>,
        fanout_strategy: Arc<dyn FanoutStrategy>,
        acceptance_strategy: Arc<dyn ConnectionAcceptanceStrategy>,
    ) -> Result<Self, NodeError> {
        // Identity/signer coherence, checked before registration so a mismatch
        // leaks nothing — no handle, no tasks (FR-024).
        if *self_id.as_public_key() != signer.public_key() {
            return Err(NodeError::IdentityMismatch(self_id));
        }

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
            connection_strategy,
            fanout_strategy,
            acceptance_strategy,
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
                // ADR 0019 carve-out: the loop inspects the event kind only to
                // know when to stop — the event's *handling* (clear + notices)
                // still lives in `state::apply`. After executing a Shutdown
                // event's effects (the Terminated notices), the loop breaks;
                // that termination is the signal `shutdown` awaits, guaranteeing
                // the notices were handed to the network first.
                let is_shutdown = matches!(event, Event::Shutdown);
                let effects = {
                    let mut guard = state_for_task
                        .lock()
                        .expect("event loop: state mutex poisoned");
                    apply(&mut guard, event)
                };
                for effect in effects {
                    execute_effect(&sender, &loop_self_id, effect).await;
                }
                if is_shutdown {
                    break;
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

        // Two node-owned producers, both named async fns handed to
        // `spawn_producer` and aborted on drop: the network mailbox and the
        // registry indexer. The indexer is a *single* reader that follows both
        // registry watches — the one chain follower a realistic deployment runs
        // (ADR 0020, 2026-06-18). Owning both registry `Arc`s, it keeps the
        // watches live for the node's lifetime.
        //
        // Cold-start ordering is intrinsic to the single reader (ADR 0020): it
        // folds the topic snapshot before the membership snapshot, so a
        // membership topic is evaluated against an already-warm registered set
        // (strict drop is correct with no cross-stream ordering primitive). Once
        // both snapshots are folded it pushes one `Synced` — the single
        // readiness signal that transitions the node to `Synced` and dials.
        node.spawn_producer(move |queue| network_mailbox_loop(queue, rx));
        node.spawn_producer(move |queue| {
            registry_indexer_loop(queue, subscription_registry, topic_registry, node_id)
        });

        // Autonomous establishment is **event-driven**, not timer-driven: the
        // indexer pushes `Synced` once both registry snapshots are folded, so the
        // node dials when its membership view has converged. No wall-clock setup
        // timer is armed (ADR 0020 supersedes 0018's timer).
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

    /// Publish a message that originates on this node — fire-and-forget.
    ///
    /// Enqueues the message for the node's event loop and returns immediately;
    /// validation and fan-out happen later in the loop, so there is no
    /// synchronous verdict. An accepted publish is recorded with a local origin
    /// (observable via [`received_messages`](Self::received_messages)) and
    /// forwarded to the node's downstream peers on the message's topic; a
    /// publish that fails a check (the topic is not in the node's subscriptions,
    /// is not registered, the publisher is not authorized, or the signature does
    /// not verify) is silently dropped (an info-level `message_dropped` event).
    ///
    /// The publisher carried in `message` need not be this node — a validly
    /// signed, authorized message from any publisher is accepted (proxy /
    /// injection). The message must already be signed; the node mints nothing.
    ///
    /// Duplicate suppression spans both the publish and receive paths: a message
    /// whose content this node has already accepted — including one it published
    /// itself, then had relayed back — is dropped with no second record and no
    /// re-forward, so forwarding terminates in cyclic topologies.
    pub fn publish(&self, message: SignedMessage) {
        self.events.push(Event::Publish(message));
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
    /// is the observability surface acceptance tests assert against. Each
    /// [`ReceivedDelivery`] carries an [`Origin`](crate::Origin) distinguishing a
    /// message this node published ([`Local`](crate::Origin::Local)) from one a
    /// peer forwarded ([`Peer`](crate::Origin::Peer)); the publisher identity is
    /// inside the message itself, independent of origin.
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

    /// Whether the node has **synced** — both registry snapshots have been
    /// applied, so it is at/near the chain tip and has begun establishing
    /// connections (ADR 0020). `false` while still replaying the registries at
    /// startup. The observable `Syncing`/`Synced` lifecycle mode.
    #[must_use]
    pub fn is_synced(&self) -> bool {
        self.state
            .lock()
            .expect("is_synced: state mutex poisoned")
            .is_synced()
    }

    /// Whether `topic` is **registered** (a legitimate topic) in the node's
    /// folded view of the topic registry. The topic-governance counterpart to
    /// [`subscriptions`](Self::subscriptions): a topic the node is a member of
    /// is only *effective* once it is also registered here (the cross-registry
    /// invariant), so this distinguishes "not a member" from "member of an
    /// unregistered topic". Derived from the topic-registry `watch` stream; a
    /// later update may supersede the answer.
    #[must_use]
    pub fn is_registered(&self, topic: &TopicId) -> bool {
        self.state
            .lock()
            .expect("is_registered: state mutex poisoned")
            .is_registered(topic)
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

    /// Gracefully shut the node down: drain any already-queued events, send one
    /// `Terminated` notice per held connection (both roles, any state), then
    /// release the node. Consuming `self` makes use-after-shutdown
    /// unrepresentable.
    ///
    /// Resolves only after the notices have been handed to the network: it
    /// pushes a `Shutdown` event and awaits the event loop's completion (the
    /// loop breaks once that event's effects have executed). Plain
    /// [`drop`](Drop) without calling this remains the abrupt, no-notice path —
    /// counterparts keep stale entries, which admit nothing and are
    /// re-confirmed idempotently if the node returns.
    pub async fn shutdown(mut self) {
        self.events.push(Event::Shutdown);
        // `JoinHandle` is `Unpin`; await by reference (the node has a `Drop`
        // impl, so the handle cannot be moved out). A join error means the loop
        // task panicked or was aborted — log and proceed to drop.
        if let Err(error) = (&mut self.event_loop).await {
            tracing::warn!(
                target: "pubsub_node::node",
                %error,
                "event loop did not complete cleanly during shutdown",
            );
        }
        // `self` drops here: `Drop` aborts the producers (and the
        // already-finished loop, a no-op).
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
/// and nothing else at this stage. The connection transitions (004) emit both
/// variants, and fan-out (006) makes `Send` the primary path — one per
/// forwarded message at the record point.
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

/// The registry indexer producer: the node's single follower of both registry
/// watches, modelling the one chain indexer a realistic deployment runs (ADR
/// 0020, 2026-06-18). A real indexer reads the chain once and has a single
/// "caught up to tip" moment covering both the topic registry and the
/// subscription list; this reader is its in-memory analogue over the two mock
/// watches.
///
/// Each `watch()` returns a current-state **snapshot** plus a live-delta stream.
/// The indexer folds the topic snapshot first (so a membership topic is
/// evaluated against an already-warm registered set — strict drop, ADR 0020),
/// then the membership snapshot, then pushes a single [`Event::Synced`]: the one
/// readiness signal, which transitions the node to `Synced` and establishes
/// connections. Thereafter it forwards live deltas from both watches until both
/// close. There are no per-registry readiness markers — the snapshot/live split
/// replaces them.
///
/// Owns both registry `Arc`s so the watches stay live for the node's lifetime;
/// aborted on drop.
async fn registry_indexer_loop<R: SubscriptionRegistry, T: TopicRegistry>(
    queue: EventQueue,
    subscription_registry: Arc<R>,
    topic_registry: Arc<T>,
    node_id: PeerId,
) {
    // Topic snapshot first — warms the registered set before any membership
    // event is folded (strict-drop ordering, intrinsic to this single reader).
    // A watch-open failure degrades to an empty snapshot + no live stream; the
    // node still reaches `Synced` (against whatever state exists).
    let mut topic_watch = match topic_registry.watch().await {
        Ok((snapshot, watch)) => {
            for (topic, publishers) in snapshot {
                queue.push(Event::TopicRegistryUpdate(TopicRegistryEvent::Registered {
                    topic,
                    publishers,
                }));
            }
            Some(watch)
        }
        Err(error) => {
            tracing::error!(
                target: "pubsub_node::node",
                %error,
                "topic-registry watch failed; node has no registered topics",
            );
            None
        }
    };
    let mut sub_watch = match subscription_registry.watch(node_id).await {
        Ok((snapshot, watch)) => {
            for (node, topics) in snapshot {
                queue.push(Event::MembershipUpdate(MembershipEvent::Joined {
                    node,
                    topics,
                }));
            }
            Some(watch)
        }
        Err(error) => {
            tracing::error!(
                target: "pubsub_node::node",
                %error,
                "subscription-registry watch failed; node has no topics",
            );
            None
        }
    };

    // Both snapshots are folded → the node is synced. One readiness signal,
    // which transitions to `Synced` and establishes connections (ADR 0020).
    queue.push(Event::Synced);

    // Live deltas: forward both streams until both close. The `if` guards keep a
    // closed watch from being polled (no busy-loop).
    let mut topic_open = topic_watch.is_some();
    let mut sub_open = sub_watch.is_some();
    while topic_open || sub_open {
        tokio::select! {
            event = topic_watch.as_mut().expect("topic_open implies Some").recv(), if topic_open => {
                match event {
                    Some(event) => queue.push(Event::TopicRegistryUpdate(event)),
                    None => topic_open = false,
                }
            }
            event = sub_watch.as_mut().expect("sub_open implies Some").recv(), if sub_open => {
                match event {
                    Some(event) => queue.push(Event::MembershipUpdate(event)),
                    None => sub_open = false,
                }
            }
        }
    }
    // Both registries are owned by this task so the watches' sender sides stay
    // alive for the loop; drop them explicitly when the task ends.
    drop(topic_registry);
    drop(subscription_registry);
}

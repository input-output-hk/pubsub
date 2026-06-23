use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;

use crate::error::NetworkError;
use crate::message::Message;
use crate::peer::PeerId;

/// Network-layer routing wrapper: carries a [`Message`] from a sender's
/// [`PeerId`] to a receiver's mailbox.
///
/// Renamed from the 001-era `Envelope` per ADR 0010, freeing the term
/// "envelope" for the whole signed message at the protocol layer. This type's
/// fields, behaviour, and routing role are unchanged from `Envelope`.
pub(crate) struct RoutingFrame {
    pub from: PeerId,
    pub message: Message,
}

/// Network abstraction for routing messages between participants.
///
/// Implementors manage peer registration and message dispatch; callers
/// register a peer via [`Network::register`] and use the returned
/// [`NetworkHandle`] for sends.
///
/// The trait carries `Send + Sync + 'static` because nodes hold the network
/// behind an `Arc` and pass it to spawned tasks.
// FUTURE: when a second `Network` impl arrives (e.g. a real TCP-based
// transport), revisit the `async fn` trait shape. Today the v1 lint
// `async_fn_in_trait` is allowed because there is exactly one implementor
// (`InMemoryNetwork`) whose returned future is `Send` by inference. With
// multiple implementors — or any impl whose body holds a non-`Send` local
// across `.await` — we should switch to a `Send`-bounded return shape, e.g.
// `-> impl Future<Output = ...> + Send` (RPITIT) or the `async-trait` /
// `trait_variant` crates. Tracked under research.md "Open follow-ups".
#[allow(async_fn_in_trait)]
pub trait Network: Send + Sync + 'static {
    /// Register a peer under `id` and return its [`NetworkHandle`].
    ///
    /// Safe to call concurrently from multiple async tasks. Returns
    /// [`NetworkError::DuplicateRegistration`] if `id` is already registered
    /// on this network instance.
    async fn register(&self, id: PeerId) -> Result<NetworkHandle, NetworkError>;
}

type PeerSenders = Arc<RwLock<HashMap<PeerId, UnboundedSender<RoutingFrame>>>>;

#[derive(Clone)]
pub(crate) struct NetworkSender {
    registry: PeerSenders,
}

impl NetworkSender {
    pub(crate) async fn send(
        &self,
        from: &PeerId,
        to: &PeerId,
        message: Message,
    ) -> Result<(), NetworkError> {
        let guard = self.registry.read().await;
        if let Some(tx) = guard.get(to) {
            let frame = RoutingFrame {
                from: from.clone(),
                message,
            };
            if tx.send(frame).is_ok() {
                tracing::debug!(
                    target: "pubsub_node::network",
                    from = %from,
                    to = %to,
                    "send.accepted",
                );
            }
        } else {
            tracing::warn!(
                target: "pubsub_node::network",
                peer_id = %to,
                "send dropped: unregistered peer id",
            );
        }
        Ok(())
    }
}

/// Per-peer attach token returned by [`Network::register`].
///
/// Bundles the peer's identity, a cloneable sender into the network's
/// dispatch fabric, and a single-consumer receiver for the peer's mailbox.
/// [`Node`](crate::Node) owns the handle for its lifetime; the sender
/// identity used for outbound messages is fixed by the handle's `id` and
/// cannot be spoofed by callers.
///
/// The handle is intentionally **not** `Clone` — the receive side is
/// single-consumer.
pub struct NetworkHandle {
    self_id: PeerId,
    tx: NetworkSender,
    rx: Option<UnboundedReceiver<RoutingFrame>>,
}

impl NetworkHandle {
    /// Return the peer's identifier (the id this handle was issued for).
    #[must_use]
    pub fn id(&self) -> &PeerId {
        &self.self_id
    }

    /// Dispatch `message` to the peer registered under `to`.
    ///
    /// Resolves once the network has accepted the message for delivery; the
    /// recipient may process it into its observable record subsequently. If
    /// `to` is not registered the message is dropped and a warn-level
    /// `tracing` event is emitted naming the unregistered id — the call
    /// still resolves with `Ok(())`.
    pub async fn send(&self, to: &PeerId, message: Message) -> Result<(), NetworkError> {
        self.tx.send(&self.self_id, to, message).await
    }

    pub(crate) fn take_receiver(&mut self) -> UnboundedReceiver<RoutingFrame> {
        self.rx
            .take()
            .expect("NetworkHandle::take_receiver called more than once")
    }

    /// Clone the sender half of this handle, for the node's effect executor to
    /// dispatch `Effect::Send` from inside the event loop. The clone stamps
    /// outbound frames with this handle's id (the loop passes it as `from`).
    pub(crate) fn sender(&self) -> NetworkSender {
        self.tx.clone()
    }
}

/// In-process, in-memory [`Network`] implementation.
///
/// Routes messages through an `Arc`-shared registry of per-peer mailboxes;
/// suitable for tests and single-process demonstrations. There is no
/// transport, no persistence, and no cross-process delivery — two processes
/// that each construct their own `InMemoryNetwork` cannot exchange messages.
///
/// Share a single instance among multiple nodes via `Arc`:
///
/// ```no_run
/// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
/// # use std::sync::Arc;
/// # use pubsub_node::{AcceptFromAllCandidates, ConnectToAllCandidates, ForwardToAll, InMemoryNetwork, InMemorySubscriptionRegistry, InMemoryTopicRegistry, MockCryptoScheme, Node, NodeConfig, PeerId, Signer, TestVerifier, Verifier};
/// # let self_id: PeerId = "node-a".parse()?;
/// # let config = NodeConfig::default();
/// let network = Arc::new(InMemoryNetwork::new());
/// let scheme = MockCryptoScheme::with_seed([0u8; 32]);
/// let signer: Arc<dyn Signer> = Arc::new(scheme.signer(scheme.keypair_from_alias("node-a").private));
/// let verifier: Arc<dyn Verifier> = Arc::new(TestVerifier);
/// let registry = Arc::new(InMemorySubscriptionRegistry::new());
/// let topic_registry = Arc::new(InMemoryTopicRegistry::new());
/// let strategy = Arc::new(ConnectToAllCandidates);
/// let fanout = Arc::new(ForwardToAll);
/// let acceptance = Arc::new(AcceptFromAllCandidates);
/// let node = Node::new(self_id, config, network.clone(), signer, verifier, registry, topic_registry, strategy, fanout, acceptance).await?;
/// # Ok(())
/// # }
/// ```
pub struct InMemoryNetwork {
    registry: PeerSenders,
}

impl InMemoryNetwork {
    /// Construct a fresh in-memory network with no registered peers.
    #[must_use]
    pub fn new() -> Self {
        Self {
            registry: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryNetwork {
    fn default() -> Self {
        Self::new()
    }
}

impl Network for InMemoryNetwork {
    async fn register(&self, id: PeerId) -> Result<NetworkHandle, NetworkError> {
        let (tx, rx) = unbounded_channel::<RoutingFrame>();
        let mut guard = self.registry.write().await;
        if guard.contains_key(&id) {
            return Err(NetworkError::DuplicateRegistration(id));
        }
        guard.insert(id.clone(), tx);
        drop(guard);

        // FUTURE: swap to bounded mpsc::channel when a real transport
        // introduces backpressure (research.md §7, ADR slot v2+).
        Ok(NetworkHandle {
            self_id: id,
            tx: NetworkSender {
                registry: Arc::clone(&self.registry),
            },
            rx: Some(rx),
        })
    }
}

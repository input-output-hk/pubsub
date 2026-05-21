use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;

use crate::error::NetworkError;
use crate::message::Message;
use crate::peer::PeerId;

pub(crate) struct Envelope {
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

type Registry = Arc<RwLock<HashMap<PeerId, UnboundedSender<Envelope>>>>;

#[derive(Clone)]
pub(crate) struct NetworkSender {
    registry: Registry,
}

impl NetworkSender {
    async fn send(&self, from: &PeerId, to: &PeerId, message: Message) -> Result<(), NetworkError> {
        let guard = self.registry.read().await;
        if let Some(tx) = guard.get(to) {
            let env = Envelope {
                from: from.clone(),
                message,
            };
            if tx.send(env).is_ok() {
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
    rx: Option<UnboundedReceiver<Envelope>>,
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

    pub(crate) fn take_receiver(&mut self) -> UnboundedReceiver<Envelope> {
        self.rx
            .take()
            .expect("NetworkHandle::take_receiver called more than once")
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
/// ```ignore
/// let network = std::sync::Arc::new(InMemoryNetwork::new());
/// let node = Node::new(self_id, peer_list, network.clone()).await?;
/// ```
pub struct InMemoryNetwork {
    registry: Registry,
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
        let (tx, rx) = unbounded_channel::<Envelope>();
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

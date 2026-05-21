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

pub struct NetworkHandle {
    self_id: PeerId,
    tx: NetworkSender,
    rx: Option<UnboundedReceiver<Envelope>>,
}

impl NetworkHandle {
    #[must_use]
    pub fn id(&self) -> &PeerId {
        &self.self_id
    }

    pub async fn send(&self, to: &PeerId, message: Message) -> Result<(), NetworkError> {
        self.tx.send(&self.self_id, to, message).await
    }

    pub(crate) fn take_receiver(&mut self) -> UnboundedReceiver<Envelope> {
        self.rx
            .take()
            .expect("NetworkHandle::take_receiver called more than once")
    }
}

pub struct InMemoryNetwork {
    registry: Registry,
}

impl InMemoryNetwork {
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

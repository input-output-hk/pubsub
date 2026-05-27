use std::sync::{Arc, Mutex};

use tokio::task::JoinHandle;

use crate::config::PeerListConfig;
use crate::error::NodeError;
use crate::message::Message;
use crate::network::{Network, NetworkHandle};
use crate::peer::{BasicPeerDescriptor, PeerId};
use crate::received::ReceivedDelivery;

/// A network participant.
///
/// Constructed via [`Node::new`], which registers the node on a
/// [`Network`], spawns a background receive task, and returns once the node
/// is ready to send and observe messages. The receive task is aborted when
/// the [`Node`] is dropped.
///
/// A node carries:
/// - its own [`PeerId`],
/// - a static peer set (no peer-set mutation API at this stage),
/// - a queryable record of received messages accessible via
///   [`received_messages`](Node::received_messages).
pub struct Node {
    handle: NetworkHandle,
    peers: Vec<BasicPeerDescriptor>,
    received: Arc<Mutex<Vec<ReceivedDelivery>>>,
    recv_task: JoinHandle<()>,
}

impl Node {
    /// Construct a node, registering on `network` under `self_id` and
    /// spawning its background receive task.
    ///
    /// Returns [`NodeError`] if registration fails (e.g. the id is already
    /// taken on this network instance).
    pub async fn new<N: Network>(
        self_id: PeerId,
        peer_list: PeerListConfig,
        network: Arc<N>,
    ) -> Result<Self, NodeError> {
        let mut handle = network.register(self_id).await?;
        let mut rx = handle.take_receiver();

        let received: Arc<Mutex<Vec<ReceivedDelivery>>> = Arc::new(Mutex::new(Vec::new()));
        let received_for_task = Arc::clone(&received);

        let recv_task = tokio::spawn(async move {
            while let Some(env) = rx.recv().await {
                tracing::debug!(
                    target: "pubsub_node::node",
                    from = %env.from,
                    "recv",
                );
                let delivery = ReceivedDelivery {
                    from: env.from,
                    message: env.message,
                };
                let mut guard = received_for_task
                    .lock()
                    .expect("recv task: received mutex poisoned");
                guard.push(delivery);
            }
        });

        let peers = peer_list
            .peers
            .into_iter()
            .map(|entry| BasicPeerDescriptor { id: entry.id })
            .collect();

        Ok(Self {
            handle,
            peers,
            received,
            recv_task,
        })
    }

    /// Dispatch `message` to the peer registered under `to`.
    ///
    /// Resolves once the network has accepted the message for delivery; the
    /// recipient may surface it via [`received_messages`](Self::received_messages)
    /// subsequently. Sending to an unregistered id is silently dropped (with
    /// a warn-level log entry); senders never observe a synchronous error
    /// for that case.
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
}

impl Drop for Node {
    fn drop(&mut self) {
        self.recv_task.abort();
    }
}

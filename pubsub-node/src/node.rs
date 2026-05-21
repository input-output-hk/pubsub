use std::sync::{Arc, Mutex};

use tokio::task::JoinHandle;

use crate::config::PeerListConfig;
use crate::error::NodeError;
use crate::message::Message;
use crate::network::{Network, NetworkHandle};
use crate::peer::{BasicPeerDescriptor, PeerId};
use crate::received::ReceivedDelivery;

pub struct Node {
    handle: NetworkHandle,
    peers: Vec<BasicPeerDescriptor>,
    received: Arc<Mutex<Vec<ReceivedDelivery>>>,
    recv_task: JoinHandle<()>,
}

impl Node {
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

    pub async fn send(&self, to: &PeerId, message: Message) -> Result<(), NodeError> {
        self.handle.send(to, message).await.map_err(NodeError::from)
    }

    #[must_use]
    pub fn id(&self) -> &PeerId {
        self.handle.id()
    }

    #[must_use]
    pub fn peers(&self) -> &[BasicPeerDescriptor] {
        &self.peers
    }

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

// Shared test-harness module. Each integration test binary in `tests/` is
// compiled separately and may use only a subset of these helpers, so silence
// per-binary `dead_code` warnings here at the module level.
#![allow(dead_code)]

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use pubsub_node::{
    InMemoryNetwork, Message, Node, PeerEntry, PeerId, PeerListConfig, ReceivedDelivery,
};

pub struct TwoNodeFixture {
    pub network: Arc<InMemoryNetwork>,
    pub a: Node,
    pub b: Node,
}

pub async fn two_node_fixture() -> TwoNodeFixture {
    let network = Arc::new(InMemoryNetwork::new());
    let a_id = PeerId::from_str("node-a").expect("valid id");
    let b_id = PeerId::from_str("node-b").expect("valid id");

    let a = Node::new(
        a_id.clone(),
        PeerListConfig {
            peers: vec![PeerEntry { id: b_id.clone() }],
        },
        network.clone(),
    )
    .await
    .expect("construct node A");

    let b = Node::new(
        b_id,
        PeerListConfig {
            peers: vec![PeerEntry { id: a_id }],
        },
        network.clone(),
    )
    .await
    .expect("construct node B");

    TwoNodeFixture { network, a, b }
}

#[derive(Debug, thiserror::Error)]
pub enum AwaitError {
    #[error("timed out after {0:?} waiting for delivery")]
    Timeout(Duration),
}

pub async fn await_delivery(
    node: &Node,
    expected_sender: &PeerId,
    expected_message: &Message,
    timeout: Duration,
) -> Result<(), AwaitError> {
    let poll_interval = Duration::from_millis(1);
    let start = tokio::time::Instant::now();
    loop {
        if matches(&node.received_messages(), expected_sender, expected_message) {
            return Ok(());
        }
        if start.elapsed() >= timeout {
            return Err(AwaitError::Timeout(timeout));
        }
        tokio::time::sleep(poll_interval).await;
    }
}

fn matches(
    record: &[ReceivedDelivery],
    expected_sender: &PeerId,
    expected_message: &Message,
) -> bool {
    record
        .iter()
        .any(|d| &d.from == expected_sender && &d.message == expected_message)
}

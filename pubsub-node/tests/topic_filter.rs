mod common;

use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::{await_delivery, two_node_fixture_with_subscriptions};
use pubsub_node::{InMemoryNetwork, Message, Node, NodeConfig, PeerEntry, PeerId, TopicId};

fn topic(s: &str) -> TopicId {
    TopicId::from_str(s).expect("valid topic id")
}

// US1 AS-1: A subscribed to {T1}; B sends Ping(42, T1) to A; A's record
// contains exactly that delivery.
#[tokio::test]
async fn on_topic_message_retained() {
    let t1 = topic("t1");
    let fx = two_node_fixture_with_subscriptions(
        HashSet::from([t1.clone()]),
        HashSet::from([t1.clone()]),
    )
    .await;
    let msg = Message::ping(t1.clone(), 42);

    fx.b.send(fx.a.id(), msg.clone()).await.expect("send Ok");

    await_delivery(&fx.a, fx.b.id(), &msg, Duration::from_secs(1))
        .await
        .expect("delivery within 1s");

    let record = fx.a.received_messages();
    assert_eq!(record.len(), 1, "A retains the on-topic delivery");
    assert_eq!(record[0].from, *fx.b.id());
    assert_eq!(record[0].message, msg);
}

// US1 AS-2: A subscribed to {T1}; B sends Ping(7, T2) to A; A's record
// stays empty after a settle window (off-topic silent drop).
#[tokio::test]
async fn off_topic_message_dropped_silently() {
    let t1 = topic("t1");
    let t2 = topic("t2");
    let fx = two_node_fixture_with_subscriptions(
        HashSet::from([t1.clone()]),
        HashSet::from([t2.clone()]),
    )
    .await;
    let off_topic_msg = Message::ping(t2, 7);

    fx.b.send(fx.a.id(), off_topic_msg).await.expect("send Ok");

    // Settle window: matches the 001 poll-interval ceiling. The drop is a
    // recv-task-side filter; if it were going to land in the snapshot it
    // would do so within this window.
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(
        fx.a.received_messages().is_empty(),
        "A's record stays empty for off-topic deliveries",
    );
}

// US1 AS-3 / FR-009: A subscribed to {T1}; A emits Ping(13, T1) to B
// (a separate peer). A's own snapshot stays empty — only network-delivered
// messages enter the snapshot, never a Node's own emissions.
#[tokio::test]
async fn own_emission_not_in_local_snapshot() {
    let t1 = topic("t1");
    let network = Arc::new(InMemoryNetwork::new());
    let a_id = PeerId::from_str("node-a").expect("valid id");
    let b_id = PeerId::from_str("node-b").expect("valid id");

    let a = Node::new(
        a_id.clone(),
        NodeConfig {
            peers: vec![PeerEntry { id: b_id.clone() }],
            subscribed_topics: vec![],
        },
        HashSet::from([t1.clone()]),
        network.clone(),
    )
    .await
    .expect("construct A");

    let b = Node::new(
        b_id,
        NodeConfig {
            peers: vec![PeerEntry { id: a_id }],
            subscribed_topics: vec![],
        },
        HashSet::from([t1.clone()]),
        network.clone(),
    )
    .await
    .expect("construct B");

    let msg = Message::ping(t1, 13);
    a.send(b.id(), msg.clone()).await.expect("send Ok");

    // Wait until B has observed the delivery — guarantees the recv task
    // has had time to run on both sides before we snapshot A.
    await_delivery(&b, a.id(), &msg, Duration::from_secs(1))
        .await
        .expect("B receives A's emission");

    assert!(
        a.received_messages().is_empty(),
        "A does not see its own emission in its local snapshot",
    );
}

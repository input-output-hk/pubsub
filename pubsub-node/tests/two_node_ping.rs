mod common;

use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::{await_delivery, test_topic, two_node_fixture};
use pubsub_node::{InMemoryNetwork, Node, NodeConfig, PeerEntry, PeerId};

// US1 AS-1: A's peer set contains B; A sends Ping(42); B's record contains it.
#[tokio::test]
async fn ping_delivered_when_a_lists_b() {
    let fx = two_node_fixture().await;
    let msg = common::ping(test_topic(), 42);

    fx.a.send(fx.b.id(), msg.clone()).await.expect("send Ok");

    await_delivery(&fx.b, fx.a.id(), &msg, Duration::from_secs(1))
        .await
        .expect("delivery within 1s");

    let record = fx.b.received_messages();
    assert_eq!(record.len(), 1);
    assert_eq!(record[0].from, *fx.a.id());
    assert_eq!(record[0].message, msg);
}

// US1 AS-2: B's peer set does NOT contain A; A still sends and B still receives
// (trust-on-arrival per FR-003).
#[tokio::test]
async fn ping_delivered_trust_on_arrival() {
    let network = Arc::new(InMemoryNetwork::new());
    let a_id = PeerId::from_str("node-a").unwrap();
    let b_id = PeerId::from_str("node-b").unwrap();

    let a = Node::new(
        a_id.clone(),
        NodeConfig {
            peers: vec![PeerEntry { id: b_id.clone() }],
            subscribed_topics: vec![],
        },
        HashSet::from([test_topic()]),
        network.clone(),
    )
    .await
    .expect("A");

    // B's peer set is EMPTY — does not list A.
    let b = Node::new(
        b_id,
        NodeConfig {
            peers: vec![],
            subscribed_topics: vec![],
        },
        HashSet::from([test_topic()]),
        network.clone(),
    )
    .await
    .expect("B");

    let msg = common::ping(test_topic(), 7);
    a.send(b.id(), msg.clone()).await.expect("send Ok");

    await_delivery(&b, a.id(), &msg, Duration::from_secs(1))
        .await
        .expect("B still receives (trust-on-arrival)");

    let record = b.received_messages();
    assert_eq!(record.len(), 1);
    assert_eq!(record[0].from, *a.id());
    assert_eq!(record[0].message, msg);
}

// US1 AS-3 + spec Edge Cases bullet 1: Node A with empty peer list sending to
// an unregistered "ghost" id. Send resolves Ok(()) (drop-on-unregistered per
// FR-010), no panic, no undefined state.
#[tokio::test]
async fn empty_peer_set_cannot_originate() {
    let network = Arc::new(InMemoryNetwork::new());
    let a_id = PeerId::from_str("node-a").unwrap();

    let a = Node::new(
        a_id,
        NodeConfig {
            peers: vec![],
            subscribed_topics: vec![],
        },
        HashSet::from([test_topic()]),
        network.clone(),
    )
    .await
    .expect("A");

    let ghost = PeerId::from_str("ghost").unwrap();
    let outcome = a.send(&ghost, common::ping(test_topic(), 0)).await;
    assert!(outcome.is_ok(), "send to unregistered id is Ok per FR-010");

    // Briefly yield so any spurious recv processing would settle.
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(
        a.received_messages().is_empty(),
        "A should observe no deliveries",
    );
}

// SC-005 / FR-013 falsifiability: 100 sequential sends from A to B with a
// deterministic sequence (0..100). Asserts both (a) no duplication —
// b.received_messages().len() == 100 — and (b) no loss — every i in 0..100
// appears exactly once as a ReceivedDelivery with from = A.
//
// Sequence: the deterministic range `0..100` is the chosen N values per
// SC-005's reproducibility rule (CHK056); the seed convention does not apply
// since no PRNG is used.
#[tokio::test]
async fn ping_n_intact_across_100_sends() {
    const TOTAL: u64 = 100;

    let fx = two_node_fixture().await;
    let topic = test_topic();

    for i in 0..TOTAL {
        let msg = common::ping(topic.clone(), i);
        fx.a.send(fx.b.id(), msg.clone()).await.expect("send Ok");
        await_delivery(&fx.b, fx.a.id(), &msg, Duration::from_secs(1))
            .await
            .expect("delivery within 1s");
    }

    let record = fx.b.received_messages();

    // (a) no duplication: exactly 100 entries.
    assert_eq!(
        record.len() as u64,
        TOTAL,
        "duplication or loss — expected {TOTAL} entries, got {}",
        record.len(),
    );

    // (b) no loss: every i in 0..TOTAL appears exactly once, all from A.
    for i in 0..TOTAL {
        let expected = common::ping(topic.clone(), i);
        let count = record
            .iter()
            .filter(|d| d.from == *fx.a.id() && d.message == expected)
            .count();
        assert_eq!(
            count, 1,
            "expected exactly one Ping({i}) from A, got {count}"
        );
    }
}

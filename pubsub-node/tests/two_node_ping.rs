mod common;

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::{await_delivery, node_with, test_topic, two_node_fixture};
use pubsub_node::{InMemoryNetwork, InMemorySubscriptionRegistry, Origin, PeerId};

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
    assert_eq!(record[0].origin, Origin::Peer(fx.a.id().clone()));
    assert_eq!(record[0].message, msg);
}

// (Retired by 004-connections.) The 001/002 "trust-on-arrival" test —
// delivery to B independent of B's config peer set — encoded pre-connection
// receive semantics: B recorded A's message without having connected to it.
// Under the connection gate (FR-016) delivery is admitted only over an Active
// upstream the receiver dialed, so config-peer-independence is no longer the
// relevant property. The connection topology's delivery behavior is covered by
// `tests/connections.rs` and the reworked fixtures here.

// US1 AS-3 + spec Edge Cases bullet 1: Node A with empty peer list sending to
// an unregistered "ghost" id. Send resolves Ok(()) (drop-on-unregistered per
// FR-010), no panic, no undefined state.
#[tokio::test]
async fn empty_peer_set_cannot_originate() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let a = node_with(&registry, &network, "node-a", &[], &[test_topic()]).await;

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
            .filter(|d| d.origin == Origin::Peer(fx.a.id().clone()) && d.message == expected)
            .count();
        assert_eq!(
            count, 1,
            "expected exactly one Ping({i}) from A, got {count}"
        );
    }
}

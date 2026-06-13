mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{await_delivery, establish_upstreams, node_with, test_topic};
use pubsub_node::{InMemoryNetwork, InMemorySubscriptionRegistry, Node};

struct FourNodeStar {
    a: Node,
    b: Node,
    c: Node,
    d: Node,
}

async fn four_node_star_fixture() -> FourNodeStar {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let topics = [test_topic()];
    let a = node_with(
        &registry,
        &network,
        "node-a",
        &["node-b", "node-c", "node-d"],
        &topics,
    )
    .await;
    let b = node_with(&registry, &network, "node-b", &[], &topics).await;
    let c = node_with(&registry, &network, "node-c", &[], &topics).await;
    let d = node_with(&registry, &network, "node-d", &[], &topics).await;

    FourNodeStar { a, b, c, d }
}

// US2 AS-1: A's peer set = {B, C, D}; A sends Ping(1)→B, Ping(2)→C, Ping(3)→D
// sequentially. Each addressed peer receives exactly its own Ping; no
// cross-talk to non-addressed peers; A itself receives nothing in this
// outbound-only scenario.
#[tokio::test]
async fn four_node_star_isolates_addressed_pings() {
    let fx = four_node_star_fixture().await;
    let topic = test_topic();

    // Establishment preamble: each recipient dials A so A's addressed pings are
    // admitted over an Active upstream (FR-016).
    establish_upstreams(&fx.b, &[&fx.a], &topic).await;
    establish_upstreams(&fx.c, &[&fx.a], &topic).await;
    establish_upstreams(&fx.d, &[&fx.a], &topic).await;

    let m1 = common::ping(topic.clone(), 1);
    let m2 = common::ping(topic.clone(), 2);
    let m3 = common::ping(topic.clone(), 3);

    fx.a.send(fx.b.id(), m1.clone()).await.expect("send to B");
    fx.a.send(fx.c.id(), m2.clone()).await.expect("send to C");
    fx.a.send(fx.d.id(), m3.clone()).await.expect("send to D");

    await_delivery(&fx.b, fx.a.id(), &m1, Duration::from_secs(1))
        .await
        .expect("B delivery");
    await_delivery(&fx.c, fx.a.id(), &m2, Duration::from_secs(1))
        .await
        .expect("C delivery");
    await_delivery(&fx.d, fx.a.id(), &m3, Duration::from_secs(1))
        .await
        .expect("D delivery");

    let b_rec = fx.b.received_messages();
    let c_rec = fx.c.received_messages();
    let d_rec = fx.d.received_messages();

    assert_eq!(b_rec.len(), 1, "B receives exactly one ping");
    assert_eq!(c_rec.len(), 1, "C receives exactly one ping");
    assert_eq!(d_rec.len(), 1, "D receives exactly one ping");

    assert_eq!(b_rec[0].from, *fx.a.id());
    assert_eq!(b_rec[0].message, m1);
    assert_eq!(c_rec[0].from, *fx.a.id());
    assert_eq!(c_rec[0].message, m2);
    assert_eq!(d_rec[0].from, *fx.a.id());
    assert_eq!(d_rec[0].message, m3);

    // A is not a recipient of anything in this scenario.
    assert!(
        fx.a.received_messages().is_empty(),
        "A receives nothing in the AS-1 scenario",
    );
}

// (Retired by 004-connections.) "Inbound traffic independent of outbound peer
// set" asserted A records B/C/D's pings without A having connected to them —
// the pre-connection trust-on-arrival property. Under the gate (FR-016) A
// admits payload only over an Active upstream it dialed, so inbound delivery is
// now *dependent* on A's connection set, not independent of any peer set. The
// gated-delivery behavior is covered by `tests/connections.rs`.

// SC-002 conjunction: 4-node graph + 100 sequential sends + isolation. A sends
// round-robin to {B, C, D} with `i % 3` partitioning over i in 0..100.
// Asserts each peer's record is exactly its slice, attributed to A, with no
// duplicates and the three record sizes summing to 100.
//
// Sequence: the deterministic range `0..100` is the chosen N values per
// SC-005's reproducibility rule (CHK056) applied here to SC-002.
#[tokio::test]
async fn four_node_star_100_send_isolation() {
    const TOTAL: u64 = 100;

    let fx = four_node_star_fixture().await;
    let topic = test_topic();

    // Establishment preamble: each recipient dials A.
    establish_upstreams(&fx.b, &[&fx.a], &topic).await;
    establish_upstreams(&fx.c, &[&fx.a], &topic).await;
    establish_upstreams(&fx.d, &[&fx.a], &topic).await;

    for i in 0..TOTAL {
        let target = match i % 3 {
            0 => &fx.b,
            1 => &fx.c,
            _ => &fx.d,
        };
        let msg = common::ping(topic.clone(), i);
        fx.a.send(target.id(), msg.clone()).await.expect("send");
        await_delivery(target, fx.a.id(), &msg, Duration::from_secs(1))
            .await
            .expect("delivery");
    }

    let b_rec = fx.b.received_messages();
    let c_rec = fx.c.received_messages();
    let d_rec = fx.d.received_messages();

    let b_expected: Vec<u64> = (0..TOTAL).filter(|&i| i % 3 == 0).collect();
    let c_expected: Vec<u64> = (0..TOTAL).filter(|&i| i % 3 == 1).collect();
    let d_expected: Vec<u64> = (0..TOTAL).filter(|&i| i % 3 == 2).collect();

    assert_eq!(b_rec.len(), b_expected.len(), "B slice size");
    assert_eq!(c_rec.len(), c_expected.len(), "C slice size");
    assert_eq!(d_rec.len(), d_expected.len(), "D slice size");
    assert_eq!(
        b_rec.len() + c_rec.len() + d_rec.len(),
        usize::try_from(TOTAL).unwrap(),
        "all 100 sends accounted for",
    );

    for (name, record, expected) in [
        ("B", &b_rec, &b_expected),
        ("C", &c_rec, &c_expected),
        ("D", &d_rec, &d_expected),
    ] {
        assert_eq!(record.len(), expected.len(), "{name}: record size");
        for &expected_n in expected {
            let expected_msg = common::ping(topic.clone(), expected_n);
            let count = record
                .iter()
                .filter(|d| d.from == *fx.a.id() && d.message == expected_msg)
                .count();
            assert_eq!(
                count, 1,
                "{name}: expected exactly one Ping({expected_n}) from A, got {count}",
            );
        }
    }
}

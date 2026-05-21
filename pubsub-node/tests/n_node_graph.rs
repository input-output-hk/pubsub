mod common;

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::await_delivery;
use pubsub_node::{InMemoryNetwork, Message, Node, PeerEntry, PeerId, PeerListConfig};

struct FourNodeStar {
    a: Node,
    b: Node,
    c: Node,
    d: Node,
}

async fn four_node_star_fixture() -> FourNodeStar {
    let network = Arc::new(InMemoryNetwork::new());
    let a_id = PeerId::from_str("node-a").expect("valid id");
    let b_id = PeerId::from_str("node-b").expect("valid id");
    let c_id = PeerId::from_str("node-c").expect("valid id");
    let d_id = PeerId::from_str("node-d").expect("valid id");

    let a = Node::new(
        a_id,
        PeerListConfig {
            peers: vec![
                PeerEntry { id: b_id.clone() },
                PeerEntry { id: c_id.clone() },
                PeerEntry { id: d_id.clone() },
            ],
        },
        network.clone(),
    )
    .await
    .expect("construct A");
    let b = Node::new(b_id, PeerListConfig { peers: vec![] }, network.clone())
        .await
        .expect("construct B");
    let c = Node::new(c_id, PeerListConfig { peers: vec![] }, network.clone())
        .await
        .expect("construct C");
    let d = Node::new(d_id, PeerListConfig { peers: vec![] }, network)
        .await
        .expect("construct D");

    FourNodeStar { a, b, c, d }
}

// US2 AS-1: A's peer set = {B, C, D}; A sends Ping(1)→B, Ping(2)→C, Ping(3)→D
// sequentially. Each addressed peer receives exactly its own Ping; no
// cross-talk to non-addressed peers; A itself receives nothing in this
// outbound-only scenario.
#[tokio::test]
async fn four_node_star_isolates_addressed_pings() {
    let fx = four_node_star_fixture().await;

    fx.a.send(fx.b.id(), Message::Ping(1))
        .await
        .expect("send to B");
    fx.a.send(fx.c.id(), Message::Ping(2))
        .await
        .expect("send to C");
    fx.a.send(fx.d.id(), Message::Ping(3))
        .await
        .expect("send to D");

    await_delivery(&fx.b, fx.a.id(), &Message::Ping(1), Duration::from_secs(1))
        .await
        .expect("B delivery");
    await_delivery(&fx.c, fx.a.id(), &Message::Ping(2), Duration::from_secs(1))
        .await
        .expect("C delivery");
    await_delivery(&fx.d, fx.a.id(), &Message::Ping(3), Duration::from_secs(1))
        .await
        .expect("D delivery");

    let b_rec = fx.b.received_messages();
    let c_rec = fx.c.received_messages();
    let d_rec = fx.d.received_messages();

    assert_eq!(b_rec.len(), 1, "B receives exactly one ping");
    assert_eq!(c_rec.len(), 1, "C receives exactly one ping");
    assert_eq!(d_rec.len(), 1, "D receives exactly one ping");

    assert_eq!(b_rec[0].from, *fx.a.id());
    assert_eq!(b_rec[0].message, Message::Ping(1));
    assert_eq!(c_rec[0].from, *fx.a.id());
    assert_eq!(c_rec[0].message, Message::Ping(2));
    assert_eq!(d_rec[0].from, *fx.a.id());
    assert_eq!(d_rec[0].message, Message::Ping(3));

    // A is not a recipient of anything in this scenario.
    assert!(
        fx.a.received_messages().is_empty(),
        "A receives nothing in the AS-1 scenario",
    );
}

// US2 AS-2: same 4-node graph. B, C, D each send a Ping addressed to A. Their
// outbound peer sets are empty, but the in-memory network routes by registered
// id and FR-003 guarantees trust-on-arrival, so A receives all three pings —
// inbound traffic is independent of A's (or any node's) outbound peer set.
#[tokio::test]
async fn inbound_traffic_independent_of_outbound_peer_set() {
    let fx = four_node_star_fixture().await;

    fx.b.send(fx.a.id(), Message::Ping(10)).await.expect("B→A");
    fx.c.send(fx.a.id(), Message::Ping(20)).await.expect("C→A");
    fx.d.send(fx.a.id(), Message::Ping(30)).await.expect("D→A");

    await_delivery(&fx.a, fx.b.id(), &Message::Ping(10), Duration::from_secs(1))
        .await
        .expect("A receives from B");
    await_delivery(&fx.a, fx.c.id(), &Message::Ping(20), Duration::from_secs(1))
        .await
        .expect("A receives from C");
    await_delivery(&fx.a, fx.d.id(), &Message::Ping(30), Duration::from_secs(1))
        .await
        .expect("A receives from D");

    let a_rec = fx.a.received_messages();
    assert_eq!(a_rec.len(), 3, "A's record holds all three inbound pings");

    let from_b = a_rec
        .iter()
        .filter(|d| d.from == *fx.b.id() && d.message == Message::Ping(10))
        .count();
    let from_c = a_rec
        .iter()
        .filter(|d| d.from == *fx.c.id() && d.message == Message::Ping(20))
        .count();
    let from_d = a_rec
        .iter()
        .filter(|d| d.from == *fx.d.id() && d.message == Message::Ping(30))
        .count();

    assert_eq!(from_b, 1, "exactly one Ping(10) from B");
    assert_eq!(from_c, 1, "exactly one Ping(20) from C");
    assert_eq!(from_d, 1, "exactly one Ping(30) from D");
}

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

    for i in 0..TOTAL {
        let target = match i % 3 {
            0 => &fx.b,
            1 => &fx.c,
            _ => &fx.d,
        };
        fx.a.send(target.id(), Message::Ping(i))
            .await
            .expect("send");
        await_delivery(target, fx.a.id(), &Message::Ping(i), Duration::from_secs(1))
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
        let mut seen = vec![false; expected.len()];
        for delivery in record {
            assert_eq!(delivery.from, *fx.a.id(), "{name}: sender attribution");
            let Message::Ping(n) = delivery.message else {
                panic!("{name}: unexpected message variant: {:?}", delivery.message);
            };
            let idx = expected
                .iter()
                .position(|&e| e == n)
                .unwrap_or_else(|| panic!("{name}: Ping({n}) is outside this peer's slice"));
            assert!(!seen[idx], "{name}: duplicate Ping({n})");
            seen[idx] = true;
        }
        assert!(
            seen.iter().all(|s| *s),
            "{name}: missing N value(s) in expected slice",
        );
    }
}

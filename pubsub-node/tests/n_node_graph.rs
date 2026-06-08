mod common;

use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::{await_delivery, test_topic};
use pubsub_node::{InMemoryNetwork, Message, Node, NodeConfig, PeerEntry, PeerId};

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
        NodeConfig {
            peers: vec![
                PeerEntry { id: b_id.clone() },
                PeerEntry { id: c_id.clone() },
                PeerEntry { id: d_id.clone() },
            ],
            subscribed_topics: vec![],
        },
        HashSet::from([test_topic()]),
        network.clone(),
        common::shared_test_verifier(),
    )
    .await
    .expect("construct A");
    let b = Node::new(
        b_id,
        NodeConfig {
            peers: vec![],
            subscribed_topics: vec![],
        },
        HashSet::from([test_topic()]),
        network.clone(),
        common::shared_test_verifier(),
    )
    .await
    .expect("construct B");
    let c = Node::new(
        c_id,
        NodeConfig {
            peers: vec![],
            subscribed_topics: vec![],
        },
        HashSet::from([test_topic()]),
        network.clone(),
        common::shared_test_verifier(),
    )
    .await
    .expect("construct C");
    let d = Node::new(
        d_id,
        NodeConfig {
            peers: vec![],
            subscribed_topics: vec![],
        },
        HashSet::from([test_topic()]),
        network,
        common::shared_test_verifier(),
    )
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
    let topic = test_topic();

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

// US2 AS-2: same 4-node graph. B, C, D each send a Ping addressed to A. Their
// outbound peer sets are empty, but the in-memory network routes by registered
// id and FR-003 guarantees trust-on-arrival, so A receives all three pings —
// inbound traffic is independent of A's (or any node's) outbound peer set.
#[tokio::test]
async fn inbound_traffic_independent_of_outbound_peer_set() {
    let fx = four_node_star_fixture().await;
    let topic = test_topic();

    let m_b = common::ping(topic.clone(), 10);
    let m_c = common::ping(topic.clone(), 20);
    let m_d = common::ping(topic.clone(), 30);

    fx.b.send(fx.a.id(), m_b.clone()).await.expect("B→A");
    fx.c.send(fx.a.id(), m_c.clone()).await.expect("C→A");
    fx.d.send(fx.a.id(), m_d.clone()).await.expect("D→A");

    await_delivery(&fx.a, fx.b.id(), &m_b, Duration::from_secs(1))
        .await
        .expect("A receives from B");
    await_delivery(&fx.a, fx.c.id(), &m_c, Duration::from_secs(1))
        .await
        .expect("A receives from C");
    await_delivery(&fx.a, fx.d.id(), &m_d, Duration::from_secs(1))
        .await
        .expect("A receives from D");

    let a_rec = fx.a.received_messages();
    assert_eq!(a_rec.len(), 3, "A's record holds all three inbound pings");

    let from_b = a_rec
        .iter()
        .filter(|d| d.from == *fx.b.id() && d.message == m_b)
        .count();
    let from_c = a_rec
        .iter()
        .filter(|d| d.from == *fx.c.id() && d.message == m_c)
        .count();
    let from_d = a_rec
        .iter()
        .filter(|d| d.from == *fx.d.id() && d.message == m_d)
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
    let topic = test_topic();

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

// ---------------------------------------------------------------------------
// 002 US2 (Multi-Topic Node in N-Node Graph) — additive over the 001 tests
// above. Uses a dedicated fixture with a designated `emitter` node and four
// recipients A/B/C/D carrying mixed subscription sets.
// ---------------------------------------------------------------------------

struct FourNodeTopicsFixture {
    emitter: Node,
    a: Node,
    b: Node,
    c: Node,
    d: Node,
    t1: pubsub_node::TopicId,
    t2: pubsub_node::TopicId,
    t3: pubsub_node::TopicId,
}

// Subscriptions:
//   A = {T1}, B = {T1, T2}, C = {T2, T3}, D = {T3}
// `emitter` carries no subscription (it only sends); its peer set lists
// A/B/C/D so it can address each.
async fn four_node_topics_fixture() -> FourNodeTopicsFixture {
    let network = Arc::new(InMemoryNetwork::new());
    let t1 = pubsub_node::TopicId::from_str("t1").expect("valid topic id");
    let t2 = pubsub_node::TopicId::from_str("t2").expect("valid topic id");
    let t3 = pubsub_node::TopicId::from_str("t3").expect("valid topic id");

    let emitter_id = PeerId::from_str("emitter").expect("valid id");
    let a_id = PeerId::from_str("node-a").expect("valid id");
    let b_id = PeerId::from_str("node-b").expect("valid id");
    let c_id = PeerId::from_str("node-c").expect("valid id");
    let d_id = PeerId::from_str("node-d").expect("valid id");

    let emitter = Node::new(
        emitter_id,
        NodeConfig {
            peers: vec![
                PeerEntry { id: a_id.clone() },
                PeerEntry { id: b_id.clone() },
                PeerEntry { id: c_id.clone() },
                PeerEntry { id: d_id.clone() },
            ],
            subscribed_topics: vec![],
        },
        HashSet::new(),
        network.clone(),
        common::shared_test_verifier(),
    )
    .await
    .expect("construct emitter");

    let a = Node::new(
        a_id,
        NodeConfig {
            peers: vec![],
            subscribed_topics: vec![],
        },
        HashSet::from([t1.clone()]),
        network.clone(),
        common::shared_test_verifier(),
    )
    .await
    .expect("construct A");

    let b = Node::new(
        b_id,
        NodeConfig {
            peers: vec![],
            subscribed_topics: vec![],
        },
        HashSet::from([t1.clone(), t2.clone()]),
        network.clone(),
        common::shared_test_verifier(),
    )
    .await
    .expect("construct B");

    let c = Node::new(
        c_id,
        NodeConfig {
            peers: vec![],
            subscribed_topics: vec![],
        },
        HashSet::from([t2.clone(), t3.clone()]),
        network.clone(),
        common::shared_test_verifier(),
    )
    .await
    .expect("construct C");

    let d = Node::new(
        d_id,
        NodeConfig {
            peers: vec![],
            subscribed_topics: vec![],
        },
        HashSet::from([t3.clone()]),
        network,
        common::shared_test_verifier(),
    )
    .await
    .expect("construct D");

    FourNodeTopicsFixture {
        emitter,
        a,
        b,
        c,
        d,
        t1,
        t2,
        t3,
    }
}

// US2 AS-1: per-recipient snapshots equal the intersection of intended
// deliveries with each recipient's subscription set. Zero false-positives,
// zero false-negatives.
//
// For each topic T ∈ {T1, T2, T3} the emitter sends one Ping(idx, T) to each
// of A/B/C/D, where `idx` is the topic's 1-based index (1, 2, 3). Twelve
// emissions; per-recipient filtered receptions:
//   A ({T1})     → [Ping(1, T1)]
//   B ({T1, T2}) → [Ping(1, T1), Ping(2, T2)]
//   C ({T2, T3}) → [Ping(2, T2), Ping(3, T3)]
//   D ({T3})     → [Ping(3, T3)]
#[tokio::test]
async fn four_node_star_three_topics_filtering() {
    let fx = four_node_topics_fixture().await;
    let topics = [(&fx.t1, 1_u64), (&fx.t2, 2_u64), (&fx.t3, 3_u64)];

    for (topic, idx) in topics {
        let msg = common::ping(topic.clone(), idx);
        for recipient in [&fx.a, &fx.b, &fx.c, &fx.d] {
            fx.emitter
                .send(recipient.id(), msg.clone())
                .await
                .expect("send");
        }
    }

    // Await every expected on-topic delivery so the recv tasks are caught up
    // before we snapshot.
    for (recipient, expected) in [
        (&fx.a, vec![(&fx.t1, 1_u64)]),
        (&fx.b, vec![(&fx.t1, 1), (&fx.t2, 2)]),
        (&fx.c, vec![(&fx.t2, 2), (&fx.t3, 3)]),
        (&fx.d, vec![(&fx.t3, 3)]),
    ] {
        for (topic, n) in expected {
            let msg = common::ping(topic.clone(), n);
            await_delivery(recipient, fx.emitter.id(), &msg, Duration::from_secs(1))
                .await
                .expect("delivery");
        }
    }

    let a_rec = fx.a.received_messages();
    let b_rec = fx.b.received_messages();
    let c_rec = fx.c.received_messages();
    let d_rec = fx.d.received_messages();

    assert_eq!(a_rec.len(), 1, "A retains one (T1) delivery");
    assert_eq!(b_rec.len(), 2, "B retains T1 + T2");
    assert_eq!(c_rec.len(), 2, "C retains T2 + T3");
    assert_eq!(d_rec.len(), 1, "D retains one (T3) delivery");

    assert_eq!(a_rec[0].from, *fx.emitter.id());
    assert_eq!(a_rec[0].message, common::ping(fx.t1.clone(), 1));

    // Treat B / C as sets — the snapshot order is sender FIFO (emitter sent
    // T1 before T2 before T3), but AS-1 asserts membership, not order; AS-2
    // is the ordering-specific test.
    let b_msgs: Vec<&Message> = b_rec.iter().map(|d| &d.message).collect();
    assert!(b_msgs.contains(&&common::ping(fx.t1.clone(), 1)));
    assert!(b_msgs.contains(&&common::ping(fx.t2.clone(), 2)));

    let c_msgs: Vec<&Message> = c_rec.iter().map(|d| &d.message).collect();
    assert!(c_msgs.contains(&&common::ping(fx.t2.clone(), 2)));
    assert!(c_msgs.contains(&&common::ping(fx.t3.clone(), 3)));

    assert_eq!(d_rec[0].from, *fx.emitter.id());
    assert_eq!(d_rec[0].message, common::ping(fx.t3.clone(), 3));

    // Cross-talk negative assertions: no recipient sees a topic outside its
    // subscription set.
    assert!(a_rec
        .iter()
        .all(|d| common::message_topic(&d.message) == &fx.t1));
    assert!(b_rec.iter().all(|d| {
        let t = common::message_topic(&d.message);
        t == &fx.t1 || t == &fx.t2
    }));
    assert!(c_rec.iter().all(|d| {
        let t = common::message_topic(&d.message);
        t == &fx.t2 || t == &fx.t3
    }));
    assert!(d_rec
        .iter()
        .all(|d| common::message_topic(&d.message) == &fx.t3));
}

// US2 AS-2: per-sender FIFO ordering survives the topic filter. Emitter
// interleaves emissions on T2 and T3 across two rounds; each recipient's
// snapshot lists the *on-topic* messages in the order the emitter sent them.
//
// Two rounds of (T2, T3) interleaved:
//   round 0: Ping(0, T2), Ping(0, T3)
//   round 1: Ping(1, T2), Ping(1, T3)
//
// Each of these four messages is addressed to all of A/B/C/D in turn.
// Recipient-side expectations (filtered, order-preserving):
//   A ({T1})     → []
//   B ({T1, T2}) → [Ping(0,T2), Ping(1,T2)]
//   C ({T2, T3}) → [Ping(0,T2), Ping(0,T3), Ping(1,T2), Ping(1,T3)]
//   D ({T3})     → [Ping(0,T3), Ping(1,T3)]
#[tokio::test]
async fn four_node_star_topic_interleave_ordering() {
    let fx = four_node_topics_fixture().await;

    for round in 0..2_u64 {
        for topic in [&fx.t2, &fx.t3] {
            let msg = common::ping(topic.clone(), round);
            for recipient in [&fx.a, &fx.b, &fx.c, &fx.d] {
                fx.emitter
                    .send(recipient.id(), msg.clone())
                    .await
                    .expect("send");
            }
        }
    }

    // Await the LAST on-topic delivery for each recipient — since the
    // emitter→recipient channel is FIFO, this also guarantees all prior
    // messages have been processed.
    await_delivery(
        &fx.b,
        fx.emitter.id(),
        &common::ping(fx.t2.clone(), 1),
        Duration::from_secs(1),
    )
    .await
    .expect("B last-T2 delivery");
    await_delivery(
        &fx.c,
        fx.emitter.id(),
        &common::ping(fx.t3.clone(), 1),
        Duration::from_secs(1),
    )
    .await
    .expect("C last-T3 delivery");
    await_delivery(
        &fx.d,
        fx.emitter.id(),
        &common::ping(fx.t3.clone(), 1),
        Duration::from_secs(1),
    )
    .await
    .expect("D last-T3 delivery");

    // Brief settle window so A's recv task has a chance to drop the four
    // off-topic messages (T2/T3 are both off-topic for A) before we snapshot.
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(fx.a.received_messages().is_empty(), "A filters all out");

    let b_seq: Vec<Message> =
        fx.b.received_messages()
            .into_iter()
            .map(|d| d.message)
            .collect();
    assert_eq!(
        b_seq,
        vec![
            common::ping(fx.t2.clone(), 0),
            common::ping(fx.t2.clone(), 1),
        ],
        "B sequence (T2 only, in send order)",
    );

    let c_seq: Vec<Message> =
        fx.c.received_messages()
            .into_iter()
            .map(|d| d.message)
            .collect();
    assert_eq!(
        c_seq,
        vec![
            common::ping(fx.t2.clone(), 0),
            common::ping(fx.t3.clone(), 0),
            common::ping(fx.t2.clone(), 1),
            common::ping(fx.t3.clone(), 1),
        ],
        "C sequence (T2+T3 interleaved, in send order)",
    );

    let d_seq: Vec<Message> =
        fx.d.received_messages()
            .into_iter()
            .map(|d| d.message)
            .collect();
    assert_eq!(
        d_seq,
        vec![
            common::ping(fx.t3.clone(), 0),
            common::ping(fx.t3.clone(), 1),
        ],
        "D sequence (T3 only, in send order)",
    );
}

// US2 AS-3 / SC-002: 4-node × 3-topic × 100-emission isolation. The emitter
// sends 100 sequential Pings distributed across {T1, T2, T3} via `i % 3`
// partitioning (deterministic; no PRNG — matches 001's T020 / SC-005 / CHK056
// precedent). Each emission is addressed to each of A/B/C/D (400 deliveries
// in total). Every recipient's snapshot equals exactly
// `intended_deliveries(recipient) ∩ subscriptions(recipient)` — zero
// false-positives, zero false-negatives, across all 100 emissions.
//
// Sequence: the deterministic range `0..100` is the chosen N values; the
// topic for emission `i` is `topics_seq[i % 3]` (T1, T2, T3, T1, T2, …).
#[tokio::test]
async fn four_node_star_100_send_topic_isolation() {
    const TOTAL: u64 = 100;

    let fx = four_node_topics_fixture().await;
    let topics_seq = [&fx.t1, &fx.t2, &fx.t3];

    let recipients = [&fx.a, &fx.b, &fx.c, &fx.d];
    let recipient_subs: [&HashSet<&pubsub_node::TopicId>; 4] = [
        &HashSet::from([&fx.t1]),
        &HashSet::from([&fx.t1, &fx.t2]),
        &HashSet::from([&fx.t2, &fx.t3]),
        &HashSet::from([&fx.t3]),
    ];

    for i in 0..TOTAL {
        let topic = topics_seq[(i % 3) as usize];
        let msg = common::ping(topic.clone(), i);
        for recipient in recipients {
            fx.emitter
                .send(recipient.id(), msg.clone())
                .await
                .expect("send");
        }
    }

    // For each recipient, await the LAST on-topic emission so we know the
    // recv task has caught up to the end of the sequence (FIFO on the
    // emitter→recipient channel implies all earlier on-topic emissions have
    // also been processed).
    for (recipient, subs) in recipients.iter().zip(recipient_subs.iter()) {
        let last_on_topic = (0..TOTAL)
            .rev()
            .find(|&i| subs.contains(&topics_seq[(i % 3) as usize]));
        if let Some(i) = last_on_topic {
            let topic = topics_seq[(i % 3) as usize];
            await_delivery(
                recipient,
                fx.emitter.id(),
                &common::ping(topic.clone(), i),
                Duration::from_secs(2),
            )
            .await
            .expect("delivery");
        }
    }

    // Per-recipient set-equality assertion: the recipient's snapshot equals
    // {common::ping(topics_seq[i % 3], i) | i ∈ 0..TOTAL, topics_seq[i%3] ∈ subs}.
    for (name, recipient, subs) in [
        ("A", &fx.a, &recipient_subs[0]),
        ("B", &fx.b, &recipient_subs[1]),
        ("C", &fx.c, &recipient_subs[2]),
        ("D", &fx.d, &recipient_subs[3]),
    ] {
        let expected_ns: Vec<u64> = (0..TOTAL)
            .filter(|&i| subs.contains(&topics_seq[(i % 3) as usize]))
            .collect();
        let record = recipient.received_messages();
        let expected_len = expected_ns.len();
        let actual_len = record.len();
        assert_eq!(
            actual_len, expected_len,
            "{name}: snapshot size mismatch — expected {expected_len}, got {actual_len}",
        );

        // No false-positives: every delivery is from emitter, on an
        // in-subscription topic, and matches the expected (topic, N) pair.
        for delivery in &record {
            assert_eq!(
                delivery.from,
                *fx.emitter.id(),
                "{name}: sender attribution"
            );
            let topic = common::message_topic(&delivery.message);
            assert!(
                subs.contains(&topic),
                "{name}: off-topic delivery leaked through filter: {topic:?}",
            );
        }

        // No false-negatives: every expected (topic, N) appears exactly once
        // in the snapshot.
        for &n in &expected_ns {
            let topic = topics_seq[(n % 3) as usize];
            let expected_msg = common::ping(topic.clone(), n);
            let count = record
                .iter()
                .filter(|d| d.from == *fx.emitter.id() && d.message == expected_msg)
                .count();
            assert_eq!(
                count, 1,
                "{name}: expected exactly one Ping({n}) on {topic:?}, got {count}",
            );
        }
    }
}

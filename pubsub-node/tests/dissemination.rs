mod common;

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::{
    assert_no_new_deliveries, await_candidate_present, await_delivery, await_downstream,
    establish_mutual, establish_upstreams, node, ping,
};
use pubsub_node::{InMemoryNetwork, InMemorySubscriptionRegistry, Message, Node, Origin, TopicId};

fn topic(s: &str) -> TopicId {
    TopicId::from_str(s).expect("valid topic id")
}

const TIMEOUT: Duration = Duration::from_secs(2);

/// Poll `node`'s record until it holds a locally-published delivery
/// (`Origin::Local`) equal to `message`, or `timeout` elapses.
async fn await_local_record(node: &Node, message: &Message, timeout: Duration) {
    let start = tokio::time::Instant::now();
    loop {
        if node
            .received_messages()
            .iter()
            .any(|d| d.origin == Origin::Local && &d.message == message)
        {
            return;
        }
        assert!(
            start.elapsed() < timeout,
            "timed out waiting for the local record",
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

/// Poll `node`'s record until it holds a delivery of `message` from any origin,
/// or `timeout` elapses. Used where the delivering peer is not deterministic (a
/// full mesh — the first copy to arrive is recorded, the rest deduped).
async fn await_recorded(node: &Node, message: &Message, timeout: Duration) {
    let start = tokio::time::Instant::now();
    loop {
        if node
            .received_messages()
            .iter()
            .any(|d| &d.message == message)
        {
            return;
        }
        assert!(
            start.elapsed() < timeout,
            "timed out waiting for the record",
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

// US1 / SC-001, SC-006: a publisher with two downstream on a shared topic
// publishes a (proxy-authored) message — it records the message locally
// (`Origin::Local`) and fans it out verbatim to both downstream, each of which
// records it (attributed to the publishing node). An off-topic publish records
// nowhere — dropped at the publisher before any fan-out.
//
// The topology is a deliberate **star**: P holds d1 and d2 as downstream, but d1
// and d2 are NOT connected to each other. Each spoke dials only P
// (`ConnectToExplicit`); the all-candidates policy on a shared topic would
// instead build a full mesh, and a d1↔d2 edge would have the spokes relay P's
// message to one another. The star isolates first-hop fan-out from relay (US2).
#[tokio::test]
async fn publish_records_local_and_reaches_both_downstream() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let t = topic("t");

    // P dials nobody (it only accepts); each spoke dials only P.
    let p = node(&registry, &network, "p")
        .topic(&t)
        .dials_nobody()
        .build()
        .await;
    let d1 = node(&registry, &network, "d1")
        .topic(&t)
        .dials(&[(&p, &t)])
        .build()
        .await;
    let d2 = node(&registry, &network, "d2")
        .topic(&t)
        .dials(&[(&p, &t)])
        .build()
        .await;

    // P must know each spoke as a candidate to accept its dial; then each spoke
    // dials P (and only P).
    await_candidate_present(&p, &t, d1.id(), TIMEOUT)
        .await
        .expect("P knows d1");
    await_candidate_present(&p, &t, d2.id(), TIMEOUT)
        .await
        .expect("P knows d2");
    establish_upstreams(&d1, &[&p], &t).await;
    establish_upstreams(&d2, &[&p], &t).await;
    await_downstream(&p, d1.id(), &t, TIMEOUT)
        .await
        .expect("P holds d1 downstream");
    await_downstream(&p, d2.id(), &t, TIMEOUT)
        .await
        .expect("P holds d2 downstream");

    // SC-006: an off-topic publish (P is not a member of "other") must be dropped
    // at P — not recorded, not fanned out. Publish it FIRST, then the valid
    // message: the node drains its queue in FIFO order through one consumer, so
    // observing the valid publish everywhere proves the off-topic one (queued
    // before it, and dropped at P with no forward) has already been processed —
    // no wall-clock settle needed.
    let Message::Dissemination(off_topic) = ping(topic("other"), 2) else {
        unreachable!("ping yields Message::Dissemination");
    };
    p.publish(off_topic);

    // The valid, proxy-signed publish — both the US1 happy path AND the barrier
    // (authored by the shared test signer, not P: proxy/injection).
    let msg = ping(t.clone(), 1);
    let Message::Dissemination(signed) = msg.clone() else {
        unreachable!("ping yields Message::Dissemination");
    };
    p.publish(signed);

    // P records the valid message locally; both downstream receive the verbatim
    // forward, attributed to P (the delivering peer).
    await_local_record(&p, &msg, TIMEOUT).await;
    await_delivery(&d1, p.id(), &msg, TIMEOUT)
        .await
        .expect("d1 receives the published message");
    await_delivery(&d2, p.id(), &msg, TIMEOUT)
        .await
        .expect("d2 receives the published message");

    // Exactly one record per node — the valid message, attributed correctly. The
    // off-topic publish, drained before it, left no trace: an off-topic record (or
    // any forward of it) would push a count to 2.
    let p_rec = p.received_messages();
    assert_eq!(p_rec.len(), 1, "P records exactly the valid publish");
    assert_eq!(p_rec[0].origin, Origin::Local);
    assert_eq!(p_rec[0].message, msg);
    for (spoke, who) in [(&d1, "d1"), (&d2, "d2")] {
        let recs = spoke.received_messages();
        assert_eq!(recs.len(), 1, "{who} records exactly the valid message");
        assert_eq!(recs[0].origin, Origin::Peer(p.id().clone()));
        assert_eq!(recs[0].message, msg);
    }
}

// US2 / SC-002 (partial), SC-004: a received message is relayed onward through
// an **acyclic line** A→B→C. B dials A and C dials B (`ConnectToExplicit`), and
// there is NO A–C edge. Publishing at A reaches C *only* via B's relay (its sole
// delivery path), and B does not echo the message back to A (split-horizon). The
// topology is acyclic (a line is a tree), so propagation terminates without dedup
// — the cyclic-mesh "exactly once" case is asserted under US3 (T012).
#[tokio::test]
async fn relayed_message_traverses_acyclic_line() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let t = topic("t");

    // A dials nobody; B dials only A; C dials only B → A→B→C, no A–C edge.
    let a = node(&registry, &network, "a")
        .topic(&t)
        .dials_nobody()
        .build()
        .await;
    let b = node(&registry, &network, "b")
        .topic(&t)
        .dials(&[(&a, &t)])
        .build()
        .await;
    let c = node(&registry, &network, "c")
        .topic(&t)
        .dials(&[(&b, &t)])
        .build()
        .await;

    // A accepts B's dial; B accepts C's dial. Then establish each line edge.
    await_candidate_present(&a, &t, b.id(), TIMEOUT)
        .await
        .expect("A knows B");
    await_candidate_present(&b, &t, c.id(), TIMEOUT)
        .await
        .expect("B knows C");
    establish_upstreams(&b, &[&a], &t).await; // A→B edge
    establish_upstreams(&c, &[&b], &t).await; // B→C edge
    await_downstream(&a, b.id(), &t, TIMEOUT)
        .await
        .expect("A holds B downstream");
    await_downstream(&b, c.id(), &t, TIMEOUT)
        .await
        .expect("B holds C downstream");

    // Publish at A (proxy-signed). It flows A → B → C.
    let msg = ping(t.clone(), 1);
    let Message::Dissemination(signed) = msg.clone() else {
        unreachable!("ping yields Message::Dissemination");
    };
    a.publish(signed);

    await_local_record(&a, &msg, TIMEOUT).await;
    // B records the message delivered by A, then relays it to C; C records it
    // delivered by B — its sole delivery path (no A–C edge).
    await_delivery(&b, a.id(), &msg, TIMEOUT)
        .await
        .expect("B receives the message from A");
    await_delivery(&c, b.id(), &msg, TIMEOUT)
        .await
        .expect("C receives the relayed message via B");

    // The line is acyclic and A has no upstream (nobody dials A), so once C has
    // recorded via B the propagation has quiesced and A can never receive an echo
    // — no settle needed. Each node holds exactly one record; A keeps only its
    // local copy (no B→A echo — split-horizon), and C's only copy is B's relay.
    let a_rec = a.received_messages();
    assert_eq!(a_rec.len(), 1, "A holds only its own published copy");
    assert_eq!(a_rec[0].origin, Origin::Local, "no echo back to A");

    let b_rec = b.received_messages();
    assert_eq!(b_rec.len(), 1, "B records the message once");
    assert_eq!(b_rec[0].origin, Origin::Peer(a.id().clone()));

    let c_rec = c.received_messages();
    assert_eq!(c_rec.len(), 1, "C records the message once, via relay only");
    assert_eq!(
        c_rec[0].origin,
        Origin::Peer(b.id().clone()),
        "C's delivery path is B's relay, not a direct copy from A",
    );
}

// US3 / SC-002 (full), SC-003, SC-005: the FIRST cyclic test — a triangle of
// three mutually-connected members on one topic (the natural full mesh built by
// the all-candidates policy). One publishes; every member records the message
// exactly once and propagation terminates in a bounded number of forwards. This
// is safe only because dedup now suppresses the redundant relayed copies that
// circulate the cycle — the cyclic counterpart to US2's acyclic line.
#[tokio::test]
async fn triangle_mesh_records_once_and_terminates() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let t = topic("t");

    // Three members, the default all-candidates policy → each dials the others.
    let a = node(&registry, &network, "a").topic(&t).build().await;
    let b = node(&registry, &network, "b").topic(&t).build().await;
    let c = node(&registry, &network, "c").topic(&t).build().await;

    // Mutually connect all three pairs → a full bidirectional mesh (a cycle).
    establish_mutual(&a, &b, std::slice::from_ref(&t)).await;
    establish_mutual(&b, &c, std::slice::from_ref(&t)).await;
    establish_mutual(&a, &c, std::slice::from_ref(&t)).await;

    // A publishes (proxy-signed). In the mesh, redundant copies circulate every
    // cycle; dedup is what bounds it.
    let msg = ping(t.clone(), 1);
    let Message::Dissemination(signed) = msg.clone() else {
        unreachable!("ping yields Message::Dissemination");
    };
    a.publish(signed);

    // Every member records the message (the delivering peer is nondeterministic
    // in a mesh — assert presence, not origin).
    await_local_record(&a, &msg, TIMEOUT).await;
    await_recorded(&b, &msg, TIMEOUT).await;
    await_recorded(&c, &msg, TIMEOUT).await;

    // The redundant relays (b→c, c→b) must be deduped, not recorded — no node's
    // count grows as they circulate. (Exactly-once is proven deterministically by
    // the T010 state tests; this is the integration-level regression window: a
    // dedup regression would record a second copy and fail this fast.)
    assert_no_new_deliveries(&[&a, &b, &c], Duration::from_millis(100)).await;

    for (node, who) in [(&a, "A"), (&b, "B"), (&c, "C")] {
        let rec = node.received_messages();
        assert_eq!(rec.len(), 1, "{who} records the message exactly once");
        assert_eq!(rec[0].message, msg, "{who} records the published message");
    }
    assert_eq!(
        a.received_messages()[0].origin,
        Origin::Local,
        "the publisher's copy is local-origin",
    );
}

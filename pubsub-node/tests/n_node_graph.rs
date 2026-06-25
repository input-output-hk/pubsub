mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{
    assert_no_new_deliveries, await_candidate_present, await_delivery, await_downstream,
    establish_upstreams, node, ping, test_topic,
};
use pubsub_node::{InMemoryNetwork, InMemorySubscriptionRegistry, Message, Node, Origin};

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
            "timed out waiting for local record"
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

// SC-001 / SC-002: a four-node star — hub A holds spokes B, C, D as downstream
// (each spoke dials only A; the spokes never interconnect, so the topology is an
// acyclic tree, no dedup involved). A publishes one message; fan-out delivers it
// to all three spokes, each recording it exactly once (attributed to A), and A
// records its own copy with a local origin. No spoke receives a second copy
// (there is no spoke-to-spoke relay). This is US1 first-hop fan-out at N=4 — the
// controlled-star successor to the pre-006 addressed-`send` isolation suite,
// which fan-out (a received message is relayed onward) made obsolete.
#[tokio::test]
async fn four_node_star_publish_reaches_every_spoke_once() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let topic = test_topic();

    // Hub A dials nobody (accept-only); each spoke dials only A.
    let a = node(&registry, &network, "node-a")
        .topic(&topic)
        .dials_nobody()
        .build()
        .await;
    let b = node(&registry, &network, "node-b")
        .topic(&topic)
        .dials(&[(&a, &topic)])
        .build()
        .await;
    let c = node(&registry, &network, "node-c")
        .topic(&topic)
        .dials(&[(&a, &topic)])
        .build()
        .await;
    let d = node(&registry, &network, "node-d")
        .topic(&topic)
        .dials(&[(&a, &topic)])
        .build()
        .await;

    // A must know each spoke as a candidate to accept its dial; then each spoke
    // dials A, so A ends holding all three as downstream.
    for spoke in [&b, &c, &d] {
        await_candidate_present(&a, &topic, spoke.id(), TIMEOUT)
            .await
            .expect("A knows the spoke");
    }
    establish_upstreams(&b, &[&a], &topic).await;
    establish_upstreams(&c, &[&a], &topic).await;
    establish_upstreams(&d, &[&a], &topic).await;
    for spoke in [&b, &c, &d] {
        await_downstream(&a, spoke.id(), &topic, TIMEOUT)
            .await
            .expect("A holds the spoke downstream");
    }

    // A publishes once → fan-out reaches every spoke.
    let msg = ping(topic.clone(), 1);
    let Message::Dissemination(signed) = msg.clone() else {
        unreachable!("ping yields Message::Dissemination");
    };
    a.publish(signed);

    await_local_record(&a, &msg, TIMEOUT).await;
    for spoke in [&b, &c, &d] {
        await_delivery(spoke, a.id(), &msg, TIMEOUT)
            .await
            .expect("spoke receives the published message");
    }

    // Each spoke received A's publish (awaited above) and A recorded its local
    // copy; no further deliveries should appear — a node relays only to its
    // downstream, and the spokes have none, so there is no spoke-to-spoke relay.
    // No-trace non-event, backed by the 006 fan-out/relay state tests.
    assert_no_new_deliveries(&[&a, &b, &c, &d], Duration::from_millis(50)).await;
    for (spoke, who) in [(&b, "B"), (&c, "C"), (&d, "D")] {
        let rec = spoke.received_messages();
        assert_eq!(rec.len(), 1, "{who} records the message exactly once");
        assert_eq!(rec[0].origin, Origin::Peer(a.id().clone()), "{who} from A");
        assert_eq!(rec[0].message, msg, "{who} records the published message");
    }
    let a_rec = a.received_messages();
    assert_eq!(a_rec.len(), 1, "A records its own publish exactly once");
    assert_eq!(a_rec[0].origin, Origin::Local);
}

// (Retired by 004-connections.) "Inbound traffic independent of outbound peer
// set" asserted A records B/C/D's pings without A having connected to them —
// the pre-connection trust-on-arrival property. Under the gate (FR-016) A admits
// payload only over an Active upstream it dialed. The gated-delivery behavior is
// covered by `tests/connections.rs`.
//
// (Retired by 006-fanout-policy.) The addressed-`send` isolation suites
// (`four_node_star_isolates_addressed_pings`, `four_node_star_100_send_isolation`)
// asserted that a directed `send` reached only its addressee. Fan-out makes that
// obsolete — a dissemination message a peer receives is relayed onward to that
// peer's downstream — so the star is reworked above to assert fan-out coverage,
// and the cyclic exactly-once case is covered by `dissemination.rs`'s triangle.

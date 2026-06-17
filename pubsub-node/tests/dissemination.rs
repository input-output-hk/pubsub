mod common;

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::{await_delivery, await_downstream, establish_upstreams, node_with, ping};
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

// US1 / SC-001, SC-006: a publisher with two downstream on a shared topic
// publishes a (proxy-authored) message — it records the message locally
// (`Origin::Local`) and fans it out verbatim to both downstream, each of which
// records it (attributed to the publishing node). An off-topic publish records
// nowhere — dropped at the publisher before any fan-out.
#[tokio::test]
async fn publish_records_local_and_reaches_both_downstream() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let t = topic("t");

    // P (publisher) and its two subscribers, all members of t.
    let p = node_with(&registry, &network, "p", &[], std::slice::from_ref(&t)).await;
    let d1 = node_with(&registry, &network, "d1", &[], std::slice::from_ref(&t)).await;
    let d2 = node_with(&registry, &network, "d2", &[], std::slice::from_ref(&t)).await;

    // Establish through the real path: d1 and d2 each dial P, so P accepts them
    // and holds them as downstream (its fan-out destinations).
    establish_upstreams(&d1, &[&p], &t).await;
    establish_upstreams(&d2, &[&p], &t).await;
    await_downstream(&p, d1.id(), &t, TIMEOUT)
        .await
        .expect("P holds d1 downstream");
    await_downstream(&p, d2.id(), &t, TIMEOUT)
        .await
        .expect("P holds d2 downstream");

    // The published message is authored by the shared test signer, not by P —
    // proxy/injection: P publishes a message whose publisher is not itself.
    let msg = ping(t.clone(), 1);
    let Message::Signed(signed) = msg.clone() else {
        unreachable!("ping yields Message::Signed");
    };
    p.publish(signed);

    // P records it locally; both downstream receive the verbatim forward,
    // attributed to P (the delivering peer).
    await_local_record(&p, &msg, TIMEOUT).await;
    await_delivery(&d1, p.id(), &msg, TIMEOUT)
        .await
        .expect("d1 receives the published message");
    await_delivery(&d2, p.id(), &msg, TIMEOUT)
        .await
        .expect("d2 receives the published message");

    let p_rec = p.received_messages();
    assert_eq!(p_rec.len(), 1, "P records the publish exactly once");
    assert_eq!(p_rec[0].origin, Origin::Local);
    assert_eq!(p_rec[0].message, msg);

    // An off-topic publish (P is not a member of "other") is dropped at P — not
    // recorded, and not fanned out to either downstream.
    let Message::Signed(off_topic) = ping(topic("other"), 2) else {
        unreachable!("ping yields Message::Signed");
    };
    p.publish(off_topic);
    // Let the event loop drain the (dropped) publish before asserting nothing
    // changed anywhere.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        p.received_messages().len(),
        1,
        "off-topic publish is not recorded at P",
    );
    assert_eq!(
        d1.received_messages().len(),
        1,
        "off-topic publish is not forwarded to d1",
    );
    assert_eq!(
        d2.received_messages().len(),
        1,
        "off-topic publish is not forwarded to d2",
    );
}

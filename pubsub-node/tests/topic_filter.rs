mod common;

use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::{
    assert_no_new_deliveries, await_delivery, establish_upstreams, node_with,
    two_node_fixture_with_subscriptions,
};
use pubsub_node::{InMemoryNetwork, InMemorySubscriptionRegistry, Origin, TopicId};

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
    let msg = common::ping(t1.clone(), 42);

    fx.b.send(fx.a.id(), msg.clone()).await.expect("send Ok");

    await_delivery(&fx.a, fx.b.id(), &msg, Duration::from_secs(1))
        .await
        .expect("delivery within 1s");

    let record = fx.a.received_messages();
    assert_eq!(record.len(), 1, "A retains the on-topic delivery");
    assert_eq!(record[0].origin, Origin::Peer(fx.b.id().clone()));
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
    let off_topic_msg = common::ping(t2, 7);

    fx.b.send(fx.a.id(), off_topic_msg).await.expect("send Ok");

    // No-trace non-event: off-topic silent drop. The window fails fast if the
    // message ever lands; the drop itself is proven by the state test
    // `off_topic_message_leaves_state_unchanged`.
    assert_no_new_deliveries(&[&fx.a], Duration::from_millis(100)).await;
}

// US1 AS-3 / FR-009: A subscribed to {T1}; A emits Ping(13, T1) to B
// (a separate peer). A's own snapshot stays empty — only network-delivered
// messages enter the snapshot, never a Node's own emissions.
#[tokio::test]
async fn own_emission_not_in_local_snapshot() {
    let t1 = topic("t1");
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let a = node_with(
        &registry,
        &network,
        "node-a",
        &["node-b"],
        std::slice::from_ref(&t1),
    )
    .await;
    let b = node_with(
        &registry,
        &network,
        "node-b",
        &["node-a"],
        std::slice::from_ref(&t1),
    )
    .await;

    // Establishment preamble: B dials A so A's emission is admitted at B.
    establish_upstreams(&b, &[&a], &t1).await;

    let msg = common::ping(t1, 13);
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

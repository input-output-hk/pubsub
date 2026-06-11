mod common;

use std::collections::{BTreeSet, HashSet};
use std::str::FromStr;
use std::time::Duration;

use common::{
    assert_subscriptions, await_delivery, await_subscriptions, two_node_fixture_with_subscriptions,
    TwoNodeFixture,
};
use pubsub_node::{SubscriptionRegistryControl, TopicId};

// ---------------------------------------------------------------------------
// Runtime subscription behaviour.
//
// The node has no local subscribe/unsubscribe mutator (removed per ADR
// 0013/0014/0015): the subscription list is the single source of truth, and a
// node's accept-filter is derived from its own entry on the registry `watch`
// stream. These tests cover (a) the derived initial filter, (b) runtime
// narrowing driven through the registry, and (c) emission being decoupled from
// the emitter's own subscription set.
// ---------------------------------------------------------------------------

fn t1() -> TopicId {
    TopicId::from_str("t1").expect("valid topic id")
}

fn t2() -> TopicId {
    TopicId::from_str("t2").expect("valid topic id")
}

/// A subscribed to {T2}, B subscribed to {T1} (so A's inbound filter is
/// exercised; B's set is orthogonal). The fixture wires A↔B as peers and awaits
/// each node's subscription convergence before returning.
async fn fixture_a_t2_only() -> TwoNodeFixture {
    two_node_fixture_with_subscriptions(HashSet::from([t2()]), HashSet::from([t1()])).await
}

// A's initial subscription = {T2} (derived from its registry entry). B emits two
// pings — one on T1 (off-topic for A) and one on T2 (on-topic). A's snapshot
// contains exactly the T2 delivery.
#[tokio::test]
async fn initial_set_filters_inbound() {
    let fx = fixture_a_t2_only().await;

    let off_topic = common::ping(t1(), 1);
    let on_topic = common::ping(t2(), 2);
    fx.b.send(fx.a.id(), off_topic)
        .await
        .expect("send Ping(1, T1)");
    fx.b.send(fx.a.id(), on_topic.clone())
        .await
        .expect("send Ping(2, T2)");

    await_delivery(&fx.a, fx.b.id(), &on_topic, Duration::from_secs(1))
        .await
        .expect("A observes Ping(2, T2)");

    let record = fx.a.received_messages();
    assert_eq!(record.len(), 1, "A retains exactly the T2 delivery");
    assert_eq!(record[0].from, *fx.b.id());
    assert_eq!(record[0].message, common::ping(t2(), 2));

    assert_subscriptions(&fx.a, &[t2()]);
}

// Runtime narrowing via the registry: the subscription list — not a local
// mutator — drives the accept-filter. Reducing A's registry entry to {T2}
// converges A's subscriptions to {T2}, after which a T1 message is dropped.
// (Runtime *expansion* — adding a topic outside A's original watch scope — is
// deferred to feature 012; the watch is scoped to A's topics at watch time.)
#[tokio::test]
async fn registry_narrowing_updates_accept_filter() {
    let fx =
        two_node_fixture_with_subscriptions(HashSet::from([t1(), t2()]), HashSet::from([t1()]))
            .await;
    assert_subscriptions(&fx.a, &[t1(), t2()]);

    // Operator reduces A's subscription-list entry to {T2}.
    fx.registry
        .set_topics(fx.a.id().clone(), BTreeSet::from([t2()]))
        .await
        .expect("narrow A's registry entry to {T2}");
    await_subscriptions(&fx.a, &[t2()], Duration::from_secs(1))
        .await
        .expect("A's accept-filter converges to {T2}");

    // T1 is now off-topic for A → dropped; T2 still accepted.
    let off_topic = common::ping(t1(), 10);
    let on_topic = common::ping(t2(), 11);
    fx.b.send(fx.a.id(), off_topic).await.expect("send T1");
    fx.b.send(fx.a.id(), on_topic.clone())
        .await
        .expect("send T2");

    await_delivery(&fx.a, fx.b.id(), &on_topic, Duration::from_secs(1))
        .await
        .expect("A observes the T2 delivery");

    let record = fx.a.received_messages();
    assert_eq!(
        record.len(),
        1,
        "only the T2 message is accepted after narrowing to {{T2}}",
    );
    assert_eq!(record[0].message, on_topic);
}

// Emission is decoupled from the emitter's subscription set: A (subs = {T2}, T1
// NOT in set) sends Ping(99, T1) to B (subs = {T1}); the send resolves Ok and B
// receives the delivery.
#[tokio::test]
async fn decoupled_emission_succeeds_on_unsubscribed_topic() {
    let fx =
        two_node_fixture_with_subscriptions(HashSet::from([t2()]), HashSet::from([t1()])).await;
    let msg = common::ping(t1(), 99);

    fx.a.send(fx.b.id(), msg.clone())
        .await
        .expect("A sends on T1 despite not subscribing");

    await_delivery(&fx.b, fx.a.id(), &msg, Duration::from_secs(1))
        .await
        .expect("B receives the T1 delivery");

    let record = fx.b.received_messages();
    assert_eq!(record.len(), 1, "B retains exactly the one delivery");
    assert_eq!(record[0].from, *fx.a.id());
    assert_eq!(record[0].message, msg);
}

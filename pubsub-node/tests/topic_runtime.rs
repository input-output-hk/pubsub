mod common;

use std::collections::{BTreeSet, HashSet};
use std::str::FromStr;
use std::time::Duration;

use common::{
    assert_subscriptions, await_delivery, await_subscriptions, two_node_fixture_with_subscriptions,
    TwoNodeFixture,
};
use pubsub_node::{Origin, SubscriptionRegistryControl, TopicId};

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

/// A subscribed to {T2}; B subscribed to {T1, T2} so B is a T2 member A can
/// connect to (the gate admits payload only over an Active upstream the
/// receiver dialed). The fixture establishes A↔B on the shared T2; A's inbound
/// filter is still exercised — B's T1 send is not admitted at A, its T2 send
/// is.
async fn fixture_a_t2_only() -> TwoNodeFixture {
    two_node_fixture_with_subscriptions(HashSet::from([t2()]), HashSet::from([t1(), t2()])).await
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
    assert_eq!(record[0].origin, Origin::Peer(fx.b.id().clone()));
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
    // B subscribes {T1, T2} so it is a member A can connect to on both; A↔B is
    // established on both shared topics by the fixture. After A narrows to {T2}
    // its (now stale) T1 upstream still passes the gate, so the subscription
    // filter behind the gate is what drops the post-narrowing T1 message.
    let fx = two_node_fixture_with_subscriptions(
        HashSet::from([t1(), t2()]),
        HashSet::from([t1(), t2()]),
    )
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

// (Retired by 004-connections.) The decoupled-emission test required B to
// *record* a message A sent on a topic A is not subscribed to. Under the
// connection gate B admits payload only over an Active upstream it dialed to A
// on that topic — and B cannot dial A on a topic A is not a member of, so the
// delivery this test asserted can no longer occur. Sending is still decoupled
// from subscription (FR-023: `send` resolves regardless); only the *receive*
// side now requires a connection, which is what this test conflicts with.

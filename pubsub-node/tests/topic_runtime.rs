mod common;

use std::collections::HashSet;
use std::str::FromStr;
use std::time::Duration;

use common::{
    assert_subscriptions, await_delivery, two_node_fixture_with_subscriptions, TwoNodeFixture,
};
use pubsub_node::{Message, SubscribeOutcome, TopicId, UnsubscribeOutcome};

// ---------------------------------------------------------------------------
// US3 (Dynamic Subscription Transitions Take Immediate Effect)
//
// AS-1..AS-5 form a narrative sequence ("continuing from..."); each test is
// self-contained per Rust's per-test isolation. Helper builders below replay
// the prior steps so each test asserts the AS-specific property without
// depending on test ordering.
// ---------------------------------------------------------------------------

fn t1() -> TopicId {
    TopicId::from_str("t1").expect("valid topic id")
}

fn t2() -> TopicId {
    TopicId::from_str("t2").expect("valid topic id")
}

/// Construct the AS-1 setup: A subscribed to {T2}, B subscribed to {T1} (so
/// the inbound topic filter is exercised against A's set; B's set is
/// orthogonal). A's peer set lists B and vice versa per the fixture builder.
async fn fixture_a_t2_only() -> TwoNodeFixture {
    two_node_fixture_with_subscriptions(HashSet::from([t2()]), HashSet::from([t1()])).await
}

/// Drive A through the AS-1 sequence: B emits Ping(1, T1) then Ping(2, T2);
/// A's filter drops the first and retains the second. Returns once the
/// retained delivery is observable in A's snapshot.
async fn drive_to_as1_state(fx: &TwoNodeFixture) {
    let off_topic = Message::ping(t1(), 1);
    let on_topic = Message::ping(t2(), 2);

    fx.b.send(fx.a.id(), off_topic)
        .await
        .expect("send Ping(1, T1)");
    fx.b.send(fx.a.id(), on_topic.clone())
        .await
        .expect("send Ping(2, T2)");

    await_delivery(&fx.a, fx.b.id(), &on_topic, Duration::from_secs(1))
        .await
        .expect("AS-1: A observes Ping(2, T2)");
}

// US3 AS-1: A's initial subscription = {T2}. B emits two pings — one on T1
// (off-topic for A) and one on T2 (on-topic). A's snapshot contains exactly
// the T2 delivery.
#[tokio::test]
async fn initial_set_filters_inbound() {
    let fx = fixture_a_t2_only().await;
    drive_to_as1_state(&fx).await;

    let record = fx.a.received_messages();
    assert_eq!(record.len(), 1, "A retains exactly the T2 delivery");
    assert_eq!(record[0].from, *fx.b.id());
    assert_eq!(record[0].message, Message::ping(t2(), 2));

    assert_subscriptions(&fx.a, &[t2()]);
}

// US3 AS-2: Continuing from AS-1. A.subscribe(T1) returns Added; the
// snapshot is unchanged by the mutator call itself; the new subscription
// set is {T1, T2}.
#[tokio::test]
async fn subscribe_returns_added_and_updates_set() {
    let fx = fixture_a_t2_only().await;
    drive_to_as1_state(&fx).await;
    let before = fx.a.received_messages();

    let outcome = fx.a.subscribe(t1());

    assert_eq!(outcome, SubscribeOutcome::Added);
    assert_subscriptions(&fx.a, &[t1(), t2()]);
    assert_eq!(
        fx.a.received_messages(),
        before,
        "subscribe() must not mutate the received snapshot",
    );
}

// US3 AS-3: Continuing from AS-2 (subs = {T1, T2}). B emits Ping(3, T1);
// A retains it. Snapshot now holds [Ping(2, T2), Ping(3, T1)] — the
// previously-retained AS-1 entry plus the new T1 entry.
#[tokio::test]
async fn subscribe_makes_subsequent_message_visible() {
    let fx = fixture_a_t2_only().await;
    drive_to_as1_state(&fx).await;
    assert_eq!(fx.a.subscribe(t1()), SubscribeOutcome::Added);

    let new_t1 = Message::ping(t1(), 3);
    fx.b.send(fx.a.id(), new_t1.clone())
        .await
        .expect("send Ping(3, T1)");

    await_delivery(&fx.a, fx.b.id(), &new_t1, Duration::from_secs(1))
        .await
        .expect("AS-3: A observes Ping(3, T1) after subscribe");

    let messages: Vec<Message> =
        fx.a.received_messages()
            .into_iter()
            .map(|d| d.message)
            .collect();
    assert_eq!(
        messages,
        vec![Message::ping(t2(), 2), Message::ping(t1(), 3)],
        "snapshot retains AS-1 T2 entry and gains the new T1 entry",
    );
}

// US3 AS-4: Continuing from AS-3 (subs = {T1, T2}; snapshot has T2 + T1
// entries). A.unsubscribe(T1) returns Removed; the subscription set
// becomes {T2}; the snapshot stays unchanged — the previously-retained
// T1 entry is NOT retroactively removed (snapshot grows monotonically per
// FR-013 / SC-007).
#[tokio::test]
async fn unsubscribe_returns_removed_and_updates_set() {
    let fx = fixture_a_t2_only().await;
    drive_to_as1_state(&fx).await;
    assert_eq!(fx.a.subscribe(t1()), SubscribeOutcome::Added);

    let new_t1 = Message::ping(t1(), 3);
    fx.b.send(fx.a.id(), new_t1.clone())
        .await
        .expect("send Ping(3, T1)");
    await_delivery(&fx.a, fx.b.id(), &new_t1, Duration::from_secs(1))
        .await
        .expect("AS-4 setup: A observes Ping(3, T1)");

    let before = fx.a.received_messages();
    let outcome = fx.a.unsubscribe(t1());

    assert_eq!(outcome, UnsubscribeOutcome::Removed);
    assert_subscriptions(&fx.a, &[t2()]);
    assert_eq!(
        fx.a.received_messages(),
        before,
        "unsubscribe() must not mutate the received snapshot",
    );
}

// US3 AS-5: Continuing from AS-4 (subs = {T2}; snapshot has [T2, T1]).
// B emits Ping(4, T1) then Ping(5, T2). The new T1 is dropped; the new T2
// is retained; the previously-retained T1 from AS-3 REMAINS in the
// snapshot (monotonicity).
#[tokio::test]
async fn unsubscribe_makes_subsequent_message_dropped() {
    let fx = fixture_a_t2_only().await;
    drive_to_as1_state(&fx).await;
    assert_eq!(fx.a.subscribe(t1()), SubscribeOutcome::Added);

    let earlier_t1 = Message::ping(t1(), 3);
    fx.b.send(fx.a.id(), earlier_t1.clone())
        .await
        .expect("send Ping(3, T1)");
    await_delivery(&fx.a, fx.b.id(), &earlier_t1, Duration::from_secs(1))
        .await
        .expect("AS-5 setup: A observes Ping(3, T1)");
    assert_eq!(fx.a.unsubscribe(t1()), UnsubscribeOutcome::Removed);

    let new_t1 = Message::ping(t1(), 4);
    let new_t2 = Message::ping(t2(), 5);
    fx.b.send(fx.a.id(), new_t1)
        .await
        .expect("send Ping(4, T1)");
    fx.b.send(fx.a.id(), new_t2.clone())
        .await
        .expect("send Ping(5, T2)");

    await_delivery(&fx.a, fx.b.id(), &new_t2, Duration::from_secs(1))
        .await
        .expect("AS-5: A observes Ping(5, T2)");

    let messages: Vec<Message> =
        fx.a.received_messages()
            .into_iter()
            .map(|d| d.message)
            .collect();
    assert_eq!(
        messages,
        vec![
            Message::ping(t2(), 2),
            Message::ping(t1(), 3),
            Message::ping(t2(), 5),
        ],
        "snapshot keeps AS-3's T1 entry (monotonic) and adds the new T2; \
         the new Ping(4, T1) is dropped post-unsubscribe",
    );
}

// US3 AS-6 / SC-005: A subscribed to {T2}; re-subscribing returns
// AlreadyPresent without state change.
#[tokio::test]
async fn subscribe_idempotent_returns_already_present() {
    let fx = fixture_a_t2_only().await;
    let snapshot_before = fx.a.received_messages();

    let outcome = fx.a.subscribe(t2());

    assert_eq!(outcome, SubscribeOutcome::AlreadyPresent);
    assert_subscriptions(&fx.a, &[t2()]);
    assert_eq!(fx.a.received_messages(), snapshot_before);
}

// US3 AS-7 / SC-005: A subscribed to {T2}; unsubscribing a topic that
// isn't subscribed returns NotSubscribed without state change.
#[tokio::test]
async fn unsubscribe_idempotent_returns_not_subscribed() {
    let fx = fixture_a_t2_only().await;
    let snapshot_before = fx.a.received_messages();

    let outcome = fx.a.unsubscribe(t1());

    assert_eq!(outcome, UnsubscribeOutcome::NotSubscribed);
    assert_subscriptions(&fx.a, &[t2()]);
    assert_eq!(fx.a.received_messages(), snapshot_before);
}

// US3 AS-8 / FR-008: emission is decoupled from the emitter's subscription
// set. A (subs = {T2}, T1 NOT in set) sends Ping(99, T1) to B (subs = {T1});
// the send resolves Ok and B receives the delivery.
#[tokio::test]
async fn decoupled_emission_succeeds_on_unsubscribed_topic() {
    let fx =
        two_node_fixture_with_subscriptions(HashSet::from([t2()]), HashSet::from([t1()])).await;
    let msg = Message::ping(t1(), 99);

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

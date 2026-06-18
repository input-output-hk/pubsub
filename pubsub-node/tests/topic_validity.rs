//! Feature 013 / US2 integration: a node effectively subscribes only to topics
//! that are both in its subscription-list entry AND registered in the topic
//! registry. Unregistered subscription topics are ignored; registering or
//! removing a topic re-evaluates the effective set without restart.

mod common;

use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::{
    await_candidates, await_delivery, await_subscriptions, establish_upstreams, node_sharing, ping,
};
use pubsub_node::{
    InMemoryNetwork, InMemorySubscriptionRegistry, InMemoryTopicRegistry, PeerId,
    SubscriptionRegistryControl, TopicId, TopicRegistryControl,
};

fn topic(s: &str) -> TopicId {
    TopicId::from_str(s).expect("valid topic id")
}

fn peer(s: &str) -> PeerId {
    PeerId::from_str(s).expect("valid peer id")
}

// 014 SC-001/003/008/010: a node subscribed to {weather, ghosttopic} with only
// `weather` registered **strict-drops** `ghosttopic` — it is not subscribed, not
// a candidate, and cannot be connected. A `weather` message is accepted (no
// regression). Registering `ghosttopic` alone does **not** auto-promote it (013
// SC-004 retired); a fresh membership event after registration is required.
#[tokio::test]
async fn unregistered_subscription_topic_is_strict_dropped() {
    let network = Arc::new(InMemoryNetwork::new());
    let subs = Arc::new(InMemorySubscriptionRegistry::new());
    let topics = Arc::new(InMemoryTopicRegistry::new());

    subs.set_topics(
        peer("node-s"),
        [topic("weather"), topic("ghosttopic")]
            .into_iter()
            .collect(),
    )
    .await
    .unwrap();
    subs.set_topics(
        peer("node-b"),
        [topic("weather"), topic("ghosttopic")]
            .into_iter()
            .collect(),
    )
    .await
    .unwrap();
    // Only weather is a registered (legitimate) topic for now — open.
    topics
        .set_topic(topic("weather"), BTreeSet::new())
        .await
        .unwrap();

    let s = node_sharing(&subs, &topics, &network, "node-s", &[]).await;
    let b = node_sharing(&subs, &topics, &network, "node-b", &["node-s"]).await;

    // Strict drop: only the registered topic is effective.
    await_subscriptions(&s, &[topic("weather")], Duration::from_secs(1))
        .await
        .expect("only the registered topic is effective");
    // Candidate gating: b is a candidate on weather only — never on the
    // unregistered ghosttopic (the cross-registry invariant, 014).
    await_candidates(&s, &topic("weather"), &["node-b"], Duration::from_secs(1))
        .await
        .expect("weather candidate recorded");
    assert!(
        s.candidates(&topic("ghosttopic")).is_empty(),
        "no candidate on an unregistered topic",
    );

    // Establish + deliver on weather; ghosttopic cannot establish (not
    // subscribed) and a ghosttopic message is dropped.
    establish_upstreams(&s, &[&b], &topic("weather")).await;
    let on = ping(topic("weather"), 1);
    b.send(s.id(), on.clone()).await.expect("send weather");
    b.send(s.id(), ping(topic("ghosttopic"), 2))
        .await
        .expect("send ghosttopic");
    await_delivery(&s, b.id(), &on, Duration::from_secs(1))
        .await
        .expect("registered weather message accepted (no regression)");
    tokio::time::sleep(Duration::from_millis(50)).await; // settle
    assert_eq!(
        s.received_messages().len(),
        1,
        "ghosttopic is unregistered → dropped; only weather accepted",
    );

    // 014: registering ghosttopic ALONE does not auto-promote it (no SC-004).
    topics
        .set_topic(topic("ghosttopic"), BTreeSet::new())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        s.subscriptions().len(),
        1,
        "registration alone does not promote a previously-dropped subscription",
    );

    // A fresh membership event after registration brings it in (the chain
    // follower's ordering, modelled here by re-emitting the entry).
    subs.set_topics(peer("node-s"), [topic("weather")].into_iter().collect())
        .await
        .unwrap();
    subs.set_topics(
        peer("node-s"),
        [topic("weather"), topic("ghosttopic")]
            .into_iter()
            .collect(),
    )
    .await
    .unwrap();
    await_subscriptions(
        &s,
        &[topic("ghosttopic"), topic("weather")],
        Duration::from_secs(1),
    )
    .await
    .expect("a fresh membership event after registration makes ghosttopic effective");
}

// SC-004 (remove direction): removing a topic from the registry stops the node
// from accepting messages on it.
#[tokio::test]
async fn removing_a_topic_stops_acceptance() {
    let network = Arc::new(InMemoryNetwork::new());
    let subs = Arc::new(InMemorySubscriptionRegistry::new());
    let topics = Arc::new(InMemoryTopicRegistry::new());

    subs.set_topics(peer("node-s"), [topic("weather")].into_iter().collect())
        .await
        .unwrap();
    subs.set_topics(peer("node-b"), [topic("weather")].into_iter().collect())
        .await
        .unwrap();
    topics
        .set_topic(topic("weather"), BTreeSet::new())
        .await
        .unwrap();

    let s = node_sharing(&subs, &topics, &network, "node-s", &[]).await;
    let b = node_sharing(&subs, &topics, &network, "node-b", &["node-s"]).await;

    await_subscriptions(&s, &[topic("weather")], Duration::from_secs(1))
        .await
        .expect("weather effective");

    // Establishment preamble: s dials b on weather.
    establish_upstreams(&s, &[&b], &topic("weather")).await;

    let first = ping(topic("weather"), 1);
    b.send(s.id(), first.clone()).await.expect("send");
    await_delivery(&s, b.id(), &first, Duration::from_secs(1))
        .await
        .expect("accepted while registered");

    // Remove weather from the topic registry → no longer a legitimate topic.
    topics.remove_topic(topic("weather")).await.unwrap();
    await_subscriptions(&s, &[], Duration::from_secs(1))
        .await
        .expect("weather leaves the effective set once removed");

    let after = ping(topic("weather"), 2);
    b.send(s.id(), after).await.expect("send");
    tokio::time::sleep(Duration::from_millis(50)).await; // settle window
    assert_eq!(
        s.received_messages().len(),
        1,
        "the post-removal weather message is dropped",
    );
}

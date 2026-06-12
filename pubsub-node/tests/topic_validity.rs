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
    await_candidates, await_delivery, await_effective_subscriptions, ping, shared_test_verifier,
};
use pubsub_node::{
    InMemoryNetwork, InMemorySubscriptionRegistry, InMemoryTopicRegistry, Node, NodeConfig,
    PeerEntry, PeerId, SubscriptionRegistryControl, TopicId, TopicRegistryControl,
};

fn topic(s: &str) -> TopicId {
    TopicId::from_str(s).expect("valid topic id")
}

fn peer(s: &str) -> PeerId {
    PeerId::from_str(s).expect("valid peer id")
}

/// Build a node sharing the given registries, with config `peers`.
async fn node(
    subs: &Arc<InMemorySubscriptionRegistry>,
    topics: &Arc<InMemoryTopicRegistry>,
    network: &Arc<InMemoryNetwork>,
    id: &str,
    peers: &[&str],
) -> Node {
    let peers = peers.iter().map(|p| PeerEntry { id: peer(p) }).collect();
    Node::new(
        peer(id),
        NodeConfig { peers },
        network.clone(),
        shared_test_verifier(),
        subs.clone(),
        topics.clone(),
    )
    .await
    .expect("construct node")
}

// SC-003 + SC-004 + SC-010: a node subscribed to {weather, ghosttopic} with only
// `weather` registered effectively subscribes to `weather` alone; a `weather`
// message is accepted (no regression) and a `ghosttopic` one dropped. Registering
// `ghosttopic` later makes it effective with no restart.
#[tokio::test]
async fn unregistered_subscription_topic_is_ignored_until_registered() {
    let network = Arc::new(InMemoryNetwork::new());
    let subs = Arc::new(InMemorySubscriptionRegistry::new());
    let topics = Arc::new(InMemoryTopicRegistry::new());

    // node-s subscribes both topics; node-b (the sender) is a weather member.
    subs.set_topics(
        peer("node-s"),
        [topic("weather"), topic("ghosttopic")]
            .into_iter()
            .collect(),
    )
    .await
    .unwrap();
    subs.set_topics(peer("node-b"), [topic("weather")].into_iter().collect())
        .await
        .unwrap();
    // Only weather is a registered (legitimate) topic for now — open.
    topics
        .set_topic(topic("weather"), BTreeSet::new())
        .await
        .unwrap();

    let s = node(&subs, &topics, &network, "node-s", &[]).await;
    let b = node(&subs, &topics, &network, "node-b", &["node-s"]).await;

    // ghosttopic is subscribed but not registered → excluded from the effective set.
    await_effective_subscriptions(&s, &[topic("weather")], Duration::from_secs(1))
        .await
        .expect("only the registered topic is effective");
    // Membership still folds into candidates regardless of the topic registry
    // (SC-009: the projections are independent): s sees b as a weather candidate.
    await_candidates(&s, &topic("weather"), &["node-b"], Duration::from_secs(1))
        .await
        .expect("candidate set is unaffected by the topic registry");

    let on = ping(topic("weather"), 1);
    let off = ping(topic("ghosttopic"), 2);
    b.send(s.id(), on.clone()).await.expect("send weather");
    b.send(s.id(), off).await.expect("send ghosttopic");

    await_delivery(&s, b.id(), &on, Duration::from_secs(1))
        .await
        .expect("registered weather message accepted (no regression, SC-010)");
    tokio::time::sleep(Duration::from_millis(50)).await; // settle for any ghosttopic processing
    assert_eq!(
        s.received_messages().len(),
        1,
        "ghosttopic is unregistered → dropped; only weather accepted",
    );

    // Register ghosttopic → it becomes effective without restart, and a
    // subsequent ghosttopic message is accepted.
    topics
        .set_topic(topic("ghosttopic"), BTreeSet::new())
        .await
        .unwrap();
    await_effective_subscriptions(
        &s,
        &[topic("ghosttopic"), topic("weather")],
        Duration::from_secs(1),
    )
    .await
    .expect("ghosttopic becomes effective once registered (SC-004)");

    let now_ok = ping(topic("ghosttopic"), 3);
    b.send(s.id(), now_ok.clone())
        .await
        .expect("send ghosttopic");
    await_delivery(&s, b.id(), &now_ok, Duration::from_secs(1))
        .await
        .expect("ghosttopic message accepted once registered");
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

    let s = node(&subs, &topics, &network, "node-s", &[]).await;
    let b = node(&subs, &topics, &network, "node-b", &["node-s"]).await;

    await_effective_subscriptions(&s, &[topic("weather")], Duration::from_secs(1))
        .await
        .expect("weather effective");

    let first = ping(topic("weather"), 1);
    b.send(s.id(), first.clone()).await.expect("send");
    await_delivery(&s, b.id(), &first, Duration::from_secs(1))
        .await
        .expect("accepted while registered");

    // Remove weather from the topic registry → no longer a legitimate topic.
    topics.remove_topic(topic("weather")).await.unwrap();
    await_effective_subscriptions(&s, &[], Duration::from_secs(1))
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

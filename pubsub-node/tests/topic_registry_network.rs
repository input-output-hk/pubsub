//! Feature 013 / US4 integration: a network of in-memory nodes sharing one
//! subscription registry and one topic registry enforces the same topic
//! legitimacy and publisher-authorization decisions, with no operator.

mod common;

use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::{
    await_delivery, await_subscriptions, build_signed_message_simple, establish_upstreams,
    node_sharing, ping, test_signer,
};
use pubsub_node::{
    InMemoryNetwork, InMemorySubscriptionRegistry, InMemoryTopicRegistry, MessagePayload, PeerId,
    PrivateKey, Signer, SubscriptionRegistryControl, TestSigner, TopicId, TopicRegistryControl,
};

fn topic(s: &str) -> TopicId {
    TopicId::from_str(s).expect("valid topic id")
}

fn peer(s: &str) -> PeerId {
    PeerId::from_str(s).expect("valid peer id")
}

// SC-003 / SC-005 / SC-008: three nodes share one subscription registry and one
// topic registry. Each node's effective set is its subscription-list topics
// intersected with the registered topics (node-c's unregistered `ghosttopic` is
// dropped); a `weather` message from the authorized publisher is accepted by
// every weather subscriber, while one from an unauthorized publisher is dropped
// by all.
//
// Under 006 fan-out the three weather members form a mesh and relay the payload
// among themselves; dedup (US3) bounds it so each node records the authorized
// message exactly once — the "exactly one record" assertions below verify that
// post-fan-out behavior, and the unauthorized message is dropped at every node
// before any fan-out.
#[tokio::test]
async fn network_enforces_legitimacy_and_authorization_uniformly() {
    let network = Arc::new(InMemoryNetwork::new());
    let subs = Arc::new(InMemorySubscriptionRegistry::new());
    let topics = Arc::new(InMemoryTopicRegistry::new());

    // Subscription list (the mocked on-chain membership).
    subs.set_topics(peer("node-a"), [topic("weather")].into_iter().collect())
        .await
        .unwrap();
    subs.set_topics(
        peer("node-b"),
        [topic("weather"), topic("sports")].into_iter().collect(),
    )
    .await
    .unwrap();
    subs.set_topics(
        peer("node-c"),
        [topic("weather"), topic("ghosttopic")]
            .into_iter()
            .collect(),
    )
    .await
    .unwrap();

    // Topic registry: weather restricted to the shared test signer; sports open;
    // ghosttopic NOT registered (an illegitimate topic).
    topics
        .set_topic(
            topic("weather"),
            BTreeSet::from([test_signer().public_key()]),
        )
        .await
        .unwrap();
    topics
        .set_topic(topic("sports"), BTreeSet::new())
        .await
        .unwrap();

    let a = node_sharing(&subs, &topics, &network, "node-a", &["node-b"]).await;
    let b = node_sharing(&subs, &topics, &network, "node-b", &["node-a", "node-c"]).await;
    let c = node_sharing(&subs, &topics, &network, "node-c", &["node-b"]).await;

    // Per-node effective sets: the registered subset of each subscription entry.
    await_subscriptions(&a, &[topic("weather")], Duration::from_secs(1))
        .await
        .expect("a effective = {weather}");
    await_subscriptions(
        &b,
        &[topic("sports"), topic("weather")],
        Duration::from_secs(1),
    )
    .await
    .expect("b effective = {sports, weather}");
    await_subscriptions(&c, &[topic("weather")], Duration::from_secs(1))
        .await
        .expect("c effective = {weather} — ghosttopic is unregistered (SC-003)");

    // Establishment preamble: a and c dial b (the publisher) on weather, so b's
    // messages are admitted over an Active upstream (FR-016).
    establish_upstreams(&a, &[&b], &topic("weather")).await;
    establish_upstreams(&c, &[&b], &topic("weather")).await;

    // An authorized weather message (the shared test signer, which `ping` uses)
    // is accepted by every weather subscriber; one from an UNauthorized publisher
    // is dropped by all. Send the forged message first, then the authorized one
    // as a FIFO barrier on each b→{a,c} channel: when the authorized delivery
    // lands, the earlier forged message has already been processed and dropped.
    let outsider = TestSigner::new(PrivateKey::new(b"unauthorized-publisher".to_vec()));
    let forged = build_signed_message_simple(&outsider, topic("weather"), MessagePayload::Ping(2));
    let authorized = ping(topic("weather"), 1);
    b.send(a.id(), forged.clone()).await.unwrap();
    b.send(c.id(), forged).await.unwrap();
    b.send(a.id(), authorized.clone()).await.unwrap();
    b.send(c.id(), authorized.clone()).await.unwrap();
    await_delivery(&a, b.id(), &authorized, Duration::from_secs(1))
        .await
        .expect("a accepts the authorized weather message");
    await_delivery(&c, b.id(), &authorized, Duration::from_secs(1))
        .await
        .expect("c accepts the authorized weather message");
    assert_eq!(
        a.received_messages().len(),
        1,
        "a drops the unauthorized-publisher message; only the authorized one lands",
    );
    assert_eq!(
        c.received_messages().len(),
        1,
        "c drops the unauthorized-publisher message; only the authorized one lands",
    );
}

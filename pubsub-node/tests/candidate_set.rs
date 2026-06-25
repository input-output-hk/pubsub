mod common;

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::{
    alias_signer, await_candidates, await_delivery, await_synced, establish_upstreams, node_with,
    ping, shared_test_verifier,
};
use pubsub_node::{
    AcceptFromAllCandidates, ConnectToAllCandidates, ForwardToAll, InMemoryNetwork,
    InMemorySubscriptionRegistry, InMemoryTopicRegistry, Node, NodeConfig, PeerId,
    SubscriptionRegistryControl, TopicId,
};

fn topic(s: &str) -> TopicId {
    TopicId::from_str(s).expect("valid topic id")
}

fn peer(s: &str) -> PeerId {
    PeerId::from_str(s).expect("valid peer id")
}

// FR-018: a node whose id has no subscription-list entry constructs cleanly and
// derives empty state from the registry stream. The node starts empty and folds
// the membership stream; with no entry, the stream replays nothing, so the node
// stays at an empty subscription set and empty candidate sets — the "registered
// but not yet present" / "initializing" posture, not a hard construction error.
#[tokio::test]
async fn node_with_no_registry_entry_derives_empty_state() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new()); // empty — no entry for ghost
    let topic_registry = Arc::new(InMemoryTopicRegistry::new()); // empty — no registered topics
    let node = Node::new(
        peer("ghost"),
        NodeConfig { peers: vec![] },
        network,
        alias_signer("ghost"),
        shared_test_verifier(),
        registry,
        topic_registry,
        Arc::new(ConnectToAllCandidates),
        Arc::new(ForwardToAll),
        Arc::new(AcceptFromAllCandidates),
    )
    .await
    .expect("construction succeeds even with no registry entry");

    // With no registry entry the node never emits a populating event, so a
    // populated set is not an observable to await. Sync readiness is: await
    // `is_synced` (both empty registry snapshots folded) — that the cold-start
    // replay has drained — then assert the state stayed empty.
    await_synced(&node, Duration::from_secs(1))
        .await
        .expect("node syncs after folding both empty registry snapshots");

    assert!(
        node.subscriptions().is_empty(),
        "no registry entry -> empty subscription set",
    );
    assert!(
        node.candidates(&topic("t1")).is_empty(),
        "no registry entry -> no candidates",
    );
}

// SC-007: a node's effective topics are its registry entry — it accepts a
// message on its registered topic and drops one on a topic it is not
// registered for (the filter is sourced from the registry, not config).
#[tokio::test]
async fn effective_topics_come_from_registry_entry() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let s = node_with(&registry, &network, "node-s", &[], &[topic("t1")]).await;
    let b = node_with(&registry, &network, "node-b", &["node-s"], &[topic("t1")]).await;

    // Establishment preamble: s dials b on t1 so b's t1 message is admitted.
    establish_upstreams(&s, &[&b], &topic("t1")).await;

    let on_topic = ping(topic("t1"), 1);
    let off_topic = ping(topic("t2"), 2);
    // Send the off-topic message first, then the on-topic one as a FIFO barrier:
    // both ride the single b→s channel, so when the on-topic delivery lands the
    // earlier off-topic message has already been processed (and dropped).
    b.send(s.id(), off_topic).await.expect("send t2");
    b.send(s.id(), on_topic.clone()).await.expect("send t1");

    await_delivery(&s, b.id(), &on_topic, Duration::from_secs(1))
        .await
        .expect("t1 message delivered");

    let record = s.received_messages();
    assert_eq!(
        record.len(),
        1,
        "only the registered-topic (t1) message is accepted; t2 is dropped",
    );
    assert_eq!(record[0].message, on_topic);
}

// SC-009 / FR-017: the registry-derived candidate set is distinct from the
// config bootstrap `peers` and does not alter it.
#[tokio::test]
async fn candidate_set_is_distinct_from_config_peers() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    // node-b is a t1 member in the registry (a candidate), but not a config peer.
    registry
        .set_topics(peer("node-b"), [topic("t1")].into_iter().collect())
        .await
        .unwrap();
    // node-s has a config bootstrap peer "boot-x" (not a t1 member) and is registered for t1.
    let s = node_with(&registry, &network, "node-s", &["boot-x"], &[topic("t1")]).await;

    await_candidates(&s, &topic("t1"), &["node-b"], Duration::from_secs(1))
        .await
        .expect("candidate set converges to the registry member node-b");

    let bootstrap: Vec<String> = s.peers().iter().map(|p| p.id.to_string()).collect();
    assert_eq!(
        bootstrap,
        vec!["boot-x".to_string()],
        "config bootstrap peers are unchanged and distinct from the candidate set",
    );
}

// US4: an in-memory network of nodes sharing one registry discovers itself —
// each node's candidate set converges to the topic-scoped, self-excluded view
// of the others.
#[tokio::test]
async fn network_discovers_itself_from_shared_registry() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let a = node_with(&registry, &network, "node-a", &[], &[topic("t1")]).await;
    let b = node_with(
        &registry,
        &network,
        "node-b",
        &[],
        &[topic("t1"), topic("t2")],
    )
    .await;
    let c = node_with(&registry, &network, "node-c", &[], &[topic("t2")]).await;

    let timeout = Duration::from_secs(1);
    await_candidates(&a, &topic("t1"), &["node-b"], timeout)
        .await
        .expect("a sees b on t1");
    await_candidates(&b, &topic("t1"), &["node-a"], timeout)
        .await
        .expect("b sees a on t1");
    await_candidates(&b, &topic("t2"), &["node-c"], timeout)
        .await
        .expect("b sees c on t2");
    await_candidates(&c, &topic("t2"), &["node-b"], timeout)
        .await
        .expect("c sees b on t2");

    // Scoping: a watches only t1, so it has no candidates on t2.
    assert!(
        a.candidates(&topic("t2")).is_empty(),
        "a does not watch t2, so no t2 candidates",
    );
    // Self-exclusion: a is never its own candidate.
    assert!(
        !a.candidates(&topic("t1")).contains(a.id()),
        "a is excluded from its own candidate set",
    );
}

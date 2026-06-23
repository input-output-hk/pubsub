mod common;

use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::{
    alias_signer, await_candidates, await_delivery, await_downstream, await_peer_forgotten,
    await_upstream_active, await_upstream_present, establish_mutual, establish_upstreams,
    node_with, ping, shared_test_verifier, tampered_ping, trigger_setup,
};
use pubsub_node::{
    AcceptFromAllCandidates, ConnectToAllCandidates, ForwardToAll, InMemoryNetwork,
    InMemorySubscriptionRegistry, InMemoryTopicRegistry, NetworkError, Node, NodeConfig, NodeError,
    Origin, PeerId, SubscriptionRegistryControl, TopicId, TopicRegistryControl, UpstreamState,
};

fn topic(s: &str) -> TopicId {
    TopicId::from_str(s).expect("valid topic id")
}

fn peer(s: &str) -> PeerId {
    PeerId::from_str(s).expect("valid peer id")
}

const TIMEOUT: Duration = Duration::from_secs(2);

// SC-001 / US1-AS1: N nodes sharing a topic converge to the full bidirectional
// per-topic graph — each holds the other N−1 as Active upstreams and as
// downstreams, with no further connection activity.
#[tokio::test]
async fn full_bidirectional_graph_for_three_nodes() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let t = topic("t");

    let a = node_with(&registry, &network, "a", &[], std::slice::from_ref(&t)).await;
    let b = node_with(&registry, &network, "b", &[], std::slice::from_ref(&t)).await;
    let c = node_with(&registry, &network, "c", &[], std::slice::from_ref(&t)).await;

    // Each node's candidate view converges to the other two before setup.
    await_candidates(&a, &t, &["b", "c"], TIMEOUT)
        .await
        .unwrap();
    await_candidates(&b, &t, &["a", "c"], TIMEOUT)
        .await
        .unwrap();
    await_candidates(&c, &t, &["a", "b"], TIMEOUT)
        .await
        .unwrap();

    trigger_setup(&a);
    trigger_setup(&b);
    trigger_setup(&c);

    for (node, others) in [(&a, ["b", "c"]), (&b, ["a", "c"]), (&c, ["a", "b"])] {
        for other in others {
            await_upstream_active(node, &peer(other), &t, TIMEOUT)
                .await
                .unwrap_or_else(|_| panic!("upstream to {other} active"));
            await_downstream(node, &peer(other), &t, TIMEOUT)
                .await
                .unwrap_or_else(|_| panic!("downstream from {other}"));
        }
    }

    // Exactly N−1 of each, all upstreams Active — no stragglers, no extras.
    for node in [&a, &b, &c] {
        let up = node.upstream_connections();
        assert_eq!(up.len(), 2, "two upstreams");
        assert!(
            up.iter()
                .all(|(_, _, state)| *state == UpstreamState::Active),
            "every upstream Active",
        );
        assert_eq!(node.downstream_connections().len(), 2, "two downstreams");
    }
}

// US1-AS3 / FR-008: with only a subset of members known at setup, the node
// connects to exactly that subset; a later membership update (a new member)
// does not establish anything — selection runs only on a setup event.
#[tokio::test]
async fn partial_convergence_stays_static_across_membership_change() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let t = topic("t");

    let a = node_with(&registry, &network, "a", &[], std::slice::from_ref(&t)).await;
    let _b = node_with(&registry, &network, "b", &[], std::slice::from_ref(&t)).await;
    await_candidates(&a, &t, &["b"], TIMEOUT).await.unwrap();

    trigger_setup(&a);
    await_upstream_active(&a, &peer("b"), &t, TIMEOUT)
        .await
        .unwrap();

    // A third member appears after a's single setup.
    let _c = node_with(&registry, &network, "c", &[], std::slice::from_ref(&t)).await;
    await_candidates(&a, &t, &["b", "c"], TIMEOUT)
        .await
        .unwrap();

    // a folded c into its candidate view but never dialed it — membership alone
    // does not establish.
    let up = a.upstream_connections();
    assert_eq!(up.len(), 1, "still just the one upstream");
    assert!(
        up.iter().all(|(p, _, _)| p != &peer("c")),
        "c was never dialed without a new setup event",
    );
}

// US1-AS4: a node that issued no requests of its own still accepts an inbound
// request from a member it knows — it ends with a downstream and no upstream.
#[tokio::test]
async fn node_that_dialed_nothing_still_accepts_inbound() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let t = topic("t");

    let a = node_with(&registry, &network, "a", &[], std::slice::from_ref(&t)).await;
    let b = node_with(&registry, &network, "b", &[], std::slice::from_ref(&t)).await;
    // a knows b as a member (so it can validate b's request), but a is never
    // triggered — only b dials.
    await_candidates(&a, &t, &["b"], TIMEOUT).await.unwrap();
    await_candidates(&b, &t, &["a"], TIMEOUT).await.unwrap();

    trigger_setup(&b);
    await_upstream_active(&b, &peer("a"), &t, TIMEOUT)
        .await
        .unwrap();
    await_downstream(&a, &peer("b"), &t, TIMEOUT).await.unwrap();

    assert!(
        a.upstream_connections().is_empty(),
        "a issued no requests of its own",
    );
    assert_eq!(
        a.downstream_connections().len(),
        1,
        "a accepted b's request"
    );
}

// US1-AS2: a peer shared across two topics yields two independent per-(peer,
// topic) connections.
#[tokio::test]
async fn two_topics_yield_two_independent_connections() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let t1 = topic("t1");
    let t2 = topic("t2");

    let a = node_with(&registry, &network, "a", &[], &[t1.clone(), t2.clone()]).await;
    let b = node_with(&registry, &network, "b", &[], &[t1.clone(), t2.clone()]).await;
    await_candidates(&a, &t1, &["b"], TIMEOUT).await.unwrap();
    await_candidates(&a, &t2, &["b"], TIMEOUT).await.unwrap();

    trigger_setup(&a);
    trigger_setup(&b);
    await_upstream_active(&a, &peer("b"), &t1, TIMEOUT)
        .await
        .unwrap();
    await_upstream_active(&a, &peer("b"), &t2, TIMEOUT)
        .await
        .unwrap();

    let up = a.upstream_connections();
    assert_eq!(up.len(), 2, "one connection per (peer, topic)");
}

// US2 / SC-002: a valid signed message from a connected source is recorded,
// while the same valid message from an unconnected peer is dropped
// (not_connected) and never recorded — connection-gated delivery, observable
// through the getter.
#[tokio::test]
async fn unconnected_sender_is_not_recorded() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let t = topic("t");

    // s (receiver) and b (connected sender) share t. ghost is a member of no
    // topic, so it is not a t candidate and s never dials it — yet it can still
    // send (sending is decoupled from subscription, FR-023).
    let s = node_with(&registry, &network, "s", &[], std::slice::from_ref(&t)).await;
    let b = node_with(&registry, &network, "b", &[], std::slice::from_ref(&t)).await;
    let ghost = node_with(&registry, &network, "ghost", &[], &[]).await;

    // s dials its only t candidate, b — ghost stays unconnected to s.
    establish_upstreams(&s, &[&b], &t).await;

    let from_b = ping(t.clone(), 1);
    let from_ghost = ping(t.clone(), 2);
    b.send(s.id(), from_b.clone()).await.expect("b → s");
    ghost.send(s.id(), from_ghost).await.expect("ghost → s");

    // The connected source's message lands.
    await_delivery(&s, b.id(), &from_b, TIMEOUT)
        .await
        .expect("connected source recorded");
    // Settle, then confirm the unconnected source's message never landed.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let record = s.received_messages();
    assert_eq!(record.len(), 1, "only the connected source is recorded");
    assert_eq!(record[0].origin, Origin::Peer(b.id().clone()));
    assert!(
        !record
            .iter()
            .any(|d| d.origin == Origin::Peer(ghost.id().clone())),
        "the unconnected sender's valid message is dropped (not_connected)",
    );
}

// US3 / SC-003: one tampered message over an Active connection severs it
// silently; the offender's subsequent valid messages on that topic are then
// excluded, while its connection on another topic is untouched.
#[tokio::test]
async fn misbehavior_severs_one_connection_silently() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let t1 = topic("t1");
    let t2 = topic("t2");

    // s (receiver) dials the offender b on both topics they share.
    let s = node_with(&registry, &network, "s", &[], &[t1.clone(), t2.clone()]).await;
    let b = node_with(&registry, &network, "b", &[], &[t1.clone(), t2.clone()]).await;
    establish_upstreams(&s, &[&b], &t1).await;
    await_upstream_active(&s, b.id(), &t2, TIMEOUT)
        .await
        .expect("the same setup dialed b on t2 too");

    // b misbehaves on t1, then sends a valid t2 message. The b→s channel is
    // FIFO, so awaiting the t2 delivery guarantees the tampered t1 was processed
    // (and the connection severed) first.
    b.send(s.id(), tampered_ping(t1.clone(), 1))
        .await
        .expect("tampered t1");
    let good_t2 = ping(t2.clone(), 2);
    b.send(s.id(), good_t2.clone()).await.expect("valid t2");
    await_delivery(&s, b.id(), &good_t2, TIMEOUT)
        .await
        .expect("the t2 connection still delivers");

    let up = s.upstream_connections();
    assert!(
        !up.iter().any(|(p, t, _)| p == b.id() && t == &t1),
        "the t1 connection was severed",
    );
    assert!(
        up.iter()
            .any(|(p, t, st)| p == b.id() && t == &t2 && *st == UpstreamState::Active),
        "the offender's t2 connection is untouched",
    );

    // A subsequent valid t1 message from b is now excluded (not_connected).
    let good_t1 = ping(t1.clone(), 3);
    b.send(s.id(), good_t1.clone()).await.expect("valid t1");
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        !s.received_messages().iter().any(|d| d.message == good_t1),
        "the severed connection drops the offender's later valid t1 message",
    );
}

// US4-AS1 / SC-004: graceful shutdown notifies counterparts — after a node
// shuts down, the survivor holds zero dangling entries about it (both roles).
#[tokio::test]
async fn graceful_shutdown_clears_counterpart_entries() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let t = topic("t");

    let a = node_with(&registry, &network, "a", &[], std::slice::from_ref(&t)).await;
    let b = node_with(&registry, &network, "b", &[], std::slice::from_ref(&t)).await;
    establish_mutual(&a, &b, std::slice::from_ref(&t)).await;
    assert!(
        !a.upstream_connections().is_empty() && !a.downstream_connections().is_empty(),
        "a holds both-role entries about b before shutdown",
    );

    // b shuts down gracefully — one Terminated per held entry, then it releases.
    b.shutdown().await;

    await_peer_forgotten(&a, &peer("b"), TIMEOUT)
        .await
        .expect("a removes every entry about the departed b");
}

// US4-AS3 / FR-021: an abrupt drop sends no notices — the survivor keeps stale
// entries (harmless: they admit nothing on their own).
#[tokio::test]
async fn abrupt_drop_leaves_stale_entries() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let t = topic("t");

    let a = node_with(&registry, &network, "a", &[], std::slice::from_ref(&t)).await;
    let b = node_with(&registry, &network, "b", &[], std::slice::from_ref(&t)).await;
    establish_mutual(&a, &b, std::slice::from_ref(&t)).await;

    // Abrupt teardown — no shutdown call, no notices.
    drop(b);
    tokio::time::sleep(Duration::from_millis(50)).await;

    // a still holds its (now stale) entries about b.
    assert!(
        a.upstream_connections()
            .iter()
            .any(|(p, _, _)| p == &peer("b")),
        "abrupt drop leaves the survivor's upstream entry stale",
    );
    assert!(
        a.downstream_connections()
            .iter()
            .any(|(p, _)| p == &peer("b")),
        "abrupt drop leaves the survivor's downstream entry stale",
    );
}

// Note (US4-AS4 / SC-004, restart recovery): a literal same-alias *restart* is
// not expressible at integration level on the in-memory network — it has no
// deregistration, so re-`register`ing a dropped node's id returns
// DuplicateRegistration. The healing mechanic restart relies on — a counterpart
// idempotently re-accepting a re-dialing peer's duplicate Request, returning it
// to Active — is covered at the state level by
// `duplicate_request_idempotent_then_stale_on_failed_revalidation` in
// `src/state.rs`.

// US5-AS1/AS2: connection snapshots are stable, consistent clones, and a request
// to a peer that never answers is a visible AwaitingAccept diagnostic.
#[tokio::test]
async fn pending_connection_is_a_visible_stable_diagnostic() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let t = topic("t");

    // ghost is a registry member (so a candidate) but is never constructed, so
    // its mailbox does not exist and s's Request to it is silently dropped.
    registry
        .set_topics(peer("ghost"), [t.clone()].into_iter().collect())
        .await
        .unwrap();
    let s = node_with(&registry, &network, "s", &[], std::slice::from_ref(&t)).await;
    await_candidates(&s, &t, &["ghost"], TIMEOUT)
        .await
        .expect("s knows ghost as a candidate");

    trigger_setup(&s); // dials ghost; the request goes unanswered
    await_upstream_present(&s, &peer("ghost"), &t, TIMEOUT)
        .await
        .expect("the pending upstream is created");

    // The pending entry is a visible diagnostic at AwaitingAccept.
    let snapshot = s.upstream_connections();
    assert_eq!(
        snapshot,
        vec![(peer("ghost"), t.clone(), UpstreamState::AwaitingAccept)],
        "the unanswered request is observable as a pending entry",
    );

    // The snapshot is an owned clone — subsequent events don't mutate it, and a
    // re-read is consistent (the entry stays pending; nothing auto-heals it).
    trigger_setup(&s); // re-dials the pending pair
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        snapshot,
        vec![(peer("ghost"), t.clone(), UpstreamState::AwaitingAccept)],
        "the earlier snapshot is a stable clone, unaffected by later events",
    );
    assert_eq!(
        s.upstream_connections(),
        vec![(peer("ghost"), t, UpstreamState::AwaitingAccept)],
        "a fresh read still shows the pending entry — it never auto-heals",
    );
}

// The autonomous path (ADR 0020): nodes dial on their own once synced — the
// registry indexer folds both registry snapshots, then pushes `Event::Synced`,
// which transitions the node to `Synced` and establishes connections. No setup
// timer, no manual trigger; the readiness signal is the trigger.
#[tokio::test]
async fn readiness_establishes_autonomously() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let topic_registry = Arc::new(InMemoryTopicRegistry::new());
    let t = topic("t");
    topic_registry
        .set_topic(t.clone(), BTreeSet::new())
        .await
        .unwrap();
    registry
        .set_topics(peer("a"), [t.clone()].into_iter().collect())
        .await
        .unwrap();
    registry
        .set_topics(peer("b"), [t.clone()].into_iter().collect())
        .await
        .unwrap();

    let a = Node::new(
        peer("a"),
        NodeConfig::default(),
        network.clone(),
        alias_signer("a"),
        shared_test_verifier(),
        registry.clone(),
        topic_registry.clone(),
        Arc::new(ConnectToAllCandidates),
        Arc::new(ForwardToAll),
        Arc::new(AcceptFromAllCandidates),
    )
    .await
    .expect("construct a");
    let b = Node::new(
        peer("b"),
        NodeConfig::default(),
        network.clone(),
        alias_signer("b"),
        shared_test_verifier(),
        registry.clone(),
        topic_registry.clone(),
        Arc::new(ConnectToAllCandidates),
        Arc::new(ForwardToAll),
        Arc::new(AcceptFromAllCandidates),
    )
    .await
    .expect("construct b");

    // No trigger_setup, no timer — each node dials once synced (the indexer
    // folds both registry snapshots, then pushes `Event::Synced`).
    await_upstream_active(&a, &peer("b"), &t, TIMEOUT)
        .await
        .expect("a dials b on sync");
    await_upstream_active(&b, &peer("a"), &t, TIMEOUT)
        .await
        .expect("b dials a on sync");
    // The `Syncing → Synced` lifecycle is observable: having established
    // connections, both nodes report synced through the public getter.
    assert!(
        a.is_synced(),
        "a is synced once it has established connections"
    );
    assert!(
        b.is_synced(),
        "b is synced once it has established connections"
    );
}

// FR-024 / N-006: a duplicate registration on the same network surfaces the
// existing typed error from construction.
#[tokio::test]
async fn construction_fails_on_duplicate_registration() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let topic_registry = Arc::new(InMemoryTopicRegistry::new());

    let _first = Node::new(
        peer("a"),
        NodeConfig::default(),
        network.clone(),
        alias_signer("a"),
        shared_test_verifier(),
        registry.clone(),
        topic_registry.clone(),
        Arc::new(ConnectToAllCandidates),
        Arc::new(ForwardToAll),
        Arc::new(AcceptFromAllCandidates),
    )
    .await
    .expect("first registration succeeds");

    // `Node` is not `Debug`, so match on the result rather than `expect_err`.
    let result = Node::new(
        peer("a"),
        NodeConfig::default(),
        network.clone(),
        alias_signer("a"),
        shared_test_verifier(),
        registry.clone(),
        topic_registry.clone(),
        Arc::new(ConnectToAllCandidates),
        Arc::new(ForwardToAll),
        Arc::new(AcceptFromAllCandidates),
    )
    .await;

    assert!(
        matches!(
            result,
            Err(NodeError::Network(NetworkError::DuplicateRegistration(_))),
        ),
        "duplicate id surfaces the network error",
    );
}

// FR-024 / N-006: a self_id that does not match the signer is rejected at
// construction (before any registration) with the typed IdentityMismatch.
#[tokio::test]
async fn construction_fails_on_identity_mismatch() {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let topic_registry = Arc::new(InMemoryTopicRegistry::new());

    // self_id "a" but the signer is b's — incoherent. (`Node` is not `Debug`,
    // so match on the result rather than `expect_err`.)
    let result = Node::new(
        peer("a"),
        NodeConfig::default(),
        network.clone(),
        alias_signer("b"),
        shared_test_verifier(),
        registry.clone(),
        topic_registry.clone(),
        Arc::new(ConnectToAllCandidates),
        Arc::new(ForwardToAll),
        Arc::new(AcceptFromAllCandidates),
    )
    .await;

    assert!(
        matches!(result, Err(NodeError::IdentityMismatch(_))),
        "mismatch surfaces IdentityMismatch",
    );

    // And nothing was registered, so a coherent node can still take the id.
    let _ok = Node::new(
        peer("a"),
        NodeConfig::default(),
        network.clone(),
        alias_signer("a"),
        shared_test_verifier(),
        registry.clone(),
        topic_registry.clone(),
        Arc::new(ConnectToAllCandidates),
        Arc::new(ForwardToAll),
        Arc::new(AcceptFromAllCandidates),
    )
    .await
    .expect("the failed construction left the id free");
}

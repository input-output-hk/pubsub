//! 015 US1 integration: the M3 publisher-link behaviours end to end — standing
//! links established unconditionally (with NO relay topology at all), delivery
//! of a node's own publications over them, and the owner-binding that keeps
//! foreign messages off them.

mod common;

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::{
    assert_no_new_deliveries, await_delivery, await_publisher_target_active,
    build_signed_message_simple, node_with_links, ping,
};
use pubsub_node::{
    AcceptFromAllCandidates, AcceptNone, ConnectToAllCandidates, DialNone, ForwardToRelays,
    InMemoryNetwork, InMemorySubscriptionRegistry, Message, MessagePayload, MockCryptoScheme, Node,
    NodeStrategies, PeerId, PublisherAdmission, TopicId,
};

/// A `Ping(n)` on `t` signed with `alias`'s own key — the message an
/// owner-bound publisher link admits from that alias.
fn alias_ping(alias: &str, t: TopicId, n: u64) -> Message {
    let scheme = MockCryptoScheme::with_seed([0u8; 32]);
    let signer = scheme.signer(scheme.keypair_from_alias(alias).private);
    build_signed_message_simple(&signer, t, MessagePayload::Ping(n))
}

fn topic(s: &str) -> TopicId {
    TopicId::from_str(s).expect("valid topic id")
}

fn peer(s: &str) -> PeerId {
    PeerId::from_str(s).expect("valid peer id")
}

const T: Duration = Duration::from_secs(2);

/// A publisher-only strategy set: NO relay dials at all (an explicit empty
/// expected set), publisher links to every candidate. Any delivery in this
/// fleet can only have crossed a publisher link.
fn publisher_only() -> NodeStrategies {
    NodeStrategies {
        relay_connection: Arc::new(DialNone),
        relay_acceptance: Arc::new(AcceptNone),
        publisher_connection: Some(Arc::new(ConnectToAllCandidates)),
        publisher_acceptance: Some(Arc::new(AcceptFromAllCandidates)),
    }
}

async fn publisher_only_fleet() -> (Arc<InMemoryNetwork>, Node, Node, Node) {
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let topics = [topic("t1")];
    // Seed all three memberships BEFORE any node syncs is not possible with the
    // per-node construction order (each node syncs at construction); the
    // readiness dial therefore fires with a partial candidate view. A follow-up
    // heartbeat after all three are up re-dials the full expected set (the
    // heartbeat is the retry primitive), so trigger one per node below.
    let a = node_with_links(
        &registry,
        &network,
        "a",
        &topics,
        publisher_only(),
        Arc::new(ForwardToRelays),
        PublisherAdmission::OwnerOnly,
        0,
    )
    .await;
    let b = node_with_links(
        &registry,
        &network,
        "b",
        &topics,
        publisher_only(),
        Arc::new(ForwardToRelays),
        PublisherAdmission::OwnerOnly,
        0,
    )
    .await;
    let c = node_with_links(
        &registry,
        &network,
        "c",
        &topics,
        publisher_only(),
        Arc::new(ForwardToRelays),
        PublisherAdmission::OwnerOnly,
        0,
    )
    .await;
    // Candidate barrier before the retry heartbeat: without it the Heartbeat
    // can beat the last membership delta into a node's queue, and with no
    // further retry the fleet would strand a pending dial.
    for (node, others) in [(&a, ["b", "c"]), (&b, ["a", "c"]), (&c, ["a", "b"])] {
        common::await_candidates(node, &topic("t1"), &others, T)
            .await
            .expect("candidates converge");
    }
    for node in [&a, &b, &c] {
        common::trigger_setup(node);
    }
    // Establishment barrier: every node holds Active publisher links to both
    // peers — despite holding NO relay links whatsoever (the unconditional
    // establishment pin, FR-002).
    for (node, peers) in [(&a, ["b", "c"]), (&b, ["a", "c"]), (&c, ["a", "b"])] {
        for p in peers {
            await_publisher_target_active(node, &peer(p), &topic("t1"), T)
                .await
                .unwrap_or_else(|e| panic!("{}→{p} publisher link: {e}", node.id()));
        }
        assert!(
            node.upstream_relays().is_empty() && node.downstream_relays().is_empty(),
            "{}: the fleet must hold no relay links at all",
            node.id(),
        );
    }
    (network, a, b, c)
}

// FR-001/002/005/006 + SC-001 slice: standing publisher links form without any
// relay topology, and a node's own publication reaches its targets over them.
#[tokio::test]
async fn own_publication_rides_publisher_links() {
    let (_network, a, b, c) = publisher_only_fleet().await;

    // a publishes a message signed with its OWN key (the owner of its links).
    let message = alias_ping("a", topic("t1"), 1);
    let Message::Dissemination(signed) = message.clone() else {
        unreachable!("builder yields a dissemination message")
    };
    a.publish(signed);

    // Both peers receive it — necessarily over a's publisher links.
    await_delivery(&b, a.id(), &message, T)
        .await
        .expect("b receives");
    await_delivery(&c, a.id(), &message, T)
        .await
        .expect("c receives");

    // And it stops there: b/c hold only publisher links, which never carry
    // relayed traffic under the default fan-out — no echoes, no extra copies.
    assert_no_new_deliveries(&[&a, &b, &c], Duration::from_millis(50)).await;
}

// FR-006 owner-binding: a message published by a foreign key does NOT pass a
// publisher link, while the same node's own-key publication does.
#[tokio::test]
async fn foreign_publisher_is_dropped_at_publisher_links() {
    let (_network, a, b, c) = publisher_only_fleet().await;

    // b publishes a message signed by the shared test fixture key — a valid,
    // authorized publisher (the topic is open), but NOT b, the link owner.
    let foreign = ping(topic("t1"), 2);
    let Message::Dissemination(signed) = foreign.clone() else {
        unreachable!()
    };
    b.publish(signed);

    // b records its own publish locally; a and c drop it at the owner gate.
    assert_no_new_deliveries(&[&a, &c], Duration::from_millis(80)).await;

    // Control: b's own-key publication on the same links IS delivered.
    let own = alias_ping("b", topic("t1"), 3);
    let Message::Dissemination(signed) = own.clone() else {
        unreachable!()
    };
    b.publish(signed);
    await_delivery(&a, b.id(), &own, T)
        .await
        .expect("a receives b's own");
    await_delivery(&c, b.id(), &own, T)
        .await
        .expect("c receives b's own");
}

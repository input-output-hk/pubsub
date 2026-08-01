//! 015 US1 integration: the M3 publisher-link behaviours end to end — standing
//! links established unconditionally (with NO relay topology at all), delivery
//! of a node's own publications over them, and the kind-agnostic receive gate
//! (M3's exclusivity is the sender's fan-out policy, not a receiver check).

mod common;

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::{
    assert_no_new_deliveries, await_delivery, await_publisher_target_active,
    build_signed_message_simple, node_with_links, ping,
};
use pubsub_node::{
    ForwardToRelays, InMemoryNetwork, InMemorySubscriptionRegistry, LinkKind, Message,
    MessagePayload, MockCryptoScheme, Node, NodeStrategies, PeerId, Selection, TopicId,
    UnifiedAcceptance,
};

/// A `Ping(n)` on `t` signed with `alias`'s own key — the publication the
/// default fan-out seeds over that alias's publisher links.
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

/// A publisher-only strategy set: NO relay links at all (pick count 0 dials
/// none; a serve-none cap refuses inbound), publisher links to every
/// candidate (the publisher instance at the plane origin). Any delivery in
/// this fleet can only have crossed a publisher link.
fn publisher_only(id: &str) -> NodeStrategies {
    NodeStrategies {
        relay_connection: Arc::new(Selection::new(peer(id), [0u8; 32]).with_pick_count(Some(0))),
        relay_acceptance: Arc::new(UnifiedAcceptance::new(peer(id)).with_accept_cap(Some(0))),
        publisher_connection: Some(Arc::new(
            Selection::new(peer(id), [0u8; 32]).for_kind(LinkKind::Publisher),
        )),
        publisher_acceptance: Some(Arc::new(
            UnifiedAcceptance::new(peer(id)).for_kind(LinkKind::Publisher),
        )),
        symmetric_edges: false,
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
        publisher_only("a"),
        Arc::new(ForwardToRelays),
        0,
    )
    .await;
    let b = node_with_links(
        &registry,
        &network,
        "b",
        &topics,
        publisher_only("b"),
        Arc::new(ForwardToRelays),
        0,
    )
    .await;
    let c = node_with_links(
        &registry,
        &network,
        "c",
        &topics,
        publisher_only("c"),
        Arc::new(ForwardToRelays),
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

// FR-006 (as amended): the receive gate is kind-agnostic — a message published
// by a foreign key passes a publisher link like any authentic message. A
// receiver validates publisher-link arrivals exactly like relay arrivals
// (signature, registration, authorization, subscription, dedup); what keeps
// foreign traffic OFF publisher links in M3 is the sender's default fan-out,
// pinned in model_family's m3_defaults_do_not_relay_over_the_chain.
#[tokio::test]
async fn foreign_publisher_is_admitted_over_publisher_links() {
    let (_network, a, b, c) = publisher_only_fleet().await;

    // b publishes a message signed by the shared test fixture key — a valid,
    // authorized publisher (the topic is open), but NOT b, the link owner.
    let foreign = ping(topic("t1"), 2);
    let Message::Dissemination(signed) = foreign.clone() else {
        unreachable!()
    };
    b.publish(signed);

    // a and c admit it over b's publisher links — no owner-binding.
    await_delivery(&a, b.id(), &foreign, T)
        .await
        .expect("a admits the foreign-key publication");
    await_delivery(&c, b.id(), &foreign, T)
        .await
        .expect("c admits the foreign-key publication");

    // b's own-key publication on the same links is delivered identically.
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

// ---- 017 US3: publisher-seam over-capacity — the explicit Rejected,
// end to end -----------------------------------------------------------------

/// A node accepting at most `cap` inbound publisher links per topic; dials
/// no links of its own.
fn capped_publisher_acceptor(id: &str, cap: usize) -> NodeStrategies {
    NodeStrategies {
        relay_connection: Arc::new(Selection::new(peer(id), [0u8; 32]).with_pick_count(Some(0))),
        relay_acceptance: Arc::new(UnifiedAcceptance::new(peer(id))),
        publisher_connection: Some(Arc::new(
            Selection::new(peer(id), [0u8; 32])
                .for_kind(LinkKind::Publisher)
                .with_pick_count(Some(0)),
        )),
        publisher_acceptance: Some(Arc::new(
            UnifiedAcceptance::new(peer(id))
                .for_kind(LinkKind::Publisher)
                .with_accept_cap(Some(cap)),
        )),
        symmetric_edges: false,
    }
}

/// A node whose publisher seam dials exactly `targets` and accepts openly.
fn explicit_publisher_dialer(id: &str, targets: &[(&str, &TopicId)]) -> NodeStrategies {
    NodeStrategies {
        relay_connection: Arc::new(Selection::new(peer(id), [0u8; 32]).with_pick_count(Some(0))),
        relay_acceptance: Arc::new(UnifiedAcceptance::new(peer(id))),
        publisher_connection: Some(Arc::new(common::ConnectToExplicit(
            targets
                .iter()
                .map(|(p, t)| (peer(p), (*t).clone()))
                .collect(),
        ))),
        publisher_acceptance: Some(Arc::new(
            UnifiedAcceptance::new(peer(id)).for_kind(LinkKind::Publisher),
        )),
        symmetric_edges: false,
    }
}

/// Poll until the dialer holds NO downstream-publisher entry for
/// `(target, topic)` — sound as a removal detector only once the dial is
/// known to have fired (a sibling target from the same heartbeat fold went
/// Active).
async fn await_publisher_entry_gone(node: &Node, target: &PeerId, t: &TopicId, timeout: Duration) {
    let start = tokio::time::Instant::now();
    loop {
        let held = node
            .downstream_publishers()
            .into_iter()
            .any(|(p, topic, _)| &p == target && &topic == t);
        if !held {
            return;
        }
        assert!(
            start.elapsed() < timeout,
            "the pending publisher entry to {target} was never cleaned up within {timeout:?}",
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

// 017 FR-011/FR-013 + §1.2 item 2's missing coverage: a publisher-seam
// over-capacity refusal emits the explicit publisher-kind Rejected end to
// end — the refused dialer's pending entry is removed and the acceptor keeps
// serving exactly its cap.
#[tokio::test]
async fn publisher_over_capacity_is_rejected_end_to_end() {
    let t = topic("t1");
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());

    // "srv" caps inbound publisher links at 1; "spare" is the open sibling
    // target proving the refused dialer's heartbeat fold ran.
    let srv = node_with_links(
        &registry,
        &network,
        "srv",
        std::slice::from_ref(&t),
        capped_publisher_acceptor("srv", 1),
        Arc::new(ForwardToRelays),
        0,
    )
    .await;
    let spare = node_with_links(
        &registry,
        &network,
        "spare",
        std::slice::from_ref(&t),
        capped_publisher_acceptor("spare", 1),
        Arc::new(ForwardToRelays),
        0,
    )
    .await;

    // First dialer fills srv's cap.
    let first = node_with_links(
        &registry,
        &network,
        "first",
        std::slice::from_ref(&t),
        explicit_publisher_dialer("first", &[("srv", &t)]),
        Arc::new(ForwardToRelays),
        0,
    )
    .await;
    common::await_candidates(&first, &t, &["srv", "spare"], T)
        .await
        .expect("candidates converge");
    common::trigger_setup(&first);
    await_publisher_target_active(&first, srv.id(), &t, T)
        .await
        .expect("the first initiation dial fills the cap");

    // The second dialer targets srv (now at cap) and spare (open).
    let second = node_with_links(
        &registry,
        &network,
        "second",
        std::slice::from_ref(&t),
        explicit_publisher_dialer("second", &[("srv", &t), ("spare", &t)]),
        Arc::new(ForwardToRelays),
        0,
    )
    .await;
    common::await_candidates(&second, &t, &["srv", "spare", "first"], T)
        .await
        .expect("candidates converge");
    common::await_candidates(&srv, &t, &["spare", "first", "second"], T)
        .await
        .expect("acceptor membership view converges");
    common::trigger_setup(&second);

    // Both dials fired in one heartbeat fold: once spare is Active, the srv
    // entry existed — its disappearance is the routed publisher-kind
    // Rejected's cleanup (over capacity is the explicit refusal; membership
    // and predicate failures stay silent).
    await_publisher_target_active(&second, spare.id(), &t, T)
        .await
        .expect("the open sibling target admits the dial");
    await_publisher_entry_gone(&second, srv.id(), &t, T).await;

    // The acceptor still serves exactly its cap — the first link, untouched.
    assert_eq!(
        srv.upstream_publishers(),
        vec![(first.id().clone(), t.clone())],
        "the capped acceptor keeps exactly the one accepted initiation link",
    );
}

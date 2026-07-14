//! Feature 015 / ADR 0035 integration: the M4 and M5 dissemination models as
//! end-to-end configurations of the same node (`formal_spec/hybrid_dissemination/models/`).

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::{alias_signer, shared_test_verifier};
use pubsub_node::{
    bucket_count, is_valid_edge_for, is_valid_edge_sym, AcceptFromAllCandidates, ForwardToAll,
    HashGatedAcceptance, HashGatedSelection, InMemoryNetwork, InMemorySubscriptionRegistry,
    InMemoryTopicRegistry, LinkDirection, LinkRole, LinkState, Message, MessagePayload, Node,
    NodeConfig, Origin, PeerId, PlainMessage, PublishInAdmission, PublisherId, RoleAgnosticFanout,
    SignedMessage, SubscriptionRegistryControl, TopicId, TopicRegistryControl,
};

const TIMEOUT: Duration = Duration::from_secs(3);

fn topic() -> TopicId {
    TopicId::from_str("t").expect("valid topic id")
}

fn peer(s: &str) -> PeerId {
    PeerId::from_str(s).expect("valid peer id")
}

/// A message published (signed) under `publisher`'s own key.
fn own_publish(publisher: &str, n: u64) -> SignedMessage {
    let signer = alias_signer(publisher);
    let plain = PlainMessage {
        topic: topic(),
        publisher_id: PublisherId::new(signer.public_key()),
        parent_hash: None,
        sequence: 0,
        timestamp: pubsub_node::Timestamp::from_millis(0),
        payload: MessagePayload::Ping(n),
    };
    let signature = signer.sign(&plain.signed_bytes());
    SignedMessage { plain, signature }
}

async fn await_delivery_from(node: &Node, message: &SignedMessage, timeout: Duration) {
    let start = tokio::time::Instant::now();
    loop {
        if node
            .received_messages()
            .iter()
            .any(|d| d.message == Message::Dissemination(message.clone()))
        {
            return;
        }
        assert!(
            start.elapsed() < timeout,
            "timed out waiting for {} to record the message",
            node.id(),
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

/// Shared fixture: register the open topic and seed the full membership before
/// any node is built, so every readiness snapshot holds the complete view.
async fn seeded_registries(
    ids: &[&str],
) -> (
    Arc<InMemorySubscriptionRegistry>,
    Arc<InMemoryTopicRegistry>,
) {
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let topic_registry = Arc::new(InMemoryTopicRegistry::new());
    topic_registry
        .set_topic(topic(), BTreeSet::new())
        .await
        .expect("register topic open");
    for id in ids {
        registry
            .set_topics(peer(id), std::iter::once(topic()).collect())
            .await
            .expect("seed membership");
    }
    (registry, topic_registry)
}

// ===========================================================================
// M4 — bidirectional RF flooding (m4/README.md): symmetric hash-gated picks,
// each edge an Out+In pair on both sides; flooding on all incident links.
// ===========================================================================

/// The symmetric relay edge set for `id` under `genesis` — computed from the
/// exported predicate, so the test asserts against the model, not itself.
fn symmetric_neighbours(genesis: u64, id: &str, all: &[&str], degree: usize) -> BTreeSet<PeerId> {
    let t = topic();
    let candidates = all.len() - 1; // full view minus self
    let buckets = bucket_count(candidates, degree);
    all.iter()
        .filter(|other| **other != id)
        .filter(|other| {
            is_valid_edge_sym(
                LinkRole::Relay,
                genesis,
                &t,
                &peer(id),
                &peer(other),
                buckets,
            )
        })
        .map(|other| peer(other))
        .collect()
}

/// Find a genesis whose symmetric graph over `all` is connected (BFS over the
/// predicate-derived adjacency) with B > 1, so the flood must actually hop.
fn genesis_with_connected_symmetric_graph(all: &[&str], degree: usize) -> u64 {
    assert!(bucket_count(all.len() - 1, degree) > 1, "want B > 1");
    'genesis: for genesis in 0..10_000u64 {
        let adjacency: BTreeMap<&str, BTreeSet<PeerId>> = all
            .iter()
            .map(|id| (*id, symmetric_neighbours(genesis, id, all, degree)))
            .collect();
        // BFS from the first node.
        let mut seen = BTreeSet::from([all[0]]);
        let mut frontier = vec![all[0]];
        while let Some(next) = frontier.pop() {
            for other in all {
                if !seen.contains(other) && adjacency[next].contains(&peer(other)) {
                    seen.insert(other);
                    frontier.push(other);
                }
            }
        }
        if seen.len() == all.len() {
            return genesis;
        }
        if genesis == 9_999 {
            break 'genesis;
        }
    }
    panic!("no genesis in the sweep yields a connected symmetric graph");
}

async fn m4_node(
    registry: &Arc<InMemorySubscriptionRegistry>,
    topic_registry: &Arc<InMemoryTopicRegistry>,
    network: &Arc<InMemoryNetwork>,
    id: &str,
    genesis: u64,
    degree: usize,
) -> Node {
    let id = peer(id);
    Node::new(
        id.clone(),
        NodeConfig { peers: Vec::new() },
        genesis,
        network.clone(),
        alias_signer(&id.to_string()),
        shared_test_verifier(),
        registry.clone(),
        topic_registry.clone(),
        Arc::new(HashGatedSelection::new(LinkRole::Relay, id.clone(), degree).with_symmetric(true)),
        Arc::new(ForwardToAll),
        Arc::new(HashGatedAcceptance::new(id.clone(), degree).with_symmetric(true)),
        Arc::new(pubsub_node::NoLinks),
        Arc::new(AcceptFromAllCandidates),
        PublishInAdmission::default(),
    )
    .await
    .expect("construct M4 node")
}

// ADR 0035 / M4: under the symmetric predicate every edge materialises as the
// Out+In pair on BOTH sides (reciprocity / pair emergence), and a publish
// floods the whole connected component — full coverage over incident links.
#[tokio::test]
async fn m4_symmetric_edges_are_reciprocal_and_flood_to_full_coverage() {
    let ids = [
        "n01", "n02", "n03", "n04", "n05", "n06", "n07", "n08", "n09", "n10", "n11", "n12",
    ];
    let degree = 3; // 11 candidates / 3 -> B = 4 (a real partial topology)
    let genesis = genesis_with_connected_symmetric_graph(&ids, degree);

    let network = Arc::new(InMemoryNetwork::new());
    let (registry, topic_registry) = seeded_registries(&ids).await;
    let mut nodes = Vec::new();
    for id in ids {
        nodes.push(m4_node(&registry, &topic_registry, &network, id, genesis, degree).await);
    }

    // Every node converges to: Active Out = the predicate's symmetric set, and
    // In = the same set (the Out+In pair emergence).
    let start = tokio::time::Instant::now();
    for (i, id) in ids.iter().enumerate() {
        let want = symmetric_neighbours(genesis, id, &ids, degree);
        loop {
            let node = &nodes[i];
            let out: BTreeSet<PeerId> = node
                .links()
                .iter()
                .filter(|(_, _, role, dir, state)| {
                    *role == LinkRole::Relay
                        && *dir == LinkDirection::Out
                        && *state == LinkState::Active
                })
                .map(|(p, _, _, _, _)| p.clone())
                .collect();
            let inbound: BTreeSet<PeerId> = node
                .links()
                .iter()
                .filter(|(_, _, role, dir, _)| {
                    *role == LinkRole::Relay && *dir == LinkDirection::In
                })
                .map(|(p, _, _, _, _)| p.clone())
                .collect();
            if out == want && inbound == want {
                break;
            }
            assert!(
                start.elapsed() < TIMEOUT,
                "timed out waiting for {id}'s symmetric links: out={out:?} in={inbound:?} want={want:?}",
            );
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }

    // Publish at the first node: the flood over incident links reaches every
    // node (the graph was chosen connected).
    let message = own_publish("n01", 4);
    nodes[0].publish(message.clone());
    for node in &nodes[1..] {
        await_delivery_from(node, &message, TIMEOUT).await;
    }
}

// ===========================================================================
// M5 — directed k_in/k_out gossip (m5/README.md): outbound standing links
// carry EVERY held message; targets admit relayed traffic over them.
// ===========================================================================

/// The directed publish edge set for `id` (the model's `k_out` picks).
fn publish_targets(genesis: u64, id: &str, all: &[&str], degree: usize) -> BTreeSet<PeerId> {
    let t = topic();
    let buckets = bucket_count(all.len() - 1, degree);
    all.iter()
        .filter(|other| **other != id)
        .filter(|other| {
            is_valid_edge_for(
                LinkRole::Publisher,
                genesis,
                &t,
                &peer(id),
                &peer(other),
                buckets,
            )
        })
        .map(|other| peer(other))
        .collect()
}

async fn m5_node(
    registry: &Arc<InMemorySubscriptionRegistry>,
    topic_registry: &Arc<InMemoryTopicRegistry>,
    network: &Arc<InMemoryNetwork>,
    id: &str,
    genesis: u64,
    k_out: usize,
) -> Node {
    let id = peer(id);
    Node::new(
        id.clone(),
        NodeConfig { peers: Vec::new() },
        genesis,
        network.clone(),
        alias_signer(&id.to_string()),
        shared_test_verifier(),
        registry.clone(),
        topic_registry.clone(),
        // `k_in` = 0 for this test: the ONLY propagation edges are the standing
        // outbound links, so delivery proves the M5 relay-over-publish path.
        Arc::new(pubsub_node::NoLinks),
        Arc::new(RoleAgnosticFanout),
        Arc::new(AcceptFromAllCandidates),
        Arc::new(HashGatedSelection::new(
            LinkRole::Publisher,
            id.clone(),
            k_out,
        )),
        Arc::new(AcceptFromAllCandidates),
        PublishInAdmission::AnyVerified,
    )
    .await
    .expect("construct M5 node")
}

// ADR 0035 / M5: a message hops A → B → C purely over standing outbound links
// — B forwards A's message (a FOREIGN publisher) over B's own k_out link, and
// C's any-verified gate admits it. With owner-only gates this exact hop is
// the relay_over_publish_link drop, so delivery at C proves the M5 semantics.
#[tokio::test]
async fn m5_standing_links_relay_foreign_messages_end_to_end() {
    let ids = ["a", "b", "c"];
    let k_out = 1; // 2 candidates / 1 -> B_p = 2
                   // Find a genesis forming the chain a -> b -> c with NO direct a -> c edge:
                   // c's copy can only have travelled through b.
    let genesis = (0..10_000u64)
        .find(|g| {
            let a = publish_targets(*g, "a", &ids, k_out);
            let b = publish_targets(*g, "b", &ids, k_out);
            a.contains(&peer("b")) && !a.contains(&peer("c")) && b.contains(&peer("c"))
        })
        .expect("a chain genesis exists in the sweep");

    let network = Arc::new(InMemoryNetwork::new());
    let (registry, topic_registry) = seeded_registries(&ids).await;
    let mut nodes = Vec::new();
    for id in ids {
        nodes.push(m5_node(&registry, &topic_registry, &network, id, genesis, k_out).await);
    }

    // Await every standing link's dial→accept round-trip before publishing —
    // establishment has no retry, so an early publish would find a pending
    // (AwaitingAccept) link and select no targets.
    let start = tokio::time::Instant::now();
    for (i, id) in ids.iter().enumerate() {
        let want = publish_targets(genesis, id, &ids, k_out);
        loop {
            let active: BTreeSet<PeerId> = nodes[i]
                .links()
                .iter()
                .filter(|(_, _, role, dir, state)| {
                    *role == LinkRole::Publisher
                        && *dir == LinkDirection::Out
                        && *state == LinkState::Active
                })
                .map(|(p, _, _, _, _)| p.clone())
                .collect();
            if active == want {
                break;
            }
            assert!(
                start.elapsed() < TIMEOUT,
                "timed out waiting for {id}'s standing links: {active:?} want {want:?}",
            );
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }

    let message = own_publish("a", 5);
    nodes[0].publish(message.clone());

    // c records a's message — delivered by b over b's standing link.
    await_delivery_from(&nodes[2], &message, TIMEOUT).await;
    let delivery = nodes[2]
        .received_messages()
        .into_iter()
        .find(|d| d.message == Message::Dissemination(message.clone()))
        .expect("just awaited");
    assert_eq!(
        delivery.origin,
        Origin::Peer(peer("b")),
        "the copy travelled a -> b -> c over standing links",
    );
    // No relay links exist anywhere (k_in = 0): the standing links were the
    // only propagation path.
    for node in &nodes {
        assert!(
            node.links()
                .iter()
                .all(|(_, _, role, _, _)| *role == LinkRole::Publisher),
            "no relay links in this topology",
        );
    }
}

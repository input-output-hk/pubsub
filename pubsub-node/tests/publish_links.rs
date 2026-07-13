//! Feature 015 integration: a publisher's **standing initiation links**
//! (`formal_spec/hybrid_dissemination/models/m3/README.md`) — always
//! established, end-to-end through the real dial→accept handshake on the
//! readiness heartbeat — carry its published message into the overlay
//! (SC-003), independent of its relay-side links (ADR 0034).

mod common;

use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::{alias_signer, ping, shared_test_verifier};
use pubsub_node::{
    bucket_count, is_valid_edge_for, AcceptFromAllCandidates, HashGatedSelection, InMemoryNetwork,
    InMemorySubscriptionRegistry, InMemoryTopicRegistry, LinkDirection, LinkRole, LinkState,
    Message, Node, NodeConfig, Origin, PeerId, PublishInAdmission, SubscriptionRegistryControl,
    TopicId, TopicRegistryControl,
};

const TIMEOUT: Duration = Duration::from_secs(2);
const RELAY_DEGREE: usize = 2;
const PUBLISH_DEGREE: usize = 2;

fn topic() -> TopicId {
    TopicId::from_str("t").expect("valid topic id")
}

fn peer(s: &str) -> PeerId {
    PeerId::from_str(s).expect("valid peer id")
}

/// Find a genesis nonce under which the publish predicate selects at least
/// one initiation target for the publisher. Computed from the same exported
/// predicate the node uses, so the test asserts against the model, not
/// against itself. (No relay-side condition: standing initiation links are
/// established unconditionally — ADR 0034.)
fn genesis_with_initiation_targets(publisher: &str, candidates: &[&str]) -> (u64, Vec<PeerId>) {
    let t = topic();
    let p = peer(publisher);
    let publish_buckets = bucket_count(candidates.len(), PUBLISH_DEGREE);
    for genesis in 0..10_000u64 {
        let targets: Vec<PeerId> = candidates
            .iter()
            .map(|c| peer(c))
            .filter(|c| is_valid_edge_for(LinkRole::Publisher, genesis, &t, &p, c, publish_buckets))
            .collect();
        if !targets.is_empty() {
            return (genesis, targets);
        }
    }
    panic!("no genesis in the sweep selects an initiation target for {publisher}");
}

async fn build_node(
    registry: &Arc<InMemorySubscriptionRegistry>,
    network: &Arc<InMemoryNetwork>,
    topic_registry: &Arc<InMemoryTopicRegistry>,
    id: &str,
    genesis: u64,
    publisher: bool,
) -> Node {
    let id = peer(id);
    let publish: Arc<dyn pubsub_node::LinkSelectionStrategy> = if publisher {
        Arc::new(HashGatedSelection::new(
            LinkRole::Publisher,
            id.clone(),
            PUBLISH_DEGREE,
        ))
    } else {
        Arc::new(pubsub_node::NoLinks)
    };
    Node::new(
        id.clone(),
        NodeConfig { peers: Vec::new() },
        genesis,
        network.clone(),
        alias_signer(&id.to_string()),
        shared_test_verifier(),
        registry.clone(),
        topic_registry.clone(),
        Arc::new(HashGatedSelection::new(
            LinkRole::Relay,
            id.clone(),
            RELAY_DEGREE,
        )),
        Arc::new(pubsub_node::ForwardToAll),
        Arc::new(AcceptFromAllCandidates),
        publish,
        Arc::new(AcceptFromAllCandidates),
        PublishInAdmission::default(),
    )
    .await
    .expect("construct node")
}

// 015 SC-003 / US3: the publisher forms its hash-selected standing initiation
// links on the readiness heartbeat — unconditionally — and its published
// message reaches the overlay through them: each accepted target records the
// message with Origin::Peer(publisher).
#[tokio::test]
async fn publisher_injects_via_standing_initiation_links() {
    let candidates = ["r1", "r2", "r3", "r4", "r5", "r6"];
    let (genesis, expected_targets) = genesis_with_initiation_targets("pub", &candidates);

    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let topic_registry = Arc::new(InMemoryTopicRegistry::new());
    let t = topic();
    topic_registry
        .set_topic(t.clone(), BTreeSet::new())
        .await
        .expect("register topic open");

    // Seed the FULL membership (publisher included) before any node is built,
    // so every node's readiness snapshot holds the complete candidate set and
    // the acceptors already know the publisher when its dial arrives.
    for id in candidates.iter().copied().chain(std::iter::once("pub")) {
        registry
            .set_topics(peer(id), std::iter::once(t.clone()).collect())
            .await
            .expect("seed membership");
    }

    let relays = {
        let mut relays = Vec::new();
        for id in candidates {
            relays.push(build_node(&registry, &network, &topic_registry, id, genesis, false).await);
        }
        relays
    };
    let publisher = build_node(&registry, &network, &topic_registry, "pub", genesis, true).await;

    // The publisher's publishing links activate (dial → accept round-trip).
    let start = tokio::time::Instant::now();
    loop {
        let active: Vec<PeerId> = publisher
            .links()
            .into_iter()
            .filter(|(_, _, role, dir, state)| {
                *role == LinkRole::Publisher
                    && *dir == LinkDirection::Out
                    && *state == LinkState::Active
            })
            .map(|(p, _, _, _, _)| p)
            .collect();
        if active.len() == expected_targets.len() {
            let mut got = active.clone();
            got.sort_by_key(ToString::to_string);
            let mut want = expected_targets.clone();
            want.sort_by_key(ToString::to_string);
            assert_eq!(got, want, "publish links match the predicate's selection");
            break;
        }
        assert!(
            start.elapsed() < TIMEOUT,
            "timed out waiting for publishing links to activate",
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
    }

    // Publish; every publishing-link target records the message, attributed to
    // the publisher (received over the In/Publisher link).
    let Message::Dissemination(signed) = ping(t.clone(), 42) else {
        unreachable!("ping yields Message::Dissemination")
    };
    // Publish under the publisher's own key so the receive gate's publisher
    // binding holds (publishing links carry only the link peer's own messages).
    let plain = pubsub_node::PlainMessage {
        publisher_id: pubsub_node::PublisherId::new(alias_signer("pub").public_key()),
        ..signed.plain
    };
    let signature = alias_signer("pub").sign(&plain.signed_bytes());
    let message = pubsub_node::SignedMessage { plain, signature };
    publisher.publish(message.clone());

    let start = tokio::time::Instant::now();
    'targets: for target in &expected_targets {
        let node = relays
            .iter()
            .find(|n| n.id() == target)
            .expect("target is a built relay");
        loop {
            if node.received_messages().iter().any(|d| {
                d.origin == Origin::Peer(peer("pub"))
                    && d.message == Message::Dissemination(message.clone())
            }) {
                continue 'targets;
            }
            assert!(
                start.elapsed() < TIMEOUT,
                "timed out waiting for {target} to record the published message",
            );
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }
}

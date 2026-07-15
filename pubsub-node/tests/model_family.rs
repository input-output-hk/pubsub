//! 015 model-family integration: M4 — bidirectional relay links from the
//! symmetric edge predicate. Every link forms as a reciprocal pair, and one
//! publication floods the whole predicate-connected graph.

mod common;

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::{
    assert_no_new_deliveries, await_candidates, await_publisher_target_active,
    await_upstream_active, node_with_links, ping, trigger_setup, ConnectToExplicit,
};
use pubsub_node::{
    is_valid_edge_sym, AcceptFromAllCandidates, AllLinks, FanoutStrategy, ForwardToAll,
    HashGatedAcceptance, HashGatedConnection, InMemoryNetwork, InMemorySubscriptionRegistry,
    Message, Node, NodeStrategies, PeerId, PublisherAdmission, TopicId,
};

fn topic(s: &str) -> TopicId {
    TopicId::from_str(s).expect("valid topic id")
}

fn peer(s: &str) -> PeerId {
    PeerId::from_str(s).expect("valid peer id")
}

const T: Duration = Duration::from_secs(2);
const BUCKETS: usize = 2; // pinned on both seams AND in the offline sweep
const DEGREE: usize = 3;

/// The symmetric-edge pairs (i < j) among `names` at `genesis` — the exact
/// edge set the fleet must realise (the predicate is pure and public).
fn sym_edges(names: &[&str], genesis: u64, t: &TopicId) -> Vec<(usize, usize)> {
    let mut edges = Vec::new();
    for i in 0..names.len() {
        for j in (i + 1)..names.len() {
            if is_valid_edge_sym(genesis, t, &peer(names[i]), &peer(names[j]), BUCKETS) {
                edges.push((i, j));
            }
        }
    }
    edges
}

/// Whether the sym-edge graph over `names` is connected at `genesis`.
fn is_connected(names: &[&str], genesis: u64, t: &TopicId) -> bool {
    let edges = sym_edges(names, genesis, t);
    let mut reached = vec![false; names.len()];
    let mut stack = vec![0usize];
    reached[0] = true;
    while let Some(u) = stack.pop() {
        for &(a, b) in &edges {
            let v = if a == u {
                b
            } else if b == u {
                a
            } else {
                continue;
            };
            if !reached[v] {
                reached[v] = true;
                stack.push(v);
            }
        }
    }
    reached.into_iter().all(|r| r)
}

/// An M4 node: symmetric hash-gated relay selection AND acceptance (pinned
/// B so the offline sweep and both seams agree by construction); no
/// publisher links.
fn m4_strategies(id: &str) -> NodeStrategies {
    NodeStrategies::relay_only(
        Arc::new(
            HashGatedConnection::new(peer(id), DEGREE)
                .with_bucket_override(Some(BUCKETS))
                .with_symmetric(true),
        ),
        Arc::new(
            HashGatedAcceptance::new(peer(id), DEGREE)
                .with_bucket_override(Some(BUCKETS))
                .with_symmetric(true),
        ),
    )
}

// SC-002: 100% link reciprocity and 100% delivery over a predicate-connected
// symmetric graph.
#[tokio::test]
async fn m4_symmetric_edges_form_reciprocal_pairs_and_flood() {
    let names = ["n0", "n1", "n2", "n3", "n4", "n5"];
    let t = topic("t1");

    // Deterministically find a genesis whose symmetric graph is connected —
    // the predicate is public, so the experiment (and this test) can sweep it.
    let genesis = (0..512u64)
        .find(|g| is_connected(&names, *g, &t))
        .expect("some genesis under 512 yields a connected symmetric graph");
    let edges = sym_edges(&names, genesis, &t);

    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let mut nodes: Vec<Node> = Vec::new();
    for id in names {
        nodes.push(
            node_with_links(
                &registry,
                &network,
                id,
                &[t.clone()],
                m4_strategies(id),
                Arc::new(ForwardToAll),
                PublisherAdmission::default(),
                genesis,
            )
            .await,
        );
    }

    // Candidate convergence on every node (the readiness dial fired against a
    // partial view; the follow-up heartbeat below is the retry pass).
    for (i, node) in nodes.iter().enumerate() {
        let others: Vec<&str> = names
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, n)| *n)
            .collect();
        await_candidates(node, &t, &others, T)
            .await
            .expect("candidates converge");
    }
    for node in &nodes {
        trigger_setup(node);
    }

    // Establishment barrier: every predicate edge is Active from BOTH ends.
    for &(i, j) in &edges {
        await_upstream_active(&nodes[i], nodes[j].id(), &t, T)
            .await
            .unwrap_or_else(|e| panic!("{}→{} upstream: {e}", names[i], names[j]));
        await_upstream_active(&nodes[j], nodes[i].id(), &t, T)
            .await
            .unwrap_or_else(|e| panic!("{}→{} upstream: {e}", names[j], names[i]));
    }

    // Reciprocity: each node's upstream peer set equals its downstream peer
    // set — every link is a pair, none dangles one-way. And the realised edge
    // set is exactly the predicate's.
    for (i, node) in nodes.iter().enumerate() {
        assert_reciprocal_and_exact(node, i, &names, &edges);
    }

    // Full-coverage flood: one publication reaches every node over the mesh.
    // (The delivering peer varies per hop, so poll for the CONTENT, not the
    // origin.)
    let message = ping(t.clone(), 42);
    let Message::Dissemination(signed) = message.clone() else {
        unreachable!()
    };
    nodes[0].publish(signed);
    for node in &nodes[1..] {
        await_content(node, &message, T).await;
    }
}

/// Assert node `i`'s upstream peer set equals its downstream peer set (every
/// link is a reciprocal pair) and equals exactly the predicate's edge set.
fn assert_reciprocal_and_exact(node: &Node, i: usize, names: &[&str], edges: &[(usize, usize)]) {
    let mut up: Vec<String> = node
        .upstream_relays()
        .into_iter()
        .map(|(p, _, _)| p.to_string())
        .collect();
    let mut down: Vec<String> = node
        .downstream_relays()
        .into_iter()
        .map(|(p, _)| p.to_string())
        .collect();
    up.sort();
    up.dedup();
    down.sort();
    down.dedup();
    assert_eq!(
        up, down,
        "{}: upstream and downstream peers must match",
        names[i]
    );

    let mut expected: Vec<String> = edges
        .iter()
        .filter_map(|&(a, b)| {
            if a == i {
                Some(names[b])
            } else if b == i {
                Some(names[a])
            } else {
                None
            }
        })
        .map(|n| peer(n).to_string())
        .collect();
    expected.sort();
    assert_eq!(
        up, expected,
        "{}: realised edges must equal the predicate's",
        names[i]
    );
}

/// Poll `node.received_messages()` until it contains `message` (any origin) or
/// `timeout` elapses.
async fn await_content(node: &Node, message: &Message, timeout: Duration) {
    let start = tokio::time::Instant::now();
    loop {
        if node
            .received_messages()
            .iter()
            .any(|d| &d.message == message)
        {
            return;
        }
        assert!(
            start.elapsed() < timeout,
            "{} did not receive the flooded message within {timeout:?}",
            node.id(),
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

// ---- M5: directed publisher chain, everything-carrying ----------------------

/// An M5-chain node: no relay links at all; publisher links to an explicit
/// target list (the directed `k_out` picks); publisher acceptance open.
fn chain_strategies(targets: &[(&str, &TopicId)]) -> NodeStrategies {
    NodeStrategies {
        connection: Arc::new(ConnectToExplicit(Vec::new())),
        acceptance: Arc::new(AcceptFromAllCandidates),
        publisher_connection: Some(Arc::new(ConnectToExplicit(
            targets
                .iter()
                .map(|(p, t)| (peer(p), (*t).clone()))
                .collect(),
        ))),
        publisher_acceptance: Some(Arc::new(AcceptFromAllCandidates)),
    }
}

/// Build the a→b→c publisher-link chain under the given fan-out + admission
/// and return the three nodes with all links Active.
async fn chain_fleet(
    fanout: fn() -> Arc<dyn FanoutStrategy>,
    admission: PublisherAdmission,
) -> (Node, Node, Node) {
    let t = topic("t1");
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let a = node_with_links(
        &registry,
        &network,
        "a",
        &[t.clone()],
        chain_strategies(&[("b", &t)]),
        fanout(),
        admission,
        0,
    )
    .await;
    let b = node_with_links(
        &registry,
        &network,
        "b",
        &[t.clone()],
        chain_strategies(&[("c", &t)]),
        fanout(),
        admission,
        0,
    )
    .await;
    let c = node_with_links(
        &registry,
        &network,
        "c",
        &[t.clone()],
        chain_strategies(&[]),
        fanout(),
        admission,
        0,
    )
    .await;
    for node in [&a, &b, &c] {
        trigger_setup(node); // retry pass once every membership has folded
    }
    await_publisher_target_active(&a, b.id(), &t, T)
        .await
        .expect("a→b publisher link");
    await_publisher_target_active(&b, c.id(), &t, T)
        .await
        .expect("b→c publisher link");
    (a, b, c)
}

// SC-003: with all-links + any-verified, a foreign publisher's message hops
// a→b→c over standing publisher links only — b relays a's message to c.
#[tokio::test]
async fn m5_chain_relays_foreign_publisher_over_standing_links() {
    let (a, _b, c) = chain_fleet(|| Arc::new(AllLinks), PublisherAdmission::AnyVerified).await;
    let t = topic("t1");

    let message = alias_ping_m5("a", &t, 5);
    let Message::Dissemination(signed) = message.clone() else {
        unreachable!()
    };
    a.publish(signed);

    // c holds no link to a — the ONLY path is the b hop, admitted by
    // any-verified and forwarded by all-links.
    await_content(&c, &message, T).await;
}

// The M3 exclusivity pin: the SAME topology under the defaults does NOT
// deliver a's message to c — forward-to-all never relays over publisher links
// (and owner-only would drop the b→c hop anyway).
#[tokio::test]
async fn m3_defaults_do_not_relay_over_the_chain() {
    let (a, b, c) = chain_fleet(|| Arc::new(ForwardToAll), PublisherAdmission::OwnerOnly).await;
    let t = topic("t1");

    let message = alias_ping_m5("a", &t, 6);
    let Message::Dissemination(signed) = message.clone() else {
        unreachable!()
    };
    a.publish(signed);

    // b receives it (a owns the a→b link)…
    await_content(&b, &message, T).await;
    // …and it stops there.
    assert_no_new_deliveries(&[&c], Duration::from_millis(80)).await;
}

/// A `Ping(n)` signed with `alias`'s own key (the chain publisher).
fn alias_ping_m5(alias: &str, t: &TopicId, n: u64) -> Message {
    use pubsub_node::{MessagePayload, MockCryptoScheme, PublisherId, SignedMessage, Signer};
    let scheme = MockCryptoScheme::with_seed([0u8; 32]);
    let signer = scheme.signer(scheme.keypair_from_alias(alias).private);
    let plain = pubsub_node::PlainMessage {
        topic: t.clone(),
        publisher_id: PublisherId::new(signer.public_key()),
        parent_hash: None,
        sequence: 0,
        timestamp: pubsub_node::Timestamp::from_millis(0),
        payload: MessagePayload::Ping(n),
    };
    let signature = signer.sign(&plain.signed_bytes());
    Message::Dissemination(SignedMessage { plain, signature })
}

//! 015 model-family integration: M4 — bidirectional relay links from the
//! symmetric edge predicate. Every link forms as a reciprocal pair, and one
//! publication floods the whole predicate-connected graph.

mod common;

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::{await_candidates, await_upstream_active, node_with_links, ping, trigger_setup};
use pubsub_node::{
    is_valid_edge_sym, HashGatedAcceptance, HashGatedConnection, InMemoryNetwork,
    InMemorySubscriptionRegistry, Message, Node, NodeStrategies, PeerId, PublisherAdmission,
    TopicId,
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

//! Feature 005 (US1) integration: the verifiable hash-gated connection-selection
//! policy, exercised through a real node + event loop, forms a partial topology
//! whose edges match the public edge predicate (SC-002, verifiable) and is
//! reproducible from the genesis (the initial epoch nonce, SC-001).

mod common;

use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::node_with_strategy;
use pubsub_node::{
    bucket_count, is_valid_edge, HashGatedConnection, InMemoryNetwork,
    InMemorySubscriptionRegistry, PeerId, SubscriptionRegistryControl, TopicId,
};

const TIMEOUT: Duration = Duration::from_secs(2);

fn topic(s: &str) -> TopicId {
    TopicId::from_str(s).expect("valid topic id")
}

fn peer(s: &str) -> PeerId {
    PeerId::from_str(s).expect("valid peer id")
}

/// The upstream set the hash-gated policy must produce for `self` on `topic`:
/// exactly the candidates satisfying the edge predicate under the genesis nonce
/// (the epoch v1 never advances), `B = max(1, round(candidates / target_degree))`.
/// The strategy is deterministic, so this is the exact expected topology.
fn expected_upstreams(genesis: u64, target_degree: usize, candidates: &[&str]) -> BTreeSet<PeerId> {
    let t = topic("topic");
    let buckets = bucket_count(candidates.len(), target_degree);
    candidates
        .iter()
        .map(|c| peer(c))
        .filter(|c| is_valid_edge(genesis, &t, &peer("self"), c, buckets))
        .collect()
}

/// Build a single node running `HashGatedConnection { target_degree }` with the
/// given `genesis` (its initial epoch nonce) on one topic with `candidates`
/// other members pre-seeded in the shared subscription registry (so the node's
/// readiness heartbeat sees the full candidate set), await the node reaching its
/// expected upstream count, and return the peers it selected.
///
/// The candidate ids are registry members only — not real network nodes — so the
/// dials stay `AwaitingAccept`; the upstream *set* is exactly the selection.
async fn selected_upstreams(
    genesis: u64,
    target_degree: usize,
    candidates: &[&str],
) -> BTreeSet<PeerId> {
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let network = Arc::new(InMemoryNetwork::new());
    let t = topic("topic");

    for c in candidates {
        registry
            .set_topics(peer(c), std::iter::once(t.clone()).collect())
            .await
            .expect("seed candidate membership");
    }

    let strategy = Arc::new(HashGatedConnection::new(peer("self"), target_degree));
    let node = node_with_strategy(
        &registry,
        &network,
        "self",
        &[],
        std::slice::from_ref(&t),
        strategy,
        genesis,
    )
    .await;

    let want = expected_upstreams(genesis, target_degree, candidates).len();
    await_upstream_count(&node, want, TIMEOUT).await;
    node.upstream_connections()
        .into_iter()
        .map(|(p, _, _)| p)
        .collect()
}

/// Poll until the node holds exactly `n` upstream entries, or time out.
async fn await_upstream_count(node: &pubsub_node::Node, n: usize, timeout: Duration) {
    let start = tokio::time::Instant::now();
    loop {
        let len = node.upstream_connections().len();
        if len == n {
            return;
        }
        assert!(
            start.elapsed() < timeout,
            "upstream count never reached {n} (last saw {len})",
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

// US1 / SC-001 + SC-002: the node's realized upstream set is exactly the edge
// predicate's — verifiable and bounded — and reproduces identically across two
// independent runs with the same genesis + membership.
#[tokio::test]
async fn hash_gated_selection_matches_predicate_and_reproduces() {
    let candidates = ["c0", "c1", "c2", "c3", "c4", "c5"];
    let target_degree = 3;
    let expected = expected_upstreams(7, target_degree, &candidates);

    let first = selected_upstreams(7, target_degree, &candidates).await;
    let second = selected_upstreams(7, target_degree, &candidates).await;

    // SC-002: the realized set is exactly the verifiable edge set (never exceeds
    // the candidate set; bounded around target_degree via the bucket count).
    assert_eq!(
        first, expected,
        "the realized upstreams must be exactly the edge-predicate set",
    );
    assert!(first
        .iter()
        .all(|p| candidates.contains(&p.to_string().as_str())));
    // SC-001: same genesis + membership reproduces the identical selection.
    assert_eq!(
        first, second,
        "the same genesis must reproduce the identical upstream selection",
    );
}

// Small-topic path across the node boundary: candidates ≤ target_degree ⇒ B=1 ⇒ every
// candidate is a valid edge (connect-to-all fallback).
#[tokio::test]
async fn small_topic_selects_all_candidates() {
    let candidates = ["c0", "c1"];
    let selected = selected_upstreams(7, 5, &candidates).await;
    assert_eq!(
        selected,
        BTreeSet::from([peer("c0"), peer("c1")]),
        "with ≤ target_degree candidates B=1, so all are selected",
    );
}

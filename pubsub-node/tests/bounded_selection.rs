//! Gated-selection integration (the 005 verifiable hash-gated behaviour as a
//! 017 plane point): a real node + event loop running gate-only selection —
//! the bucket count fed, no pick count — forms a partial topology whose edges
//! are exactly the public edge predicate's, reproducibly from the genesis
//! (the initial epoch nonce).

mod common;

use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::node_with_strategy;
use pubsub_node::{
    is_valid_edge, InMemoryNetwork, InMemorySubscriptionRegistry, PeerId, Selection,
    SubscriptionRegistryControl, TopicId,
};

const TIMEOUT: Duration = Duration::from_secs(2);

fn topic(s: &str) -> TopicId {
    TopicId::from_str(s).expect("valid topic id")
}

fn peer(s: &str) -> PeerId {
    PeerId::from_str(s).expect("valid peer id")
}

/// The upstream set gate-only selection must produce for `self` on `topic`:
/// exactly the candidates satisfying the edge predicate under the genesis
/// nonce (the epoch v1 never advances) at the fed bucket count. The
/// selection is deterministic, so this is the exact expected topology.
fn expected_upstreams(genesis: u64, buckets: usize, candidates: &[&str]) -> BTreeSet<PeerId> {
    let t = topic("topic");
    candidates
        .iter()
        .map(|c| peer(c))
        .filter(|c| is_valid_edge(genesis, &t, &peer("self"), c, buckets))
        .collect()
}

/// Build a single node running gate-only `Selection` at `buckets` with the
/// given `genesis` (its initial epoch nonce) on one topic with `candidates`
/// other members pre-seeded in the shared subscription registry (so the
/// node's readiness heartbeat sees the full candidate set), await the node
/// reaching its expected upstream count, and return the peers it selected.
///
/// The candidate ids are registry members only — not real network nodes — so
/// the dials stay `AwaitingAccept`; the upstream *set* is exactly the
/// selection.
async fn selected_upstreams(
    genesis: u64,
    buckets: Option<usize>,
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

    let strategy = Arc::new(Selection::new(peer("self"), [0u8; 32]).with_bucket_count(buckets));
    let node = node_with_strategy(
        &registry,
        &network,
        "self",
        std::slice::from_ref(&t),
        strategy,
        genesis,
    )
    .await;

    let want = match buckets {
        Some(b) => expected_upstreams(genesis, b, candidates).len(),
        None => candidates.len(),
    };
    await_upstream_count(&node, want, TIMEOUT).await;
    node.upstream_relays()
        .into_iter()
        .map(|(p, _, _)| p)
        .collect()
}

/// Poll until the node holds exactly `n` upstream entries, or time out.
async fn await_upstream_count(node: &pubsub_node::Node, n: usize, timeout: Duration) {
    let start = tokio::time::Instant::now();
    loop {
        let len = node.upstream_relays().len();
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

// 005 US1 lineage / 017-FR-001: the node's realized upstream set is exactly
// the edge predicate's at the fed bucket count — verifiable — and reproduces
// identically across two independent runs with the same genesis + membership.
#[tokio::test]
async fn gated_selection_matches_predicate_and_reproduces() {
    let candidates = ["c0", "c1", "c2", "c3", "c4", "c5"];
    let buckets = 2;
    let expected = expected_upstreams(7, buckets, &candidates);

    let first = selected_upstreams(7, Some(buckets), &candidates).await;
    let second = selected_upstreams(7, Some(buckets), &candidates).await;

    assert_eq!(
        first, expected,
        "the realized upstreams must be exactly the edge-predicate set",
    );
    assert!(first
        .iter()
        .all(|p| candidates.contains(&p.to_string().as_str())));
    assert_eq!(
        first, second,
        "the same genesis must reproduce the identical upstream selection",
    );
}

// The ungated contrast across the node boundary: the same membership with
// the bucket count absent selects every candidate (there is no derived
// small-topic floor any more — ungated is a configuration, not a fallback).
#[tokio::test]
async fn ungated_selection_connects_to_all() {
    let candidates = ["c0", "c1", "c2", "c3", "c4", "c5"];
    let selected = selected_upstreams(7, None, &candidates).await;
    assert_eq!(
        selected,
        candidates.iter().map(|c| peer(c)).collect::<BTreeSet<_>>(),
        "an absent bucket count selects every candidate",
    );
}

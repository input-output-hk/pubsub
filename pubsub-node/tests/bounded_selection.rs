//! Feature 005 (US1) integration: the seeded bounded connection-selection
//! policy, exercised through a real node + event loop, forms a partial topology
//! that is capped at the out-degree (SC-002) and reproducible from the seed
//! (SC-001).

mod common;

use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::node_with_strategy;
use pubsub_node::{
    InMemoryNetwork, InMemorySubscriptionRegistry, PeerId, SeededBoundedConnection,
    SubscriptionRegistryControl, TopicId,
};

const TIMEOUT: Duration = Duration::from_secs(2);

fn topic(s: &str) -> TopicId {
    TopicId::from_str(s).expect("valid topic id")
}

fn peer(s: &str) -> PeerId {
    PeerId::from_str(s).expect("valid peer id")
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

/// Build a single node running `SeededBoundedConnection { seed, out_degree }` on
/// one topic with `candidates` other members **pre-seeded** in the shared
/// subscription registry (so the node's one readiness-driven selection sees the
/// full candidate set), and return the set of peers it selected as upstreams.
///
/// The candidate ids are registry members only — not real network nodes — so the
/// node's dials stay `AwaitingAccept`; the upstream *set* is exactly the
/// selection. Each call uses a fresh network + registry, so two calls with the
/// same seed are independent runs.
async fn selected_upstreams(seed: u64, out_degree: usize, candidates: &[&str]) -> HashSet<PeerId> {
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let network = Arc::new(InMemoryNetwork::new());
    let t = topic("topic");

    // Pre-seed the candidate memberships before constructing the node, so they
    // are present in the registry snapshot the node folds at startup.
    for c in candidates {
        registry
            .set_topics(peer(c), std::iter::once(t.clone()).collect())
            .await
            .expect("seed candidate membership");
    }

    let strategy = Arc::new(SeededBoundedConnection::new(seed, peer("self"), out_degree));
    let node = node_with_strategy(
        &registry,
        &network,
        "self",
        &[],
        std::slice::from_ref(&t),
        strategy,
    )
    .await;

    // The realized selection is capped at the out-degree but never exceeds the
    // number of available candidates.
    let expected = out_degree.min(candidates.len());
    await_upstream_count(&node, expected, TIMEOUT).await;
    node.upstream_connections()
        .into_iter()
        .map(|(p, _, _)| p)
        .collect()
}

// US1 Independent Test / SC-001 + SC-002: with more candidates than the bound,
// the node selects exactly `out_degree` upstreams, and the same seed reproduces
// the identical selection across two independent runs.
#[tokio::test]
async fn bounded_selection_is_capped_and_reproducible() {
    let candidates = ["c0", "c1", "c2", "c3", "c4", "c5"];
    let out_degree = 3;

    let first = selected_upstreams(7, out_degree, &candidates).await;
    let second = selected_upstreams(7, out_degree, &candidates).await;

    // SC-002: never more than the out-degree bound.
    assert_eq!(
        first.len(),
        out_degree,
        "selection is capped at the out-degree"
    );
    // Every selected peer is one of the candidates (self never selects itself).
    assert!(first
        .iter()
        .all(|p| candidates.contains(&p.to_string().as_str())));
    // SC-001: same seed + membership reproduces an identical selection.
    assert_eq!(
        first, second,
        "the same seed must reproduce the identical upstream selection",
    );
}

// FR-002 across the node boundary: candidates at or below the bound ⇒ all are
// selected (the bound is a ceiling, not a quota).
#[tokio::test]
async fn selects_all_candidates_when_at_or_below_bound() {
    let candidates = ["c0", "c1"];
    let selected = selected_upstreams(7, 5, &candidates).await;
    assert_eq!(
        selected,
        HashSet::from([peer("c0"), peer("c1")]),
        "with fewer candidates than the bound, all are selected",
    );
}

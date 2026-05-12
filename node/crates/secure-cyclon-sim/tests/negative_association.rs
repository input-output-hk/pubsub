//! Regression test for Cyclon's Negative-Association property.
//!
//! The cross-view covariance `Cov = Pr[u ∈ view_v1 ∩ view_v2] − Pr[u ∈ view_v]²`
//! must be strictly negative once the overlay has mixed. The protocol's
//! swap-not-copy descriptor mechanic is what produces this anti-correlation;
//! any future change that breaks it should trip this test.

use std::collections::HashSet;

use secure_cyclon::NodeId;
use secure_cyclon_sim::stats::cross_view_covariance;
use secure_cyclon_sim::SimBuilder;

#[tokio::test(start_paused = true)]
async fn cross_view_covariance_is_negative() {
    const N: usize = 80;
    const VIEW_LEN: usize = 10;
    const SWAP_LEN: usize = 3;
    const WARMUP: usize = 60;
    const SNAPSHOTS: usize = 20;
    const GAP: usize = 90; // ≈ 3 · N / SWAP_LEN

    let sim = SimBuilder::new(N)
        .view_len(VIEW_LEN)
        .swap_len(SWAP_LEN)
        .seed(2026)
        .seeds_per_node(VIEW_LEN)
        .build()
        .await;
    sim.bootstrap_all().await.expect("bootstrap should succeed");
    sim.ticks(WARMUP).await;

    let node_ids: Vec<NodeId> = sim.node_ids.clone();
    let mut snapshots: Vec<Vec<HashSet<NodeId>>> = Vec::with_capacity(SNAPSHOTS);
    for _ in 0..SNAPSHOTS {
        snapshots.push(sim.snapshot_views().await);
        sim.ticks(GAP).await;
    }

    let stats = cross_view_covariance(&snapshots, &node_ids, VIEW_LEN);

    // Marginal-uniformity sanity: if this fails the simulator harness is
    // broken and the covariance number is meaningless.
    let marginal = VIEW_LEN as f64 / (N - 1) as f64;
    assert!(
        (stats.p_single - marginal).abs() < 0.005,
        "p_single = {} drifted from c/(N-1) = {}",
        stats.p_single,
        marginal
    );

    // Sign — Negative Association.
    assert!(
        stats.cov < 0.0,
        "expected negative cross-view covariance, got {}",
        stats.cov
    );

    // Order-of-magnitude band around the strict-conservation reference.
    // Fresh-self-injection narrows |cov| below the bound; nothing should
    // push it more than ~5× past it in the opposite direction.
    let bound = stats.conservation_bound;
    assert!(
        stats.cov > 5.0 * bound,
        "cov = {} is more negative than 5 × bound ({})",
        stats.cov,
        bound
    );
    assert!(
        stats.cov < 0.05 * bound,
        "cov = {} is too close to zero (less than 5% of bound {}); the negative correlation may have disappeared",
        stats.cov,
        bound
    );
}

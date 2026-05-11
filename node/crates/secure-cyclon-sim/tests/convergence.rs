//! Vanilla Cyclon convergence properties (paper §II.B, Fig. 2).
//!
//! After enough gossip cycles, every node's view should be approximately
//! random-graph: indegree closely concentrated around `view_len`, and the
//! overlay connected.

use secure_cyclon_sim::SimBuilder;

fn mean_and_stddev(values: &[usize]) -> (f64, f64) {
    let n = values.len() as f64;
    let mean = values.iter().sum::<usize>() as f64 / n;
    let var = values
        .iter()
        .map(|&v| {
            let d = v as f64 - mean;
            d * d
        })
        .sum::<f64>()
        / n;
    (mean, var.sqrt())
}

#[tokio::test(start_paused = true)]
async fn random_graph_50_nodes_converges_to_tight_indegree() {
    let sim = SimBuilder::new(50)
        .view_len(10)
        .swap_len(3)
        .seed(42)
        .seeds_per_node(3)
        .build()
        .await;
    sim.bootstrap_all().await.unwrap();
    sim.ticks(30).await;

    let dist = sim.indegree_distribution().await;
    let counts: Vec<usize> = dist.values().copied().collect();
    let (mean, stddev) = mean_and_stddev(&counts);

    // Each view holds at most `view_len` slots, so total edges <= N * view_len
    // and the per-node indegree mean is bounded by view_len; once converged
    // every view is full and mean == view_len.
    assert!(
        mean > 8.0 && mean <= 10.0,
        "indegree mean out of range: {mean}"
    );
    // Paper Fig. 2: indegrees are very tightly bound around view_len.
    assert!(stddev < 2.5, "indegree std-dev too high: {stddev}");

    let min = *counts.iter().min().unwrap();
    let max = *counts.iter().max().unwrap();
    assert!(min > 0, "some nodes have no indegree: counts={counts:?}");
    assert!(max < 2 * 10, "indegree exceeds 2 * view_len: max={max}");
}

#[tokio::test(start_paused = true)]
async fn indegree_bounded_100_nodes() {
    let sim = SimBuilder::new(100)
        .view_len(10)
        .swap_len(3)
        .seed(7)
        .seeds_per_node(3)
        .build()
        .await;
    sim.bootstrap_all().await.unwrap();
    sim.ticks(40).await;

    let dist = sim.indegree_distribution().await;
    let max = dist.values().copied().max().unwrap();
    assert!(max < 2 * 10, "max indegree {max} >= 2 * view_len");
}

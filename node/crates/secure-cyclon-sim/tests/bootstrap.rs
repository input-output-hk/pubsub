//! A node bootstrapped from only a couple of seeds reaches a near-full view
//! within a few gossip cycles.

use secure_cyclon_sim::SimBuilder;

#[tokio::test(start_paused = true)]
async fn populates_from_two_seeds_in_20_cycles() {
    let sim = SimBuilder::new(3000)
        .view_len(10)
        .swap_len(3)
        .seed(11)
        .seeds_per_node(2)
        .build()
        .await;
    sim.bootstrap_all().await.unwrap();
    sim.ticks(20).await;

    // Every node has either a full view, or close to it, after 20 cycles
    // despite starting from only 2 seeds.
    for i in 0..sim.node_count() {
        let size = sim.view_size(i).await;
        assert!(
            size >= 8,
            "node {i} view size {size} too low after bootstrap"
        );
    }
}

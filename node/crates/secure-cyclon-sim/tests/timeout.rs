//! An unresponsive peer must not permanently leak descriptor slots in the
//! views of honest nodes.

use secure_cyclon_sim::SimBuilder;

#[tokio::test(start_paused = true)]
async fn unresponsive_peer_does_not_collapse_view() {
    let sim = SimBuilder::new(10)
        .view_len(6)
        .swap_len(3)
        .exchange_timeout_ms(20)
        .seed(99)
        .seeds_per_node(5)
        .build()
        .await;
    sim.bootstrap_all().await.unwrap();
    // Warm the overlay so every node has a near-full view.
    sim.ticks(15).await;

    // Mark node 0 as silently unreachable. Exchanges to it now hang; each
    // initiator's exchange timeout fires and the shipped descriptors are
    // re-inserted so the initiator's view does not bleed slots.
    let victim = sim.node_ids[0];
    sim.network.drop_peer(victim);

    sim.ticks(50).await;

    // Every honest node retains a healthy view even though a peer in their
    // ring is silently unresponsive across many cycles. The victim's
    // descriptor itself can still circulate — it only leaves a view when
    // some node picks it as its oldest peer and the exchange times out.
    for i in 1..sim.node_count() {
        let size = sim.view_size(i).await;
        assert!(
            size >= 4,
            "node {i} view collapsed to {size} after 50 unresponsive cycles"
        );
    }
}

//! View-distribution statistics over snapshots of an N-node Cyclon overlay.
//!
//! The headline measurement is the **cross-view covariance** for a shared
//! peer `u` in two distinct views `v1`, `v2`:
//!
//! ```text
//! Cov = Pr[u ∈ view_v1 ∧ u ∈ view_v2]  −  Pr[u ∈ view_v]²
//! ```
//!
//! `Cov = 0` would mean the two views are independent (the "joint
//! uniformity" property would hold). The descriptor-conservation argument
//! for Cyclon predicts `Cov < 0` of order `-(c/N²) · (1 − c/N)` — slightly
//! anti-correlated views, i.e. Negative Association.

use std::collections::HashSet;

use secure_cyclon::NodeId;

#[derive(Debug, Clone, Copy)]
pub struct CovarianceStats {
    pub n: usize,
    pub view_len: usize,
    pub snapshots: usize,
    /// Average `1[u ∈ view_v]` over distinct (v, u) pairs and snapshots.
    /// If the protocol satisfies marginal uniformity (property 1) this
    /// equals `view_len / (n - 1)` exactly.
    pub p_single: f64,
    /// Average `1[u ∈ view_v1] · 1[u ∈ view_v2]` over distinct triples
    /// (v1, v2, u) and snapshots.
    pub p_both: f64,
    /// `p_both − p_single²`. Zero iff the two view memberships are
    /// independent; negative under descriptor-conservation arguments.
    pub cov: f64,
    /// `cov · n²`. Expected to be roughly constant across n if the
    /// conservation argument is correct.
    pub cov_scaled: f64,
    /// `−(c/n²)·(1 − c/n)` — strict-conservation reference for `cov`.
    pub conservation_bound: f64,
}

/// Compute cross-view covariance statistics across `snapshots`.
///
/// Each snapshot must contain exactly `n` views (in the same node order).
/// Aggregation is closed-form in the in-degrees of each node: for fixed
/// `u`, the number of ordered (v1, v2) pairs with `u ∈ view_v1 ∩ view_v2`
/// and `v1 ≠ v2` is `k_u · (k_u − 1)` where `k_u` is `u`'s in-degree in
/// the snapshot. That makes the loop O(snapshots · n²) for the in-degree
/// pass instead of the naive O(snapshots · n³).
///
/// # Panics
///
/// Panics if `snapshots` is empty, or if any snapshot has a length other
/// than `node_ids.len()`.
pub fn cross_view_covariance(
    snapshots: &[Vec<HashSet<NodeId>>],
    node_ids: &[NodeId],
    view_len: usize,
) -> CovarianceStats {
    assert!(!snapshots.is_empty(), "need at least one snapshot");
    let n = node_ids.len();
    assert!(n >= 3, "need at least 3 nodes to form (v1, v2, u) triples");
    for s in snapshots {
        assert_eq!(s.len(), n, "snapshot length must match node_ids");
    }

    let mut sum_single: u128 = 0;
    let mut sum_both: u128 = 0;

    for snap in snapshots {
        for &u in node_ids {
            let k_u: u64 = snap.iter().filter(|view| view.contains(&u)).count() as u64;
            sum_single += u128::from(k_u);
            sum_both += u128::from(k_u) * u128::from(k_u.saturating_sub(1));
        }
    }

    let count_single = snapshots.len() as u128 * (n as u128) * ((n - 1) as u128);
    let count_both = snapshots.len() as u128 * (n as u128) * ((n - 1) as u128) * ((n - 2) as u128);

    let p_single = sum_single as f64 / count_single as f64;
    let p_both = sum_both as f64 / count_both as f64;
    let cov = p_both - p_single * p_single;
    let n_f = n as f64;
    let c_f = view_len as f64;
    let conservation_bound = -(c_f / (n_f * n_f)) * (1.0 - c_f / n_f);

    CovarianceStats {
        n,
        view_len,
        snapshots: snapshots.len(),
        p_single,
        p_both,
        cov,
        cov_scaled: cov * n_f * n_f,
        conservation_bound,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secure_cyclon::NodeId;

    fn id(byte: u8) -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = byte;
        NodeId::from_bytes(bytes)
    }

    #[test]
    fn fully_independent_views_give_zero_covariance() {
        // Five nodes, each holding {self + 1, self + 2} (mod 5) in its view.
        // Each peer has in-degree exactly 2, so cross-view membership is
        // identical for every peer. The covariance turns out negative
        // because in-degree variance is zero (no spread to drive p_both up).
        let ids: Vec<NodeId> = (0..5u8).map(id).collect();
        let mut views: Vec<HashSet<NodeId>> = Vec::new();
        for i in 0..5 {
            let mut s = HashSet::new();
            s.insert(ids[(i + 1) % 5]);
            s.insert(ids[(i + 2) % 5]);
            views.push(s);
        }
        let stats = cross_view_covariance(&[views], &ids, 2);
        // Sanity: p_single = c / (n-1) = 2/4 = 0.5 exactly.
        assert!((stats.p_single - 0.5).abs() < 1e-12);
        // Every peer has k=2 → sum_both = n · k(k-1) = 5·2 = 10
        // count_both = n(n-1)(n-2) = 60 → p_both = 1/6.
        assert!((stats.p_both - 1.0 / 6.0).abs() < 1e-12);
        // cov = 1/6 - 1/4 = -1/12.
        assert!((stats.cov - (-1.0 / 12.0)).abs() < 1e-12);
    }

    #[test]
    fn perfectly_clustered_views_give_positive_covariance() {
        // Three nodes; nodes 0 and 1 both have node 2 in their view, node
        // 2 has nobody. p_single = 2/6 = 1/3. p_both: ordered (v1,v2,u)
        // with u ∈ view_v1 ∩ view_v2 — only (0,1,2) and (1,0,2) qualify =
        // 2; count_both = 3·2·1 = 6 → p_both = 1/3. cov = 1/3 - 1/9 = 2/9.
        let ids: Vec<NodeId> = (0..3u8).map(id).collect();
        let v0: HashSet<NodeId> = [ids[2]].into_iter().collect();
        let v1: HashSet<NodeId> = [ids[2]].into_iter().collect();
        let v2: HashSet<NodeId> = HashSet::new();
        let stats = cross_view_covariance(&[vec![v0, v1, v2]], &ids, 1);
        assert!((stats.p_single - 1.0 / 3.0).abs() < 1e-12);
        assert!((stats.p_both - 1.0 / 3.0).abs() < 1e-12);
        assert!((stats.cov - 2.0 / 9.0).abs() < 1e-12);
    }
}

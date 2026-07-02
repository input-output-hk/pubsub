//! The shared **verifiable edge predicate** and its bucket/cap formulae
//! (bucketed-pull, ADR 0024/0025/0030).
//!
//! Both seams consult this one module — the dial side to *select* upstreams, the
//! accept side to *verify* an inbound request — so the two can never drift: an
//! edge is admissible iff [`is_valid_edge`] holds for the same tuple on both ends.

use sha2::{Digest, Sha256};

use crate::peer::PeerId;
use crate::topic::TopicId;

/// Domain-separation tag so the edge predicate never shares a hash domain with
/// any other SHA-256 use in the crate (e.g. `MessageHash`).
const EDGE_DOMAIN: &[u8] = b"pubsub/bucketed-pull/edge/v1";

/// Per-topic bucket count for a fixed target connection degree `target_degree`: `max(1, round(candidates / target_degree))`.
///
/// Expected valid edges per topic = `candidates / B ≈ target_degree`. When there are `≤ ~target_degree`
/// candidates, `B` floors to **1** and [`is_valid_edge`] always holds — the
/// connect-to-all small-topic fallback, with no threshold and no `ln` degeneracy
/// (ADR 0024).
#[must_use]
pub fn bucket_count(candidates_len: usize, target_degree: usize) -> usize {
    if target_degree == 0 {
        return 1;
    }
    #[allow(clippy::cast_precision_loss)]
    let ratio = candidates_len as f64 / target_degree as f64;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let buckets = ratio.round() as usize;
    buckets.max(1)
}

/// The per-topic downstream accept cap for a fixed target connection degree `target_degree`: `⌈target_degree + c·√target_degree⌉`
/// (the `OC` variance buffer of `docs/extensions/bucketed-pull.md`; `c` default 3).
#[must_use]
pub fn accept_cap(target_degree: usize, c: usize) -> usize {
    #[allow(clippy::cast_precision_loss)]
    let cap = target_degree as f64 + (c as f64) * (target_degree as f64).sqrt();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let cap = cap.ceil() as usize;
    cap
}

/// The verifiable directional edge predicate: `requester → candidate` on `topic`
/// at `interval` is valid iff `H(genesis, topic, requester, candidate, interval)
/// mod buckets == 0` (ADR 0024). Pure and public — both peers compute it, so the
/// acceptor verifies rather than trusts.
///
/// `buckets == 1` (the small-topic floor) makes every edge valid (connect-to-all).
#[must_use]
pub fn is_valid_edge(
    genesis: u64,
    topic: &TopicId,
    requester: &PeerId,
    candidate: &PeerId,
    interval: u64,
    buckets: usize,
) -> bool {
    // Length-prefix each variable-width component so distinct tuples cannot
    // collide via concatenation.
    #[allow(clippy::cast_possible_truncation)]
    fn feed(hasher: &mut Sha256, bytes: &[u8]) {
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }

    if buckets <= 1 {
        return true;
    }

    let mut hasher = Sha256::new();
    feed(&mut hasher, EDGE_DOMAIN);
    hasher.update(genesis.to_le_bytes());
    feed(&mut hasher, topic.to_string().as_bytes());
    feed(&mut hasher, requester.to_string().as_bytes());
    feed(&mut hasher, candidate.to_string().as_bytes());
    hasher.update(interval.to_le_bytes());
    let digest: [u8; 32] = hasher.finalize().into();

    // Reduce the leading 8 bytes modulo the bucket count.
    let value = u64::from_le_bytes(digest[..8].try_into().expect("8 bytes"));
    value % (buckets as u64) == 0
}

#[cfg(test)]
mod tests {
    use super::{accept_cap, bucket_count, is_valid_edge};
    use crate::peer::PeerId;
    use crate::topic::TopicId;
    use std::str::FromStr;

    fn peer(s: &str) -> PeerId {
        PeerId::from_str(s).expect("valid peer id")
    }
    fn topic(s: &str) -> TopicId {
        TopicId::from_str(s).expect("valid topic id")
    }

    #[test]
    fn bucket_count_floors_at_one_for_small_topics() {
        assert_eq!(bucket_count(0, 8), 1);
        assert_eq!(bucket_count(4, 8), 1); // 4/8 rounds to 0 -> floored to 1
        assert_eq!(bucket_count(8, 8), 1); // exactly target_degree -> 1
        assert_eq!(bucket_count(80, 8), 10); // 80/8 = 10
    }

    #[test]
    fn accept_cap_is_degree_plus_buffer() {
        // target_degree=8, c=3 -> 8 + 3*sqrt(8) = 8 + 8.485... = 16.48 -> 17
        assert_eq!(accept_cap(8, 3), 17);
        // target_degree=3, c=3 -> 3 + 3*sqrt(3) = 8.196 -> 9 (doc example ~8)
        assert_eq!(accept_cap(3, 3), 9);
    }

    #[test]
    fn buckets_one_admits_every_edge() {
        // The connect-to-all fallback: buckets == 1 -> always valid.
        assert!(is_valid_edge(0, &topic("t1"), &peer("a"), &peer("b"), 0, 1));
    }

    #[test]
    fn predicate_is_deterministic_and_directional() {
        let g = 7;
        let t = topic("t1");
        let ab = is_valid_edge(g, &t, &peer("a"), &peer("b"), 0, 4);
        // Deterministic: same inputs, same result.
        assert_eq!(ab, is_valid_edge(g, &t, &peer("a"), &peer("b"), 0, 4));
        // Directional: (a->b) and (b->a) are independent draws (not required to
        // differ, but computed over distinct tuples).
        let _ba = is_valid_edge(g, &t, &peer("b"), &peer("a"), 0, 4);
    }

    // Over a sweep of intervals the accepted fraction approximates 1/B.
    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn edge_density_approximates_one_over_buckets() {
        let buckets = 8usize;
        let sweeps = 4000u64;
        let hits = (0..sweeps)
            .filter(|i| is_valid_edge(0, &topic("t1"), &peer("a"), &peer("b"), *i, buckets))
            .count();
        let frac = hits as f64 / sweeps as f64;
        let expected = 1.0 / buckets as f64;
        assert!(
            (frac - expected).abs() < 0.03,
            "edge density {frac:.3} deviates from 1/B = {expected:.3}",
        );
    }
}

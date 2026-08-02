//! The shared **verifiable edge predicate** (bucketed-pull, ADR 0024/0025/0030).
//!
//! Both seams consult this one module — the dial side to *select* upstreams, the
//! accept side to *verify* an inbound request — so the two can never drift: an
//! edge is admissible iff [`is_valid_edge`] holds for the same tuple on both
//! ends. The bucket count `B` the predicate is evaluated at is **fed
//! configuration on both ends** — one per-seam value drives the dial gate and
//! the acceptor's verification, which is the agreement condition
//! verifiability rests on. (The pre-017 derive-from-membership arm and the
//! bucket/cap formulae are gone; the balanced-point and cap-headroom guidance
//! lives with the model recipes as parameter-choosing documentation.)

use sha2::{Digest, Sha256};

use crate::message::push_len_prefixed;
use crate::peer::PeerId;
use crate::topic::TopicId;

/// Domain-separation tag for the directional **relay** edge predicate —
/// exclusively the relay seams' draw (publisher and symmetric edges have
/// their own tags below), and never shared with any other SHA-256 use in the
/// crate (e.g. `MessageHash`). Renamed from `…/edge/v1` alongside the 015
/// link-kind split, before any experiment results existed to keep
/// reproducible; the version suffix is the predicate's version knob — the one
/// place a pre-image change is recorded once the protocol has released peers
/// to stay compatible with (pre-release iterations keep it at `v1`).
const RELAY_EDGE_DOMAIN: &[u8] = b"pubsub/bucketed-pull/relay-edge/v1";

/// Domain tag for **publisher-link** edges: an independent draw from the relay
/// domain, so a node's publisher targets are uncorrelated with its relay
/// upstreams over the same nonce/topic/pair (feature 015, M3).
const PUBLISHER_EDGE_DOMAIN: &[u8] = b"pubsub/bucketed-pull/publisher-edge/v1";

/// Domain tag for **symmetric** relay edges (feature 015, M4): its own domain,
/// so the symmetric draw is independent of the directional ones — a pair whose
/// directional preimage happens to already be in canonical order must not
/// correlate with the directional predicate.
const SYM_EDGE_DOMAIN: &[u8] = b"pubsub/bucketed-pull/edge-sym/v1";

/// The verifiable directional edge predicate: `requester → candidate` on `topic`
/// under the epoch `nonce` is valid iff `H(nonce, topic, requester, candidate)
/// mod buckets == 0` (ADR 0024/0031). Pure and public — both peers compute it,
/// so the acceptor verifies rather than trusts. The `nonce` is the epoch
/// randomness context (v1: the configured genesis; later: an externally
/// observable beacon such as a block hash).
///
/// `buckets == 1` (the small-topic floor) makes every edge valid (connect-to-all).
#[must_use]
pub fn is_valid_edge(
    nonce: u64,
    topic: &TopicId,
    requester: &PeerId,
    candidate: &PeerId,
    buckets: usize,
) -> bool {
    is_valid_edge_in(
        RELAY_EDGE_DOMAIN,
        nonce,
        topic,
        requester,
        candidate,
        buckets,
    )
}

/// The publisher-link twin of [`is_valid_edge`]: the same directional
/// predicate under the dedicated publisher domain — an independent draw, so
/// selecting a peer as a relay upstream says nothing about selecting it as a
/// publisher target (feature 015).
#[must_use]
pub fn is_valid_edge_publisher(
    nonce: u64,
    topic: &TopicId,
    requester: &PeerId,
    candidate: &PeerId,
    buckets: usize,
) -> bool {
    is_valid_edge_in(
        PUBLISHER_EDGE_DOMAIN,
        nonce,
        topic,
        requester,
        candidate,
        buckets,
    )
}

/// The **symmetric** edge predicate (feature 015, M4): valid for the
/// *unordered* pair — the two peers are fed to the hash in canonical byte
/// order, so both ends compute the identical draw and, when it holds, both
/// dial each other. Each such edge materialises as a reciprocal pair of
/// directed links on both sides; bidirectionality is emergent, not stored.
#[must_use]
pub fn is_valid_edge_sym(
    nonce: u64,
    topic: &TopicId,
    one: &PeerId,
    other: &PeerId,
    buckets: usize,
) -> bool {
    // Canonical order: lexicographic on the raw public-key bytes (the same
    // bytes the preimage encodes), so the two ends cannot disagree.
    let (first, second) = if one.as_public_key().as_bytes() <= other.as_public_key().as_bytes() {
        (one, other)
    } else {
        (other, one)
    };
    is_valid_edge_in(SYM_EDGE_DOMAIN, nonce, topic, first, second, buckets)
}

/// The one predicate body, parameterised by hash domain.
fn is_valid_edge_in(
    domain: &[u8],
    nonce: u64,
    topic: &TopicId,
    requester: &PeerId,
    candidate: &PeerId,
    buckets: usize,
) -> bool {
    if buckets <= 1 {
        return true;
    }

    // Build the canonical pre-image with the crate's one length-prefix primitive
    // (`message::push_len_prefixed`) so a future canonical-encoding change touches
    // a single place, then hash it. Variable-width components are length-prefixed
    // so distinct tuples cannot collide via concatenation; `nonce` is fixed-width.
    // Peers are fed by **raw key bytes** (`PeerId`'s `Display` is non-injective —
    // an alias, its hex, and the mock suffix can all collide); the topic by its
    // exact string.
    let mut preimage = Vec::new();
    push_len_prefixed(&mut preimage, domain);
    preimage.extend_from_slice(&nonce.to_le_bytes());
    push_len_prefixed(&mut preimage, topic.as_str().as_bytes());
    push_len_prefixed(&mut preimage, requester.as_public_key().as_bytes());
    push_len_prefixed(&mut preimage, candidate.as_public_key().as_bytes());
    let digest: [u8; 32] = Sha256::digest(&preimage).into();

    // Reduce the leading 8 bytes modulo the bucket count.
    let value = u64::from_le_bytes(digest[..8].try_into().expect("8 bytes"));
    value % (buckets as u64) == 0
}

#[cfg(test)]
mod tests {
    use super::is_valid_edge;
    use crate::strategies::test_support::{peer, topic};

    #[test]
    fn buckets_one_admits_every_edge() {
        // The connect-to-all fallback: buckets == 1 -> always valid.
        assert!(is_valid_edge(0, &topic("t1"), &peer("a"), &peer("b"), 1));
    }

    #[test]
    fn predicate_is_deterministic_and_directional() {
        let nonce = 7;
        let t = topic("t1");
        let ab = is_valid_edge(nonce, &t, &peer("a"), &peer("b"), 4);
        // Deterministic: same inputs, same result.
        assert_eq!(ab, is_valid_edge(nonce, &t, &peer("a"), &peer("b"), 4));
        // Directional: (a->b) and (b->a) are independent draws (not required to
        // differ, but computed over distinct tuples).
        let _ba = is_valid_edge(nonce, &t, &peer("b"), &peer("a"), 4);
    }

    // 015 M4: the symmetric predicate is order-independent — both ends compute
    // the identical draw for the unordered pair.
    #[test]
    fn symmetric_predicate_is_order_independent() {
        use super::is_valid_edge_sym;
        let t = topic("t1");
        for nonce in 0..64u64 {
            assert_eq!(
                is_valid_edge_sym(nonce, &t, &peer("a"), &peer("b"), 4),
                is_valid_edge_sym(nonce, &t, &peer("b"), &peer("a"), 4),
                "nonce {nonce}: the two ends must agree",
            );
        }
    }

    // 015 M4: the symmetric domain is independent of the directional one — for
    // an already-canonically-ordered pair the two draws must still diverge
    // somewhere over a nonce sweep (same preimage layout, different domain).
    #[test]
    fn symmetric_domain_is_an_independent_draw() {
        use super::is_valid_edge_sym;
        let t = topic("t1");
        // "a" < "b" in key-byte order, so the directional (a→b) preimage equals
        // the symmetric pair's ordering — only the domain tag differs.
        let diverges = (0..256u64).any(|nonce| {
            is_valid_edge(nonce, &t, &peer("a"), &peer("b"), 4)
                != is_valid_edge_sym(nonce, &t, &peer("a"), &peer("b"), 4)
        });
        assert!(
            diverges,
            "the symmetric domain must not mirror the directional one"
        );
    }

    // Over a sweep of nonces the symmetric density also approximates 1/B.
    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn symmetric_density_approximates_one_over_buckets() {
        use super::is_valid_edge_sym;
        let buckets = 8usize;
        let sweeps = 4000u64;
        let hits = (0..sweeps)
            .filter(|nonce| {
                is_valid_edge_sym(*nonce, &topic("t1"), &peer("a"), &peer("b"), buckets)
            })
            .count();
        let frac = hits as f64 / sweeps as f64;
        assert!(
            (frac - 1.0 / buckets as f64).abs() < 0.03,
            "symmetric density {frac:.3} deviates from 1/B",
        );
    }

    // 015: the publisher domain is an independent draw — over a nonce sweep the
    // relay and publisher predicates must disagree somewhere (same tuple).
    #[test]
    fn publisher_domain_is_an_independent_draw() {
        use super::is_valid_edge_publisher;
        let t = topic("t1");
        let diverges = (0..256u64).any(|nonce| {
            is_valid_edge(nonce, &t, &peer("a"), &peer("b"), 4)
                != is_valid_edge_publisher(nonce, &t, &peer("a"), &peer("b"), 4)
        });
        assert!(diverges, "the two domains must not be correlated");
    }

    // Over a sweep of epoch nonces the accepted fraction approximates 1/B.
    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn edge_density_approximates_one_over_buckets() {
        let buckets = 8usize;
        let sweeps = 4000u64;
        let hits = (0..sweeps)
            .filter(|nonce| is_valid_edge(*nonce, &topic("t1"), &peer("a"), &peer("b"), buckets))
            .count();
        let frac = hits as f64 / sweeps as f64;
        let expected = 1.0 / buckets as f64;
        assert!(
            (frac - expected).abs() < 0.03,
            "edge density {frac:.3} deviates from 1/B = {expected:.3}",
        );
    }
}

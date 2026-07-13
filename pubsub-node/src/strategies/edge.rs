//! The shared **verifiable edge predicate** and its bucket/cap formulae
//! (bucketed-pull, ADR 0024/0025/0030).
//!
//! Both seams consult this one module — the dial side to *select* upstreams, the
//! accept side to *verify* an inbound request — so the two can never drift: an
//! edge is admissible iff [`is_valid_edge`] holds for the same tuple on both ends.

use sha2::{Digest, Sha256};

use crate::connection_state::LinkRole;
use crate::message::push_len_prefixed;
use crate::peer::PeerId;
use crate::topic::TopicId;

/// Domain-separation tag so the edge predicate never shares a hash domain with
/// any other SHA-256 use in the crate (e.g. `MessageHash`). The version suffix
/// is the predicate's version knob — the one place a pre-image change is
/// recorded once the protocol has released peers to stay compatible with
/// (pre-release iterations keep it at `v1`).
const EDGE_DOMAIN: &[u8] = b"pubsub/bucketed-pull/edge/v1";

/// Domain-separation tag for **publishing-link** edges (feature 015, ADR 0033).
/// A distinct tag makes the publish edge set an independent hash draw from the
/// relay edge set for the same `(nonce, topic, requester, candidate)` tuple —
/// sweeping either seam's degree leaves the other's selection untouched.
const PUBLISH_EDGE_DOMAIN: &[u8] = b"pubsub/bucketed-pull/publish-edge/v1";

/// Domain-separation tags for the **symmetric** edge predicates (feature 015,
/// ADR 0035 — the M4 bidirectional mode). Distinct tags make the symmetric
/// draws independent of the directional ones for every pair, including pairs
/// whose directional preimage happens to already be in canonical order.
const EDGE_SYM_DOMAIN: &[u8] = b"pubsub/bucketed-pull/edge-sym/v1";
const PUBLISH_EDGE_SYM_DOMAIN: &[u8] = b"pubsub/bucketed-pull/publish-edge-sym/v1";

/// The domain-separation tag for a link role's edge predicate.
const fn edge_domain(role: LinkRole) -> &'static [u8] {
    match role {
        LinkRole::Relay => EDGE_DOMAIN,
        LinkRole::Publisher => PUBLISH_EDGE_DOMAIN,
    }
}

/// The domain-separation tag for a link role's **symmetric** edge predicate.
const fn edge_sym_domain(role: LinkRole) -> &'static [u8] {
    match role {
        LinkRole::Relay => EDGE_SYM_DOMAIN,
        LinkRole::Publisher => PUBLISH_EDGE_SYM_DOMAIN,
    }
}

/// Per-topic bucket count for a fixed target degree (`relay_degree` on the
/// relay seams, `publish_degree` on the publish seams): `max(1, round(candidates / degree))`.
///
/// Expected valid edges per topic = `candidates / B ≈ degree`. When there are `≤ ~degree`
/// candidates, `B` floors to **1** and [`is_valid_edge`] always holds — the
/// connect-to-all small-topic fallback, with no threshold and no `ln` degeneracy
/// (ADR 0024).
///
/// **Verifiability caveat (matters once a discovery layer lands).** The dialer
/// and acceptor must compute the *same* `B` for the predicate to be verifiable.
/// Today they do because v1 uses the **full candidate set** — every node's
/// `candidates_len` for a topic is `S_T − 1` (all members minus itself), so `B`
/// is uniform. Once per-node view sampling (`H_v`) is introduced, two nodes will
/// see different subsets and this local count will diverge — `B` must then derive
/// from a **globally-agreed** per-topic count (the registry's `S_T`, or a fixed
/// `H_v` parameter), *not* the sampled view size, or verification silently breaks.
#[must_use]
pub fn bucket_count(candidates_len: usize, degree: usize) -> usize {
    if degree == 0 {
        return 1;
    }
    // round(len / degree) in exact integer arithmetic — no float
    // precision questions in a predicate both peers must agree on.
    ((candidates_len + degree / 2) / degree).max(1)
}

/// The bucket count both seams feed the predicate: the pinned `bucket_override`
/// when configured (`--bucket-count` — verifiable by construction, both peers
/// use the same value), else derived per topic via [`bucket_count`].
///
/// This is the **one** place the derive-or-override rule lives: the dial and
/// accept seams both call it, so a future change to the derivation (e.g. the
/// globally-agreed count the `H_v` caveat on [`bucket_count`] anticipates)
/// cannot be applied to one side and silently break verification on the other.
///
/// Note a pinned override replaces the derived value **including the small-topic
/// `B = 1` floor**: an override larger than a topic's candidate count can leave
/// a node with zero upstreams on that topic (no retry/back-fill).
#[must_use]
pub fn resolve_buckets(
    bucket_override: Option<usize>,
    candidates_len: usize,
    degree: usize,
) -> usize {
    bucket_override.unwrap_or_else(|| bucket_count(candidates_len, degree))
}

/// The per-topic inbound accept cap for a fixed target degree (`relay_degree`
/// or `publish_degree`, per seam): `⌈degree + c·√degree⌉`
/// (the `OC` variance buffer of `docs/extensions/bucketed-pull.md`; `c` default 3).
#[must_use]
pub fn accept_cap(degree: usize, c: usize) -> usize {
    #[allow(clippy::cast_precision_loss)]
    let cap = degree as f64 + (c as f64) * (degree as f64).sqrt();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let cap = cap.ceil() as usize;
    cap
}

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
    is_valid_edge_for(LinkRole::Relay, nonce, topic, requester, candidate, buckets)
}

/// The edge predicate for a link `role`: the relay predicate under the relay
/// domain tag (byte-identical to [`is_valid_edge`]), or the publishing-link
/// predicate under its own domain tag — an independent hash draw (ADR 0033).
#[must_use]
pub fn is_valid_edge_for(
    role: LinkRole,
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
    push_len_prefixed(&mut preimage, edge_domain(role));
    preimage.extend_from_slice(&nonce.to_le_bytes());
    push_len_prefixed(&mut preimage, topic.as_str().as_bytes());
    push_len_prefixed(&mut preimage, requester.as_public_key().as_bytes());
    push_len_prefixed(&mut preimage, candidate.as_public_key().as_bytes());
    let digest: [u8; 32] = Sha256::digest(&preimage).into();

    // Reduce the leading 8 bytes modulo the bucket count.
    let value = u64::from_le_bytes(digest[..8].try_into().expect("8 bytes"));
    value % (buckets as u64) == 0
}

/// The **symmetric** edge predicate (feature 015, ADR 0035 — the M4
/// bidirectional mode): the edge between `a` and `b` on `topic` under the
/// epoch `nonce` is valid iff the hash of the **unordered** pair passes the
/// bucket test — `is_valid_edge_sym(…, a, b, …) == is_valid_edge_sym(…, b, a, …)`
/// by construction (the two keys are fed in canonical byte order). Both ends
/// therefore compute the same expected edge set, so each dials the other and
/// the link materialises as the Out+In pair on both sides (research R10 —
/// bidirectionality is emergent, no stored `Both`).
///
/// Same verifiability, purity, and `buckets <= 1` connect-to-all floor as the
/// directional predicate; drawn from its own domain tag, independent of the
/// directional draws.
#[must_use]
pub fn is_valid_edge_sym(
    role: LinkRole,
    nonce: u64,
    topic: &TopicId,
    a: &PeerId,
    b: &PeerId,
    buckets: usize,
) -> bool {
    if buckets <= 1 {
        return true;
    }

    // Canonical pair order: lower raw key bytes first, so both ends build the
    // identical preimage regardless of who evaluates.
    let (lo, hi) = if a.as_public_key().as_bytes() <= b.as_public_key().as_bytes() {
        (a, b)
    } else {
        (b, a)
    };
    let mut preimage = Vec::new();
    push_len_prefixed(&mut preimage, edge_sym_domain(role));
    preimage.extend_from_slice(&nonce.to_le_bytes());
    push_len_prefixed(&mut preimage, topic.as_str().as_bytes());
    push_len_prefixed(&mut preimage, lo.as_public_key().as_bytes());
    push_len_prefixed(&mut preimage, hi.as_public_key().as_bytes());
    let digest: [u8; 32] = Sha256::digest(&preimage).into();

    let value = u64::from_le_bytes(digest[..8].try_into().expect("8 bytes"));
    value % (buckets as u64) == 0
}

#[cfg(test)]
mod tests {
    use super::{accept_cap, bucket_count, is_valid_edge};
    use crate::strategies::test_support::{peer, topic};

    #[test]
    fn bucket_count_floors_at_one_for_small_topics() {
        assert_eq!(bucket_count(0, 8), 1);
        assert_eq!(bucket_count(4, 8), 1); // 4/8 rounds to 0 -> floored to 1
        assert_eq!(bucket_count(8, 8), 1); // exactly degree -> 1
        assert_eq!(bucket_count(80, 8), 10); // 80/8 = 10
    }

    #[test]
    fn accept_cap_is_degree_plus_buffer() {
        // degree=8, c=3 -> 8 + 3*sqrt(8) = 8 + 8.485... = 16.48 -> 17
        assert_eq!(accept_cap(8, 3), 17);
        // degree=3, c=3 -> 3 + 3*sqrt(3) = 8.196 -> 9 (doc example ~8)
        assert_eq!(accept_cap(3, 3), 9);
    }

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

    // ADR 0035: the symmetric predicate is order-independent and an
    // independent draw from the directional one.
    #[test]
    fn symmetric_predicate_is_symmetric_and_domain_separated() {
        use super::is_valid_edge_sym;
        use crate::connection_state::LinkRole;
        let t = topic("t1");
        for nonce in 0..64u64 {
            assert_eq!(
                is_valid_edge_sym(LinkRole::Relay, nonce, &t, &peer("a"), &peer("b"), 4),
                is_valid_edge_sym(LinkRole::Relay, nonce, &t, &peer("b"), &peer("a"), 4),
                "symmetric in its peers",
            );
        }
        // Independent draw: over a sweep, symmetric and directional must
        // disagree at least once.
        let differs = (0..200u64).any(|nonce| {
            is_valid_edge_sym(LinkRole::Relay, nonce, &t, &peer("a"), &peer("b"), 4)
                != is_valid_edge(nonce, &t, &peer("a"), &peer("b"), 4)
        });
        assert!(differs, "symmetric draw is domain-separated");
    }

    // ADR 0035: symmetric edge density also approximates 1/B.
    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn symmetric_edge_density_approximates_one_over_buckets() {
        use super::is_valid_edge_sym;
        use crate::connection_state::LinkRole;
        let buckets = 8usize;
        let sweeps = 4000u64;
        let hits = (0..sweeps)
            .filter(|nonce| {
                is_valid_edge_sym(
                    LinkRole::Relay,
                    *nonce,
                    &topic("t1"),
                    &peer("a"),
                    &peer("b"),
                    buckets,
                )
            })
            .count();
        let frac = hits as f64 / sweeps as f64;
        assert!(
            (frac - 1.0 / buckets as f64).abs() < 0.03,
            "symmetric edge density {frac:.3} deviates from 1/B",
        );
    }
}

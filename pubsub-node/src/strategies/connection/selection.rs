//! The unified dial-side selection policy: [`Selection`] — one implementation
//! over the two plane knobs, the **bucket count** (hash-gate width) and the
//! **pick count** (seeded uniform picks among gate survivors).

use std::collections::BTreeSet;

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use sha2::{Digest, Sha256};

use super::ConnectionStrategy;
use crate::connection_state::LinkKind;
use crate::message::push_len_prefixed;
use crate::peer::PeerId;
use crate::strategies::edge::{is_valid_edge, is_valid_edge_publisher, is_valid_edge_sym};
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

/// Domain tag for the **relay** instance's per-topic draw — the sampling
/// twin of the edge predicate's per-seam domain split, so one node's two
/// seam instances never derive the same RNG stream over a shared seed.
const RELAY_DRAW_DOMAIN: &[u8] = b"pubsub/uniform-selection/relay/v1";

/// Domain tag for the **publisher** instance's per-topic draw: an
/// independent stream from the relay instance's over the same (seed,
/// self-identity, nonce, topic). There is no symmetric draw domain — the
/// symmetric switch changes the gate predicate and the handshake vocabulary,
/// not the draw.
const PUBLISHER_DRAW_DOMAIN: &[u8] = b"pubsub/uniform-selection/publisher/v1";

/// The unified link-selection policy: per subscribed topic, **gate**
/// candidates by the seam's verifiable edge predicate at the configured
/// bucket count, then **pick** exactly `min(pick count, survivors)` seeded
/// uniform picks without replacement.
///
/// The two knobs are independently optional:
///
/// - **bucket count** absent ≡ `B = 1`: every candidate survives the gate
///   (the predicate's existing short-circuit — the filter is skipped
///   entirely). Present: survivors are exactly the candidates the shared
///   edge predicate admits under the seam's hash domain, so every dialed
///   edge is acceptor-checkable.
/// - **pick count** absent: dial every gate survivor. `0`: dial none.
///   `k ≥ 1`: exactly `min(k, survivors)` uniform picks, without
///   replacement, drawn from the instance's seed.
///
/// The four pre-017 dial strategies are points of this plane: connect-to-all
/// (both absent), uniform sampling (pick count only), hash-gated (bucket
/// count only), and gated picks (both).
///
/// Selection is pure and reproducible: the result is a function of the
/// instance's fields and the [`NodeView`] (candidate sets, epoch nonce),
/// order-independent in the candidate set — repeated dial ticks over the
/// same view re-dial the same set (the heartbeat retry primitive). The seed
/// is read only when a pick count `≥ 1` is configured.
// 017-FR-001, 017-FR-002, 017-FR-004 (per-seam instances + symmetric
// composition); research R1/R2.
pub struct Selection {
    self_id: PeerId,
    kind: LinkKind,
    symmetric: bool,
    bucket_count: Option<usize>,
    pick_count: Option<usize>,
    seed: [u8; 32],
}

impl Selection {
    /// Build the policy at the plane origin — both knobs absent, so every
    /// candidate on every joined topic is expected (the connect-to-all
    /// default). `seed` feeds the pick draw and is read only once a pick
    /// count is configured via [`with_pick_count`](Self::with_pick_count).
    // 017-FR-016: the constructor takes the 32 seed bytes directly — the
    // experiments driver's per-participant seed injection is unchanged.
    #[must_use]
    pub fn new(self_id: PeerId, seed: [u8; 32]) -> Self {
        Self {
            self_id,
            kind: LinkKind::Relay,
            symmetric: false,
            bucket_count: None,
            pick_count: None,
            seed,
        }
    }

    /// Configure the bucket count `B` the gate filters at. `None` (the
    /// constructor default) is ungated — equivalent to `B = 1`, where the
    /// predicate admits everyone. A pinned `B` larger than a topic's
    /// candidate count can leave zero survivors, hence zero expected links
    /// on that topic (no retry) — the parameter-setter's responsibility.
    #[must_use]
    pub fn with_bucket_count(mut self, bucket_count: Option<usize>) -> Self {
        self.bucket_count = bucket_count;
        self
    }

    /// Configure the pick count: `None` (the constructor default) expects
    /// every gate survivor; `Some(k)` draws exactly `min(k, survivors)`
    /// seeded uniform picks; `Some(0)` expects nothing (the dial-none
    /// boundary).
    #[must_use]
    pub fn with_pick_count(mut self, pick_count: Option<usize>) -> Self {
        self.pick_count = pick_count;
        self
    }

    /// Re-target the instance at a link kind (`Relay` is the constructor
    /// default): the kind selects both the hash domain the gate filters
    /// under and the domain the pick draw derives from — the publisher
    /// instance's gate AND picks are independent draws from the relay
    /// instance's over the same view and seed.
    #[must_use]
    pub fn for_kind(mut self, kind: LinkKind) -> Self {
        self.kind = kind;
        self
    }

    /// Switch the gate to the **symmetric** edge predicate: survivors are
    /// gated for the unordered pair under the dedicated symmetric domain, so
    /// both ends of a valid edge see it and reciprocity is constructed by
    /// the symmetric handshake (ADR 0034). Applies to relay instances; the
    /// publisher seam stays directional and never sets this.
    #[must_use]
    pub fn with_symmetric(mut self, symmetric: bool) -> Self {
        self.symmetric = symmetric;
        self
    }

    /// The per-topic sampling seed feeding the pick draw's RNG:
    /// `SHA-256( lp(domain) ‖ lp(seed) ‖ lp(self-id key bytes) ‖ nonce_le8 ‖
    /// lp(topic) )` with `lp` the crate's one length-prefix primitive and the
    /// domain selected per seam by the instance's link kind. Each component
    /// carries a property: the per-seam domain → one node's relay and
    /// publisher instances draw independently; self-identity → a
    /// fleet-shared seed still yields per-node-independent draws; the epoch
    /// nonce → picks re-randomise on an epoch change and stay stable across
    /// heartbeats within one; the length prefixes → no concatenation
    /// collisions across distinct tuples.
    // 017-FR-015/FR-026 commit B (017-T024): supersedes the commit-A
    // reproduction of the experiments sampler derivation — the one
    // deliberate, re-baselined result change (research R2; analysis.md I2).
    fn topic_seed(&self, nonce: u64, topic: &TopicId) -> [u8; 32] {
        let domain = match self.kind {
            LinkKind::Relay => RELAY_DRAW_DOMAIN,
            LinkKind::Publisher => PUBLISHER_DRAW_DOMAIN,
        };
        let mut preimage = Vec::new();
        push_len_prefixed(&mut preimage, domain);
        push_len_prefixed(&mut preimage, &self.seed);
        push_len_prefixed(&mut preimage, self.self_id.as_public_key().as_bytes());
        preimage.extend_from_slice(&nonce.to_le_bytes());
        push_len_prefixed(&mut preimage, topic.as_str().as_bytes());
        Sha256::digest(&preimage).into()
    }

    /// Whether the gate admits `candidate` on `topic` at `buckets` — the
    /// seam's edge predicate under the instance's hash domain.
    fn gate_admits(&self, nonce: u64, topic: &TopicId, candidate: &PeerId, buckets: usize) -> bool {
        if self.symmetric {
            is_valid_edge_sym(nonce, topic, &self.self_id, candidate, buckets)
        } else {
            match self.kind {
                LinkKind::Relay => is_valid_edge(nonce, topic, &self.self_id, candidate, buckets),
                LinkKind::Publisher => {
                    is_valid_edge_publisher(nonce, topic, &self.self_id, candidate, buckets)
                }
            }
        }
    }
}

impl ConnectionStrategy for Selection {
    fn expected_links(&self, view: &NodeView<'_>) -> BTreeSet<(PeerId, TopicId)> {
        let mut expected = BTreeSet::new();
        for topic in view.subscriptions {
            // Gate: survivors in the view's sorted, self-excluded order. An
            // absent bucket count skips the filter entirely (≡ B = 1 — the
            // predicate's short-circuit admits everyone), so the survivor
            // list IS the candidate list, in the same order.
            let survivors: Vec<&PeerId> = match self.bucket_count {
                None => view.candidates_for(topic).collect(),
                Some(buckets) => view
                    .candidates_for(topic)
                    .filter(|candidate| {
                        self.gate_admits(view.epoch_nonce, topic, candidate, buckets)
                    })
                    .collect(),
            };
            match self.pick_count {
                // All survivors — the previous connect-to-all/hash-gated
                // behaviours, depending on the gate.
                None => {
                    for candidate in survivors {
                        expected.insert((candidate.clone(), topic.clone()));
                    }
                }
                // Exactly min(pick count, survivors) uniform picks without
                // replacement: index sampling over the ordered survivor list
                // is deterministic in (seed, survivor set) — a pure function
                // of the set, order-independent by construction.
                Some(pick_count) => {
                    if survivors.is_empty() {
                        continue;
                    }
                    let amount = pick_count.min(survivors.len());
                    let mut rng = ChaCha20Rng::from_seed(self.topic_seed(view.epoch_nonce, topic));
                    for index in rand::seq::index::sample(&mut rng, survivors.len(), amount) {
                        expected.insert((survivors[index].clone(), topic.clone()));
                    }
                }
            }
        }
        expected
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::Selection;
    use crate::connection_state::LinkKind;
    use crate::peer::PeerId;
    use crate::strategies::connection::ConnectionStrategy;
    use crate::strategies::edge::{is_valid_edge, is_valid_edge_publisher, is_valid_edge_sym};
    use crate::strategies::test_support::{
        candidates, no_links, peer, subscriptions, topic, view, view_with_nonce,
    };
    use crate::topic::TopicId;

    fn ids(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("c{i:03}")).collect()
    }

    /// The expected set of a `Selection` over a fresh single-topic view of
    /// `n` candidates named `c000..`.
    fn expected_of(selection: &Selection, n: usize) -> BTreeSet<(PeerId, TopicId)> {
        let ids = ids(n);
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &refs)]);
        selection.expected_links(&view(&subs, &cands, no_links()))
    }

    /// The `(candidate, topic)` pairs among `n` generated candidates that
    /// pass `predicate` — the directly computed gate-survivor set the
    /// strategy's output is compared against.
    fn survivors_by(n: usize, predicate: impl Fn(&PeerId) -> bool) -> BTreeSet<(PeerId, TopicId)> {
        ids(n)
            .iter()
            .map(|name| peer(name))
            .filter(predicate)
            .map(|p| (p, topic("t1")))
            .collect()
    }

    // 017-FR-001: exactly min(pick count, survivors) picks, without
    // replacement (the BTreeSet result deduplicates, so its size proves the
    // without-replacement property).
    #[test]
    fn picks_exactly_the_pick_count_when_enough_survivors() {
        let selection = Selection::new(peer("self"), [7u8; 32]).with_pick_count(Some(5));
        assert_eq!(expected_of(&selection, 20).len(), 5);
    }

    // 017-FR-001: min(pick count, survivors) degeneracy — fewer survivors
    // than the pick count means all of them are picked (matches the previous
    // small-topic behaviour's direction; no retry or back-fill).
    #[test]
    fn degenerates_to_all_survivors_when_short() {
        let selection = Selection::new(peer("self"), [7u8; 32]).with_pick_count(Some(8));
        let expected = expected_of(&selection, 3);
        assert_eq!(expected.len(), 3);
    }

    // 017-FR-002: pick count absent selects every gate survivor — ungated,
    // that is every candidate (the connect-to-all point, both knobs absent).
    #[test]
    fn pick_count_absent_selects_every_candidate_when_ungated() {
        let selection = Selection::new(peer("self"), [7u8; 32]);
        assert_eq!(expected_of(&selection, 20).len(), 20);
    }

    // 017-FR-002: pick count 0 selects nothing (the dial-none boundary, M1).
    #[test]
    fn pick_count_zero_selects_nothing() {
        let selection = Selection::new(peer("self"), [7u8; 32]).with_pick_count(Some(0));
        assert!(expected_of(&selection, 20).is_empty());
    }

    // 017-FR-001: bucket count present, pick count absent — the previous
    // hash-gated behaviour: exactly the predicate survivors are selected.
    #[test]
    fn gate_present_pick_absent_selects_the_predicate_survivors() {
        let selection = Selection::new(peer("self"), [7u8; 32]).with_bucket_count(Some(3));
        let expected = expected_of(&selection, 60);
        let survivors = survivors_by(60, |p| is_valid_edge(0, &topic("t1"), &peer("self"), p, 3));
        assert_eq!(expected, survivors);
    }

    // 017-FR-001: gate-then-pick composition — every pick passes the
    // predicate and the pick count binds within the survivor set.
    #[test]
    fn gate_then_pick_composes() {
        let survivors = survivors_by(60, |p| is_valid_edge(0, &topic("t1"), &peer("self"), p, 3));
        let selection = Selection::new(peer("self"), [7u8; 32])
            .with_bucket_count(Some(3))
            .with_pick_count(Some(4));
        let expected = expected_of(&selection, 60);
        assert_eq!(expected.len(), 4.min(survivors.len()));
        assert!(
            expected.is_subset(&survivors),
            "every dialed edge must pass the predicate",
        );
    }

    // The degenerate gated direction: a gate wider than the candidate set
    // leaves fewer survivors than the pick count — all survivors selected.
    #[test]
    fn pick_count_exceeding_gate_survivors_selects_all_survivors() {
        let survivors = survivors_by(60, |p| is_valid_edge(0, &topic("t1"), &peer("self"), p, 40));
        let selection = Selection::new(peer("self"), [7u8; 32])
            .with_bucket_count(Some(40))
            .with_pick_count(Some(5));
        let expected = expected_of(&selection, 60);
        assert!(survivors.len() < 5, "fixture: the gate must bind first");
        assert_eq!(expected, survivors);
    }

    // 017-FR-002 boundary: bucket count 1 (legal in core construction — the
    // sweep config's ungated axis point) selects identically to absent.
    #[test]
    fn bucket_count_one_equals_absent() {
        let gated = Selection::new(peer("self"), [7u8; 32])
            .with_bucket_count(Some(1))
            .with_pick_count(Some(5));
        let ungated = Selection::new(peer("self"), [7u8; 32]).with_pick_count(Some(5));
        assert_eq!(expected_of(&gated, 20), expected_of(&ungated, 20));
    }

    // 005-era invariant preserved: selection is a function of the candidate
    // *set*, not its declaration order.
    #[test]
    fn selection_is_order_independent() {
        let ids = ids(40);
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let mut rev = refs.clone();
        rev.reverse();
        let subs = subscriptions(&["t1"]);
        let selection = Selection::new(peer("self"), [7u8; 32])
            .with_bucket_count(Some(2))
            .with_pick_count(Some(5));
        let one = selection.expected_links(&view(&subs, &candidates(&[("t1", &refs)]), no_links()));
        let two = selection.expected_links(&view(&subs, &candidates(&[("t1", &rev)]), no_links()));
        assert_eq!(one, two, "selection must not depend on iteration order");
    }

    // The heartbeat retry primitive: the same instance and view yield the
    // same expected set on every call (repeated heartbeats within an epoch
    // re-dial the SAME set).
    #[test]
    fn repeated_calls_yield_the_same_set() {
        let selection = Selection::new(peer("self"), [9u8; 32])
            .with_bucket_count(Some(2))
            .with_pick_count(Some(5));
        assert_eq!(expected_of(&selection, 20), expected_of(&selection, 20));
    }

    // Distinct seeds draw distinct pick sets (deterministically checked pair).
    #[test]
    fn distinct_seeds_draw_distinct_sets() {
        let a = Selection::new(peer("self"), [0u8; 32]).with_pick_count(Some(5));
        let b = Selection::new(peer("self"), [1u8; 32]).with_pick_count(Some(5));
        assert_ne!(expected_of(&a, 40), expected_of(&b, 40));
    }

    // The seed feeds only the pick draw: with the pick count absent the
    // expected set is seed-independent (gate-only configurations need no
    // sampling randomness).
    #[test]
    fn seed_is_unread_without_a_pick_count() {
        let a = Selection::new(peer("self"), [0u8; 32]).with_bucket_count(Some(3));
        let b = Selection::new(peer("self"), [1u8; 32]).with_bucket_count(Some(3));
        assert_eq!(expected_of(&a, 60), expected_of(&b, 60));
    }

    // 017-FR-004: the per-kind hash domains are preserved — a publisher
    // instance gates under the publisher domain, an independent draw from
    // the relay instance's over the same view.
    #[test]
    fn publisher_instances_gate_under_the_publisher_domain() {
        let selection = Selection::new(peer("self"), [7u8; 32])
            .for_kind(LinkKind::Publisher)
            .with_bucket_count(Some(3));
        let expected = expected_of(&selection, 60);
        let survivors = survivors_by(60, |p| {
            is_valid_edge_publisher(0, &topic("t1"), &peer("self"), p, 3)
        });
        assert_eq!(expected, survivors);
    }

    // 017-FR-004: symmetric composes with the plane — the gate draws the
    // unordered-pair predicate under the symmetric domain, and picks stay
    // within its survivors.
    #[test]
    fn symmetric_gate_uses_the_unordered_pair_predicate() {
        let survivors = survivors_by(60, |p| {
            is_valid_edge_sym(0, &topic("t1"), &peer("self"), p, 3)
        });
        let gate_only = Selection::new(peer("self"), [7u8; 32])
            .with_symmetric(true)
            .with_bucket_count(Some(3));
        assert_eq!(expected_of(&gate_only, 60), survivors);

        let gated_picks = Selection::new(peer("self"), [7u8; 32])
            .with_symmetric(true)
            .with_bucket_count(Some(3))
            .with_pick_count(Some(4));
        let expected = expected_of(&gated_picks, 60);
        assert_eq!(expected.len(), 4.min(survivors.len()));
        assert!(expected.is_subset(&survivors));
    }

    // ADR 0031: the epoch nonce is read from the view for the gate — some
    // nonce among many must gate differently from nonce 0.
    #[test]
    fn gate_varies_by_epoch_nonce() {
        let ids = ids(60);
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &refs)]);
        let selection = Selection::new(peer("self"), [7u8; 32]).with_bucket_count(Some(3));
        let at_zero = selection.expected_links(&view(&subs, &cands, no_links()));
        let diverges = (1..=16u64).any(|n| {
            selection.expected_links(&view_with_nonce(&subs, &cands, no_links(), n)) != at_zero
        });
        assert!(diverges, "the epoch nonce must vary the gate");
    }
}

/// The derivation value pin (017-T004 lineage). The commit-A fixtures —
/// verified value-for-value against the experiments `UniformSampler` before
/// its deletion, and byte-identity-proven against the recorded baselines at
/// the 017-T017 gate — were superseded **by design** at the commit-B
/// derivation swap (017-T024, 017-FR-026): the values below pin the
/// commit-B derivation (per-seam domain, self-identity, epoch nonce,
/// length-prefixed preimage) as independent literals, guarding against
/// silent drift; the layout itself is reconstructed end to end in
/// `seed_properties::draw_preimage_layout_is_pinned`.
#[cfg(test)]
mod derivation_pin {
    use std::collections::BTreeSet;

    use super::Selection;
    use crate::peer::PeerId;
    use crate::strategies::connection::ConnectionStrategy;
    use crate::strategies::test_support::{candidates, no_links, peer, subscriptions, view};
    use crate::topic::TopicId;

    fn picks(seed: [u8; 32], pick_count: usize, n: usize) -> BTreeSet<(PeerId, TopicId)> {
        let ids: Vec<String> = (0..n).map(|i| format!("c{i:03}")).collect();
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let subs = subscriptions(&["t0"]);
        let cands = candidates(&[("t0", &refs)]);
        let v = view(&subs, &cands, no_links());
        Selection::new(peer("pin"), seed)
            .with_pick_count(Some(pick_count))
            .expected_links(&v)
    }

    fn expected(names: &[&str]) -> BTreeSet<(PeerId, TopicId)> {
        names
            .iter()
            .map(|n| (peer(n), "t0".parse::<TopicId>().expect("valid topic id")))
            .collect()
    }

    // Commit-B values: unlike the commit-A pin, these are specific to the
    // instance identity ("pin"), the relay draw domain, and epoch nonce 0 —
    // all now preimage components.
    #[test]
    fn pick_sets_match_the_commit_b_derivation() {
        assert_eq!(
            picks([7u8; 32], 5, 20),
            expected(&["c007", "c011", "c013", "c014", "c018"]),
        );
        assert_eq!(picks([9u8; 32], 3, 8), expected(&["c001", "c005", "c006"]));
    }
}

/// 017-T023 — the seed-property battery for the commit-B draw derivation
/// (017-FR-015; spec US4 scenarios): the per-topic draw is a pure function
/// of (the seam's domain, seed, self-identity key bytes, epoch nonce,
/// topic).
#[cfg(test)]
mod seed_properties {
    use std::collections::BTreeSet;

    use rand::SeedableRng;
    use rand_chacha::ChaCha20Rng;
    use sha2::{Digest, Sha256};

    use super::Selection;
    use crate::connection_state::LinkKind;
    use crate::message::push_len_prefixed;
    use crate::peer::PeerId;
    use crate::strategies::connection::ConnectionStrategy;
    use crate::strategies::test_support::{
        candidates, no_links, peer, subscriptions, view_with_nonce,
    };
    use crate::topic::TopicId;

    /// Picks of `selection` over a fresh 20-candidate single-topic view at
    /// `nonce`.
    fn picks_at(selection: &Selection, nonce: u64) -> BTreeSet<(PeerId, TopicId)> {
        let ids: Vec<String> = (0..20).map(|i| format!("c{i:03}")).collect();
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let subs = subscriptions(&["t0"]);
        let cands = candidates(&[("t0", &refs)]);
        selection.expected_links(&view_with_nonce(&subs, &cands, no_links(), nonce))
    }

    // Purity: the same (seed, self-identity, nonce, view) reproduces the
    // identical picks on every call.
    #[test]
    fn identical_inputs_reproduce_identical_picks() {
        let selection = Selection::new(peer("node"), [7u8; 32]).with_pick_count(Some(3));
        assert_eq!(picks_at(&selection, 5), picks_at(&selection, 5));
    }

    // US4 scenario 2: two nodes sharing one seed value draw independently —
    // self-identity is mixed into the draw preimage, so a fleet-shared seed
    // never yields correlated topologies.
    #[test]
    fn fleet_shared_seed_draws_per_node_independent_picks() {
        let one = Selection::new(peer("one"), [7u8; 32]).with_pick_count(Some(3));
        let two = Selection::new(peer("two"), [7u8; 32]).with_pick_count(Some(3));
        assert_ne!(
            picks_at(&one, 0),
            picks_at(&two, 0),
            "self-identity must decorrelate a fleet-shared seed",
        );
    }

    // Analysis I2: one node's relay and publisher instances draw under
    // separate per-seam domains — the sampling analogue of edge.rs's
    // publisher_domain_is_an_independent_draw — so an M3/M5 node's publisher
    // targets are uncorrelated with its relay upstreams even over one shared
    // seed, ungated seams, and equal pick counts.
    #[test]
    fn relay_and_publisher_instances_draw_independently() {
        let relay = Selection::new(peer("node"), [7u8; 32]).with_pick_count(Some(3));
        let publisher = Selection::new(peer("node"), [7u8; 32])
            .for_kind(LinkKind::Publisher)
            .with_pick_count(Some(3));
        assert_ne!(
            picks_at(&relay, 0),
            picks_at(&publisher, 0),
            "the two seam instances must not derive the same RNG stream",
        );
    }

    // US4 scenario 3: picks are stable across repeated dial ticks within an
    // epoch and re-drawn when the epoch nonce changes — exactly as the gate's
    // edges re-shuffle.
    #[test]
    fn nonce_change_redraws_and_heartbeats_stay_stable() {
        let selection = Selection::new(peer("node"), [9u8; 32]).with_pick_count(Some(3));
        assert_eq!(
            picks_at(&selection, 0),
            picks_at(&selection, 0),
            "a repeated heartbeat re-dials the same set",
        );
        assert_ne!(
            picks_at(&selection, 0),
            picks_at(&selection, 1),
            "an epoch-nonce change must re-randomise the picks",
        );
    }

    // The pinned preimage layout (017-FR-015; research R2): the per-topic
    // draw seed is SHA-256( lp(per-seam domain) ‖ lp(seed) ‖ lp(self-id key
    // bytes) ‖ nonce_le8 ‖ lp(topic) ) with lp the crate's one length-prefix
    // primitive, and the draw is index sampling over the sorted survivor
    // list from a ChaCha20 stream keyed by it. Reconstructed here
    // independently, end to end.
    #[test]
    fn draw_preimage_layout_is_pinned() {
        let self_id = peer("pin");
        let seed = [7u8; 32];
        let nonce = 5u64;
        let ids: Vec<String> = (0..20).map(|i| format!("c{i:03}")).collect();

        let mut preimage = Vec::new();
        push_len_prefixed(&mut preimage, b"pubsub/uniform-selection/relay/v1");
        push_len_prefixed(&mut preimage, &seed);
        push_len_prefixed(&mut preimage, self_id.as_public_key().as_bytes());
        preimage.extend_from_slice(&nonce.to_le_bytes());
        push_len_prefixed(&mut preimage, b"t0");
        let topic_seed: [u8; 32] = Sha256::digest(&preimage).into();

        let mut rng = ChaCha20Rng::from_seed(topic_seed);
        let expected: BTreeSet<(PeerId, TopicId)> = rand::seq::index::sample(&mut rng, 20, 3)
            .into_iter()
            .map(|index| {
                (
                    peer(&ids[index]),
                    "t0".parse::<TopicId>().expect("valid topic id"),
                )
            })
            .collect();

        let selection = Selection::new(self_id, seed).with_pick_count(Some(3));
        assert_eq!(picks_at(&selection, nonce), expected);
    }
}

//! The seeded, bounded connection-selection policy: [`SeededBoundedSelection`]
//! (feature 005, ADR 0024).

use std::collections::{HashMap, HashSet};

use super::ConnectionStrategy;
use crate::peer::PeerId;
use crate::topic::TopicId;

/// The seeded, bounded connection-selection policy (feature 005, ADR 0024).
///
/// For each joined topic, ranks the candidate peers by a stable keyed hash of
/// `(seed, self_id, topic, candidate_id)` and keeps the lowest `upstream_degree`,
/// breaking ties on `candidate_id`. Selection is a pure function of its inputs:
/// the seed and identity are fixed fields, no randomness is drawn at decision
/// time, and the result is independent of candidate-set iteration order — so the
/// topology is reproducible from the single network seed (the node folds its own
/// `self_id` in, giving per-node diversity). The node hands in the **viable**
/// candidate view (candidates minus peers a dial already failed with), so
/// back-fill of the next-ranked candidate falls out of recomputation without any
/// extra input to this policy.
pub struct SeededBoundedSelection {
    seed: u64,
    self_id: PeerId,
    upstream_degree: usize,
}

impl SeededBoundedSelection {
    /// Build the policy for one node from already-parsed inputs.
    #[must_use]
    pub fn new(seed: u64, self_id: PeerId, upstream_degree: usize) -> Self {
        Self {
            seed,
            self_id,
            upstream_degree,
        }
    }
}

/// The stable ranking key for one candidate: SHA-256 over a canonical,
/// length-prefixed encoding of `(domain, seed, self_id, topic, candidate)`.
/// SHA-256 (the in-tree digest) is chosen over `DefaultHasher` for
/// cross-platform stability — selection must reproduce identically on any
/// machine (ADR 0024).
fn rank_key(seed: u64, self_id: &PeerId, topic: &TopicId, candidate: &PeerId) -> [u8; 32] {
    use sha2::{Digest, Sha256};

    // Length-prefix each variable-width component so distinct tuples cannot
    // collide via concatenation. usize lengths are widened to a fixed u64.
    #[allow(clippy::cast_possible_truncation)]
    fn feed(hasher: &mut Sha256, bytes: &[u8]) {
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }

    let mut hasher = Sha256::new();
    // Domain-separate by the strategy's own unique byte-string tag, so distinct
    // strategies never share a hash domain (ADR 0024).
    feed(
        &mut hasher,
        super::ConnectionStrategyKind::SeededBounded.tag(),
    );
    hasher.update(seed.to_le_bytes());
    feed(&mut hasher, self_id.to_string().as_bytes());
    feed(&mut hasher, topic.to_string().as_bytes());
    feed(&mut hasher, candidate.to_string().as_bytes());
    hasher.finalize().into()
}

impl ConnectionStrategy for SeededBoundedSelection {
    fn expected_upstream(
        &self,
        subscriptions: &HashSet<TopicId>,
        candidates: &HashMap<TopicId, HashSet<PeerId>>,
    ) -> HashSet<(PeerId, TopicId)> {
        let mut expected = HashSet::new();
        for topic in subscriptions {
            let Some(peers) = candidates.get(topic) else {
                continue;
            };
            // Rank by (hash key, candidate id) so the order is total and
            // independent of set iteration order; keep the lowest `upstream_degree`.
            let mut ranked: Vec<&PeerId> = peers.iter().collect();
            ranked.sort_by(|a, b| {
                rank_key(self.seed, &self.self_id, topic, a)
                    .cmp(&rank_key(self.seed, &self.self_id, topic, b))
                    .then_with(|| a.to_string().cmp(&b.to_string()))
            });
            for peer in ranked.into_iter().take(self.upstream_degree) {
                expected.insert((peer.clone(), topic.clone()));
            }
        }
        expected
    }
}

#[cfg(test)]
mod tests {
    use super::SeededBoundedSelection;
    use crate::peer::PeerId;
    use crate::strategies::connection::ConnectionStrategy;
    use crate::topic::TopicId;
    use std::collections::{HashMap, HashSet};
    use std::str::FromStr;

    fn peer(s: &str) -> PeerId {
        PeerId::from_str(s).expect("valid peer id")
    }

    fn topic(s: &str) -> TopicId {
        TopicId::from_str(s).expect("valid topic id")
    }

    fn subscriptions(topics: &[&str]) -> HashSet<TopicId> {
        topics.iter().map(|t| topic(t)).collect()
    }

    fn candidates(entries: &[(&str, &[&str])]) -> HashMap<TopicId, HashSet<PeerId>> {
        entries
            .iter()
            .map(|(t, peers)| (topic(t), peers.iter().map(|p| peer(p)).collect()))
            .collect()
    }

    // FR-001: more candidates than the bound ⇒ exactly `upstream_degree` selected.
    #[test]
    fn bounded_selects_exactly_upstream_degree() {
        let policy = SeededBoundedSelection::new(7, peer("self"), 2);
        let expected = policy.expected_upstream(
            &subscriptions(&["t1"]),
            &candidates(&[("t1", &["a", "b", "c", "d"])]),
        );
        assert_eq!(
            expected.len(),
            2,
            "the upstream degree bound is the upper limit"
        );
        for (p, t) in &expected {
            assert_eq!(t, &topic("t1"));
            assert!(["a", "b", "c", "d"].contains(&p.to_string().as_str()));
        }
    }

    // FR-002: candidates at or below the bound ⇒ all selected (bound is a ceiling).
    #[test]
    fn bounded_selects_all_when_at_or_below_bound() {
        let policy = SeededBoundedSelection::new(7, peer("self"), 5);
        let expected =
            policy.expected_upstream(&subscriptions(&["t1"]), &candidates(&[("t1", &["a", "b"])]));
        assert_eq!(
            expected,
            HashSet::from([(peer("a"), topic("t1")), (peer("b"), topic("t1"))]),
        );
    }

    // FR-003: identical (seed, self_id, topic, candidates) ⇒ identical selection,
    // independent of candidate-set construction/iteration order.
    #[test]
    fn bounded_selection_is_deterministic_and_order_independent() {
        let policy = SeededBoundedSelection::new(42, peer("self"), 3);
        let one = policy.expected_upstream(
            &subscriptions(&["t1"]),
            &candidates(&[("t1", &["a", "b", "c", "d", "e"])]),
        );
        let two = policy.expected_upstream(
            &subscriptions(&["t1"]),
            &candidates(&[("t1", &["e", "d", "c", "b", "a"])]),
        );
        assert_eq!(one, two, "selection must not depend on iteration order");
        assert_eq!(one.len(), 3);
    }

    // FR-004: the default seed (0) yields a deterministic, repeatable selection.
    #[test]
    fn default_seed_zero_is_deterministic() {
        let cands = candidates(&[("t1", &["a", "b", "c", "d"])]);
        let first = SeededBoundedSelection::new(0, peer("self"), 2)
            .expected_upstream(&subscriptions(&["t1"]), &cands);
        let again = SeededBoundedSelection::new(0, peer("self"), 2)
            .expected_upstream(&subscriptions(&["t1"]), &cands);
        assert_eq!(first, again, "seed 0 must reproduce identically");
        assert_eq!(first.len(), 2);
    }

    // FR-005: the node's own id is folded into the ranking, so two nodes with the
    // same seed and candidate set can select different subsets.
    #[test]
    fn bounded_selection_varies_by_self_id() {
        let cands = candidates(&[("t1", &["a", "b", "c", "d", "e", "f"])]);
        let by_x = SeededBoundedSelection::new(1, peer("x"), 2)
            .expected_upstream(&subscriptions(&["t1"]), &cands);
        let by_y = SeededBoundedSelection::new(1, peer("y"), 2)
            .expected_upstream(&subscriptions(&["t1"]), &cands);
        assert_ne!(
            by_x, by_y,
            "per-node derivation should diverge for distinct self ids on this set",
        );
    }

    // A candidate on an unjoined topic is never selected (membership-scoped).
    #[test]
    fn bounded_ignores_unjoined_topics() {
        let policy = SeededBoundedSelection::new(7, peer("self"), 3);
        let expected = policy.expected_upstream(
            &subscriptions(&["t1"]),
            &candidates(&[("t1", &["a"]), ("t2", &["b", "c"])]),
        );
        assert_eq!(expected, HashSet::from([(peer("a"), topic("t1"))]));
    }

    // SC-003 / US3: distinct seeds explore distinct selections (the topology
    // varies with the seed). Over a handful of seeds on a set larger than the
    // bound, more than one distinct selection appears.
    #[test]
    fn distinct_seeds_produce_distinct_selections() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["c0", "c1", "c2", "c3", "c4", "c5"])]);
        let mut distinct = HashSet::new();
        for seed in 0..16u64 {
            let sel =
                SeededBoundedSelection::new(seed, peer("self"), 3).expected_upstream(&subs, &cands);
            let mut ids: Vec<String> = sel.iter().map(|(p, _)| p.to_string()).collect();
            ids.sort();
            distinct.insert(ids.join(","));
        }
        assert!(
            distinct.len() > 1,
            "distinct seeds should explore more than one selection (saw {})",
            distinct.len(),
        );
    }

    // FR-007 / SC-004 / US3: over a fixed sweep of seeds, selection is unbiased
    // with respect to candidate identity. A chi-square goodness-of-fit against
    // the uniform expectation (10 candidates, choose 3) must not reject at
    // p < 0.001 (df = 9 ⇒ critical value 27.88). The sweep is a fixed seed range,
    // so the test is reproducible. (T022: the SHA-256 ranking already satisfies
    // this — no ranking-key adjustment needed.)
    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn selection_is_unbiased_over_a_seed_sweep() {
        let ids = ["c0", "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8", "c9"];
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &ids)]);
        let upstream_degree = 3usize;
        let sweeps = 2000u64;

        let mut counts: HashMap<String, u64> = HashMap::new();
        for seed in 0..sweeps {
            let sel = SeededBoundedSelection::new(seed, peer("self"), upstream_degree)
                .expected_upstream(&subs, &cands);
            assert_eq!(sel.len(), upstream_degree);
            for (p, _) in sel {
                *counts.entry(p.to_string()).or_insert(0) += 1;
            }
        }

        let expected = (sweeps as f64) * (upstream_degree as f64) / (ids.len() as f64);
        let chi2: f64 = ids
            .iter()
            .map(|id| {
                let observed = *counts.get(*id).unwrap_or(&0) as f64;
                (observed - expected).powi(2) / expected
            })
            .sum();
        assert!(
            chi2 < 27.88,
            "selection biased: chi^2 = {chi2:.2} exceeds the p<0.001 critical value (df=9)",
        );
    }
}

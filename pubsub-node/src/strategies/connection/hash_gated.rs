//! The verifiable hash-gated connection-selection policy: [`HashGatedConnection`]
//! (bucketed-pull, ADR 0024).

use std::collections::{BTreeMap, BTreeSet};

use super::ConnectionStrategy;
use crate::peer::PeerId;
use crate::strategies::edge::{bucket_count, is_valid_edge};
use crate::topic::TopicId;

/// The verifiable, bucketed connection-selection policy (ADR 0024).
///
/// For each joined topic `T` at the current `interval`, dial candidate `U` iff
/// the shared edge predicate `H(genesis, T, self, U, interval) mod B == 0` holds,
/// where `B = max(1, round(|candidates_T| / rf))`. Expected out-degree per topic
/// ≈ `rf`; a topic with `≤ ~rf` candidates has `B = 1` and connects to **all** of
/// them (small-topic fallback). Selection is pure and reproducible: `genesis` and
/// `rf` are fixed fields, the interval is an input, the hash and modulus are
/// fixed, and the result is a function of the *set* (order-independent). The
/// acceptor recomputes the same predicate to **verify** the request (ADR 0025).
pub struct HashGatedConnection {
    genesis: u64,
    self_id: PeerId,
    rf: usize,
}

impl HashGatedConnection {
    /// Build the policy for one node from already-parsed inputs.
    #[must_use]
    pub fn new(genesis: u64, self_id: PeerId, rf: usize) -> Self {
        Self {
            genesis,
            self_id,
            rf,
        }
    }
}

impl ConnectionStrategy for HashGatedConnection {
    fn expected_upstream(
        &self,
        subscriptions: &BTreeSet<TopicId>,
        candidates: &BTreeMap<TopicId, BTreeSet<PeerId>>,
        interval: u64,
    ) -> BTreeSet<(PeerId, TopicId)> {
        let mut expected = BTreeSet::new();
        for topic in subscriptions {
            let Some(peers) = candidates.get(topic) else {
                continue;
            };
            let buckets = bucket_count(peers.len(), self.rf);
            for candidate in peers {
                if is_valid_edge(
                    self.genesis,
                    topic,
                    &self.self_id,
                    candidate,
                    interval,
                    buckets,
                ) {
                    expected.insert((candidate.clone(), topic.clone()));
                }
            }
        }
        expected
    }
}

#[cfg(test)]
mod tests {
    use super::HashGatedConnection;
    use crate::peer::PeerId;
    use crate::strategies::connection::ConnectionStrategy;
    use crate::topic::TopicId;
    use std::collections::{BTreeMap, BTreeSet};
    use std::str::FromStr;

    fn peer(s: &str) -> PeerId {
        PeerId::from_str(s).expect("valid peer id")
    }
    fn topic(s: &str) -> TopicId {
        TopicId::from_str(s).expect("valid topic id")
    }
    fn subscriptions(topics: &[&str]) -> BTreeSet<TopicId> {
        topics.iter().map(|t| topic(t)).collect()
    }
    fn candidates(entries: &[(&str, &[&str])]) -> BTreeMap<TopicId, BTreeSet<PeerId>> {
        entries
            .iter()
            .map(|(t, peers)| (topic(t), peers.iter().map(|p| peer(p)).collect()))
            .collect()
    }
    fn ids(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("c{i:03}")).collect()
    }

    // FR-001 small-topic (≤ rf candidates ⇒ B=1 ⇒ connect-to-all).
    #[test]
    fn small_topic_connects_to_all() {
        let policy = HashGatedConnection::new(7, peer("self"), 8);
        let expected = policy.expected_upstream(
            &subscriptions(&["t1"]),
            &candidates(&[("t1", &["a", "b", "c"])]),
            0,
        );
        assert_eq!(
            expected,
            BTreeSet::from([
                (peer("a"), topic("t1")),
                (peer("b"), topic("t1")),
                (peer("c"), topic("t1")),
            ]),
            "with ≤ rf candidates B=1, so every candidate is a valid edge",
        );
    }

    // FR-002/SC-001: identical inputs ⇒ identical selection, order-independent.
    #[test]
    fn selection_is_deterministic_and_order_independent() {
        let ids = ids(80);
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let policy = HashGatedConnection::new(42, peer("self"), 8);
        let one =
            policy.expected_upstream(&subscriptions(&["t1"]), &candidates(&[("t1", &refs)]), 0);
        let mut rev = refs.clone();
        rev.reverse();
        let two =
            policy.expected_upstream(&subscriptions(&["t1"]), &candidates(&[("t1", &rev)]), 0);
        assert_eq!(one, two, "selection must not depend on iteration order");
    }

    // FR-003/SC-004: expected out-degree tracks rf on a large candidate set.
    #[test]
    fn out_degree_tracks_rf() {
        let ids = ids(80);
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let rf = 8usize;
        let expected = HashGatedConnection::new(1, peer("self"), rf).expected_upstream(
            &subscriptions(&["t1"]),
            &candidates(&[("t1", &refs)]),
            0,
        );
        // 80 candidates, B = round(80/8) = 10 ⇒ expected ≈ 8. Lenient bound.
        assert!(
            (3..=18).contains(&expected.len()),
            "degree {} should be near rf={rf}",
            expected.len(),
        );
    }

    // FR-005: folding self_id in ⇒ two nodes on the same set select differently.
    #[test]
    fn selection_varies_by_self_id() {
        let ids = ids(60);
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let cands = candidates(&[("t1", &refs)]);
        let by_x = HashGatedConnection::new(1, peer("x"), 8).expected_upstream(
            &subscriptions(&["t1"]),
            &cands,
            0,
        );
        let by_y = HashGatedConnection::new(1, peer("y"), 8).expected_upstream(
            &subscriptions(&["t1"]),
            &cands,
            0,
        );
        assert_ne!(by_x, by_y, "per-node derivation should diverge");
    }

    // A candidate on an unjoined topic is never selected (membership-scoped).
    #[test]
    fn ignores_unjoined_topics() {
        let policy = HashGatedConnection::new(7, peer("self"), 8);
        let expected = policy.expected_upstream(
            &subscriptions(&["t1"]),
            &candidates(&[("t1", &["a"]), ("t2", &["b", "c"])]),
            0,
        );
        assert_eq!(expected, BTreeSet::from([(peer("a"), topic("t1"))]));
    }

    // FR-004: the default genesis (0) yields a deterministic, repeatable selection.
    #[test]
    fn default_genesis_zero_is_deterministic() {
        let ids = ids(40);
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let cands = candidates(&[("t1", &refs)]);
        let first = HashGatedConnection::new(0, peer("self"), 8).expected_upstream(
            &subscriptions(&["t1"]),
            &cands,
            0,
        );
        let again = HashGatedConnection::new(0, peer("self"), 8).expected_upstream(
            &subscriptions(&["t1"]),
            &cands,
            0,
        );
        assert_eq!(first, again, "genesis 0 must reproduce identically");
    }
}

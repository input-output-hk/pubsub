//! The verifiable hash-gated connection-selection policy: [`HashGatedConnection`]
//! (bucketed-pull, ADR 0024).

use std::collections::BTreeSet;

use super::ConnectionStrategy;
use crate::connection_state::LinkKind;
use crate::peer::PeerId;
use crate::strategies::edge::{
    is_valid_edge, is_valid_edge_publisher, is_valid_edge_sym, resolve_buckets,
};
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

/// The verifiable, bucketed connection-selection policy (ADR 0024/0031).
///
/// For each joined topic `T` under the current epoch nonce, dial candidate `U`
/// iff the shared edge predicate `H(nonce, T, self, U) mod B == 0` holds, where
/// `B = max(1, round(|candidates_T| / target_degree))`. Expected out-degree
/// per topic ≈ `target_degree`; a topic with `≤ ~target_degree` candidates has
/// `B = 1` and connects to **all** of them (small-topic fallback). Selection is
/// pure and reproducible: `target_degree` is a fixed field, the epoch nonce
/// comes from the [`NodeView`] (v1: the configured genesis), the hash and
/// modulus are fixed, and the result is a function of the *set*
/// (order-independent). The acceptor recomputes the same predicate to **verify**
/// the request (ADR 0025).
///
/// **B-agreement assumption.** Verifiability requires the dialer and acceptor to
/// compute the *same* `B`. Deriving it locally from `|candidates_T|` holds only
/// while both ends see the same candidate set — true in v1 (full candidate set,
/// dials happen after `Synced`) but not guaranteed under registry-fold lag or a
/// future discovery layer (the `H_v` caveat in `strategies::edge`). A
/// pinned [`bucket_override`](Self::with_bucket_override) sidesteps this: both
/// ends use the same configured `B`, so verification holds by construction.
pub struct HashGatedConnection {
    self_id: PeerId,
    target_degree: usize,
    bucket_override: Option<usize>,
    kind: LinkKind,
    symmetric: bool,
}

impl HashGatedConnection {
    /// Build the policy for one node from already-parsed inputs. `B` is derived
    /// per topic from `target_degree`; use [`with_bucket_override`](Self::with_bucket_override)
    /// to pin it instead.
    #[must_use]
    pub fn new(self_id: PeerId, target_degree: usize) -> Self {
        Self {
            self_id,
            target_degree,
            bucket_override: None,
            kind: LinkKind::Relay,
            symmetric: false,
        }
    }

    /// Switch the instance to the **symmetric** edge predicate (M4): picks are
    /// drawn for the unordered pair under the dedicated symmetric domain, so
    /// both ends of a valid edge dial each other and the link materialises as
    /// a reciprocal pair — bidirectionality is emergent, never stored. Applies
    /// to relay instances; the publisher seam stays directional (no published
    /// model uses symmetric publisher links) and never sets this.
    #[must_use]
    pub fn with_symmetric(mut self, symmetric: bool) -> Self {
        self.symmetric = symmetric;
        self
    }

    /// Re-target the instance at a link kind (`Relay` is the constructor
    /// default): the kind selects the hash domain the picks are drawn from —
    /// the publisher instance's picks are an independent draw from the relay
    /// instance's over the same view.
    #[must_use]
    pub fn for_kind(mut self, kind: LinkKind) -> Self {
        self.kind = kind;
        self
    }

    /// Pin the bucket count `B` for every topic instead of deriving it from the
    /// local candidate count (`--bucket-count`). When both seams pin the same
    /// value the edge predicate is verifiable by construction, independent of
    /// local fold state (see the B-agreement note on the type). A `None` restores
    /// the derived behaviour. Validated `≥ 1` at build time.
    ///
    /// A pinned `B` replaces the derived value **including the small-topic
    /// `B = 1` connect-to-all floor**: an override larger than a topic's
    /// candidate count can select zero upstreams on that topic (no retry).
    #[must_use]
    pub fn with_bucket_override(mut self, bucket_override: Option<usize>) -> Self {
        self.bucket_override = bucket_override;
        self
    }
}

impl ConnectionStrategy for HashGatedConnection {
    fn expected_links(&self, view: &NodeView<'_>) -> BTreeSet<(PeerId, TopicId)> {
        let mut expected = BTreeSet::new();
        for topic in view.subscriptions {
            // Derive B from the local candidate count (self-excluded, via the
            // view's read seam), unless pinned. The derived value is only
            // verifiable while the acceptor sees the same count (the
            // B-agreement assumption on the type); a pinned override removes
            // that dependence.
            let buckets = resolve_buckets(
                self.bucket_override,
                view.candidates_len(topic),
                self.target_degree,
            );
            for candidate in view.candidates_for(topic) {
                let valid = if self.symmetric {
                    is_valid_edge_sym(view.epoch_nonce, topic, &self.self_id, candidate, buckets)
                } else {
                    match self.kind {
                        LinkKind::Relay => is_valid_edge(
                            view.epoch_nonce,
                            topic,
                            &self.self_id,
                            candidate,
                            buckets,
                        ),
                        LinkKind::Publisher => is_valid_edge_publisher(
                            view.epoch_nonce,
                            topic,
                            &self.self_id,
                            candidate,
                            buckets,
                        ),
                    }
                };
                if valid {
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
    use crate::strategies::connection::ConnectionStrategy;
    use crate::strategies::test_support::{
        candidates, peer, subscriptions, topic, view, view_with_nonce,
    };
    use std::collections::{BTreeMap, BTreeSet};

    fn ids(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("c{i:03}")).collect()
    }

    // 005 FR-001 small-topic (≤ target_degree candidates ⇒ B=1 ⇒ connect-to-all).
    #[test]
    fn small_topic_connects_to_all() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a", "b", "c"])]);
        let down = BTreeMap::new();
        let expected =
            HashGatedConnection::new(peer("self"), 8).expected_links(&view(&subs, &cands, &down));
        assert_eq!(
            expected,
            BTreeSet::from([
                (peer("a"), topic("t1")),
                (peer("b"), topic("t1")),
                (peer("c"), topic("t1")),
            ]),
            "with ≤ target_degree candidates B=1, so every candidate is a valid edge",
        );
    }

    // 005 FR-002/SC-001: identical inputs ⇒ identical selection, order-independent.
    #[test]
    fn selection_is_deterministic_and_order_independent() {
        let ids = ids(80);
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let mut rev = refs.clone();
        rev.reverse();
        let subs = subscriptions(&["t1"]);
        let down = BTreeMap::new();
        let policy = HashGatedConnection::new(peer("self"), 8);
        let one = policy.expected_links(&view(&subs, &candidates(&[("t1", &refs)]), &down));
        let two = policy.expected_links(&view(&subs, &candidates(&[("t1", &rev)]), &down));
        assert_eq!(one, two, "selection must not depend on iteration order");
    }

    // 005 FR-003/SC-004: expected out-degree tracks target_degree on a large set.
    #[test]
    fn out_degree_tracks_target_degree() {
        let ids = ids(80);
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &refs)]);
        let down = BTreeMap::new();
        let expected =
            HashGatedConnection::new(peer("self"), 8).expected_links(&view(&subs, &cands, &down));
        // 80 candidates, B = round(80/8) = 10 ⇒ expected ≈ 8. Lenient bound.
        assert!(
            (3..=18).contains(&expected.len()),
            "degree {} should be near target_degree=8",
            expected.len(),
        );
    }

    // 005 FR-005: folding self_id in ⇒ two nodes on the same set select differently.
    #[test]
    fn selection_varies_by_self_id() {
        let ids = ids(60);
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &refs)]);
        let down = BTreeMap::new();
        let by_x =
            HashGatedConnection::new(peer("x"), 8).expected_links(&view(&subs, &cands, &down));
        let by_y =
            HashGatedConnection::new(peer("y"), 8).expected_links(&view(&subs, &cands, &down));
        assert_ne!(by_x, by_y, "per-node derivation should diverge");
    }

    // A candidate on an unjoined topic is never selected (membership-scoped).
    #[test]
    fn ignores_unjoined_topics() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a"]), ("t2", &["b", "c"])]);
        let down = BTreeMap::new();
        let expected =
            HashGatedConnection::new(peer("self"), 8).expected_links(&view(&subs, &cands, &down));
        assert_eq!(expected, BTreeSet::from([(peer("a"), topic("t1"))]));
    }

    // A pinned bucket override replaces the derived B: with B=1 every candidate
    // is a valid edge (connect-to-all), independent of the candidate count.
    #[test]
    fn bucket_override_pins_the_bucket_count() {
        let ids = ids(80);
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &refs)]);
        let down = BTreeMap::new();
        // Derived B on 80 candidates ⇒ ~8 selected; pinned B=1 ⇒ all 80.
        let pinned = HashGatedConnection::new(peer("self"), 8)
            .with_bucket_override(Some(1))
            .expected_links(&view(&subs, &cands, &down));
        assert_eq!(pinned.len(), 80, "B=1 connects to every candidate");
    }

    // 005 FR-004: the default epoch nonce (0) yields a deterministic, repeatable
    // selection.
    #[test]
    fn default_nonce_zero_is_deterministic() {
        let ids = ids(40);
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &refs)]);
        let down = BTreeMap::new();
        let first =
            HashGatedConnection::new(peer("self"), 8).expected_links(&view(&subs, &cands, &down));
        let again =
            HashGatedConnection::new(peer("self"), 8).expected_links(&view(&subs, &cands, &down));
        assert_eq!(first, again, "nonce 0 must reproduce identically");
    }

    // ADR 0031: the epoch nonce is read from the view — some nonce among many
    // must select differently from nonce 0, else the nonce is not being hashed.
    #[test]
    fn selection_varies_by_epoch_nonce() {
        let ids = ids(60);
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &refs)]);
        let down = BTreeMap::new();
        let policy = HashGatedConnection::new(peer("self"), 8);
        let at_zero = policy.expected_links(&view(&subs, &cands, &down));
        let diverges = (1..=16u64)
            .any(|n| policy.expected_links(&view_with_nonce(&subs, &cands, &down, n)) != at_zero);
        assert!(diverges, "the epoch nonce must vary the selection");
    }
}

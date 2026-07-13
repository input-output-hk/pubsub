//! The verifiable hash-gated selection policy: [`HashGatedSelection`]
//! (bucketed-pull, ADR 0024/0033/0034).

use std::collections::BTreeSet;

use super::LinkSelectionStrategy;
use crate::connection_state::LinkRole;
use crate::peer::PeerId;
use crate::strategies::edge::{is_valid_edge_for, is_valid_edge_sym, resolve_buckets};
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

/// The verifiable, bucketed selection policy, instantiated per role slot
/// (ADR 0034 — the union of the former `HashGatedConnection` and
/// `HashGatedPublish`).
///
/// For each joined topic `T` under the current epoch nonce, select candidate
/// `U` iff the **role's** edge predicate holds — the relay domain for a relay
/// slot (`H(nonce, T, self, U) mod B == 0`, ADR 0024), the publish domain for
/// a publish slot (an independent hash draw, ADR 0033). `B` derives per topic
/// from the slot's `degree` (`relay_degree` / `publish_degree`), so expected
/// out-degree per topic ≈ `degree`; a topic with `≤ ~degree` candidates has
/// `B = 1` and selects **all** of them (small-topic fallback). Selection is
/// pure and reproducible from the view's epoch nonce; the acceptor recomputes
/// the same predicate to **verify** the request (ADR 0025).
///
/// On the publish slot the selected links are the node's **standing initiation
/// links** — always established, unconditionally, per the M3 model
/// (`formal_spec/hybrid_dissemination/models/m3/README.md`: "each node opens
/// s−1 standing initiation links"; `publish_degree` ≈ the model's `s−1`).
///
/// **B-agreement assumption** (unchanged from 005): deriving `B` locally from
/// the candidate count matches the dialer's only while both ends see the same
/// set; a pinned [`bucket_override`](Self::with_bucket_override) removes the
/// dependence.
pub struct HashGatedSelection {
    role: LinkRole,
    self_id: PeerId,
    degree: usize,
    bucket_override: Option<usize>,
    symmetric: bool,
}

impl HashGatedSelection {
    /// Build the policy for one node's role slot from already-parsed inputs.
    /// `degree` is the slot's target out-degree (`relay_degree` /
    /// `publish_degree`); `B` is derived per topic from it — use
    /// [`with_bucket_override`](Self::with_bucket_override) to pin it.
    #[must_use]
    pub fn new(role: LinkRole, self_id: PeerId, degree: usize) -> Self {
        Self {
            role,
            self_id,
            degree,
            bucket_override: None,
            symmetric: false,
        }
    }

    /// Select under the **symmetric** edge predicate (ADR 0035 — the M4
    /// bidirectional mode): both ends compute the same expected edge set, so
    /// each dials the other and every link materialises as the Out+In pair.
    /// The acceptance seam must run the same mode (`--symmetric-edges` wires
    /// both), or every dial is silently dropped as illegitimate.
    #[must_use]
    pub fn with_symmetric(mut self, symmetric: bool) -> Self {
        self.symmetric = symmetric;
        self
    }

    /// Pin the bucket count `B` for every topic instead of deriving it from
    /// the local candidate count (`--bucket-count`). When both seams pin the
    /// same value the edge predicate is verifiable by construction (see the
    /// B-agreement note on the type). A pinned `B` replaces the derived value
    /// **including the small-topic `B = 1` floor**. Validated `≥ 1` at build
    /// time.
    #[must_use]
    pub fn with_bucket_override(mut self, bucket_override: Option<usize>) -> Self {
        self.bucket_override = bucket_override;
        self
    }
}

impl LinkSelectionStrategy for HashGatedSelection {
    fn expected_links(&self, view: &NodeView<'_>) -> BTreeSet<(PeerId, TopicId)> {
        // One derivation site per topic (`resolve_buckets` — the derive-or-
        // override rule), one predicate evaluation per candidate under the
        // slot's role domain. Selection is a pure function of the view.
        // (Feature 016's symmetric variant re-extracts this loop into a shared
        // helper when it becomes the second consumer.)
        let mut expected = BTreeSet::new();
        for topic in view.subscriptions {
            let Some(peers) = view.candidates.get(topic) else {
                continue;
            };
            let buckets = resolve_buckets(self.bucket_override, peers.len(), self.degree);
            for candidate in peers {
                let valid = if self.symmetric {
                    is_valid_edge_sym(
                        self.role,
                        view.epoch_nonce,
                        topic,
                        &self.self_id,
                        candidate,
                        buckets,
                    )
                } else {
                    is_valid_edge_for(
                        self.role,
                        view.epoch_nonce,
                        topic,
                        &self.self_id,
                        candidate,
                        buckets,
                    )
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
    use super::HashGatedSelection;
    use crate::connection_state::LinkRole;
    use crate::strategies::edge::{bucket_count, is_valid_edge};
    use crate::strategies::selection::LinkSelectionStrategy;
    use crate::strategies::test_support::{
        candidates, downstream, peer, subscriptions, topic, view, view_with_nonce,
    };

    fn names(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("c{i}")).collect()
    }

    // 005 FR-001: exactly the predicate-valid candidates are selected on the
    // relay slot, reproducibly.
    #[test]
    fn relay_selection_matches_predicate_and_reproduces() {
        let subs = subscriptions(&["t1"]);
        let names = names(24);
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let cands = candidates(&[("t1", &refs[..])]);
        let store = downstream(&[]);
        let degree = 4;
        let buckets = bucket_count(24, degree);

        let policy = HashGatedSelection::new(LinkRole::Relay, peer("self"), degree);
        let selected = policy.expected_links(&view_with_nonce(&subs, &cands, &store, 7));
        let expected: std::collections::BTreeSet<_> = refs
            .iter()
            .filter(|c| is_valid_edge(7, &topic("t1"), &peer("self"), &peer(c), buckets))
            .map(|c| (peer(c), topic("t1")))
            .collect();
        assert_eq!(selected, expected, "selection = the predicate's edge set");
        let again = policy.expected_links(&view_with_nonce(&subs, &cands, &store, 7));
        assert_eq!(selected, again, "same nonce reproduces the selection");
    }

    // 005: a small topic (≤ ~degree candidates) floors B to 1 → select all.
    #[test]
    fn small_topic_selects_all_candidates() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a", "b", "c"])]);
        let store = downstream(&[]);
        let selected = HashGatedSelection::new(LinkRole::Relay, peer("self"), 8)
            .expected_links(&view(&subs, &cands, &store));
        assert_eq!(selected.len(), 3, "B = 1 selects every candidate");
    }

    // 015 FR-009/SC-004: relay and publish slots draw from distinct hash
    // domains — over a nonce sweep their selections must differ at least once.
    #[test]
    fn publish_draw_is_independent_of_relay_draw() {
        let subs = subscriptions(&["t1"]);
        let names = names(16);
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let cands = candidates(&[("t1", &refs[..])]);
        let store = downstream(&[]);
        let differs = (0..200u64).any(|nonce| {
            let view = view_with_nonce(&subs, &cands, &store, nonce);
            let relay = HashGatedSelection::new(LinkRole::Relay, peer("self"), 4)
                .with_bucket_override(Some(4))
                .expected_links(&view);
            let publish = HashGatedSelection::new(LinkRole::Publisher, peer("self"), 4)
                .with_bucket_override(Some(4))
                .expected_links(&view);
            relay != publish
        });
        assert!(
            differs,
            "relay and publish selections must be independent draws",
        );
    }

    // 015 (ADR 0034): the publish slot selects unconditionally — the same view
    // yields initiation links regardless of any relay-side state (the former
    // M3 trigger is gone: standing initiation links are always established).
    #[test]
    fn publish_selection_is_unconditional() {
        let subs = subscriptions(&["t1"]);
        let names = names(12);
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let cands = candidates(&[("t1", &refs[..])]);
        // A store ALREADY holding relay downstream — irrelevant to selection.
        let store = downstream(&[("c0", "t1"), ("c1", "t1")]);
        let with_downstream = HashGatedSelection::new(LinkRole::Publisher, peer("self"), 3)
            .expected_links(&view(&subs, &cands, &store));
        let empty = downstream(&[]);
        let without_downstream = HashGatedSelection::new(LinkRole::Publisher, peer("self"), 3)
            .expected_links(&view(&subs, &cands, &empty));
        assert_eq!(
            with_downstream, without_downstream,
            "initiation-link selection ignores relay state",
        );
        assert!(
            !with_downstream.is_empty(),
            "a 12-candidate topic at degree 3 selects some targets",
        );
    }

    // ADR 0035 / M4: under the symmetric mode, A's expected set contains B iff
    // B's contains A — the pair emergence that makes every link bidirectional.
    #[test]
    fn symmetric_selection_is_reciprocal() {
        let subs = subscriptions(&["t1"]);
        let names = names(16);
        let mut all: Vec<&str> = names.iter().map(String::as_str).collect();
        all.push("self");
        let cands_of_self = candidates(&[("t1", &all[..all.len() - 1])]);
        let store = downstream(&[]);
        let view_self = view_with_nonce(&subs, &cands_of_self, &store, 9);
        let selected_by_self = HashGatedSelection::new(LinkRole::Relay, peer("self"), 4)
            .with_symmetric(true)
            .expected_links(&view_self);

        for c in &names {
            // c's candidate set: everyone except c (self included). Same size
            // as self's, so the derived B agrees — the v1 full-view property.
            let others: Vec<&str> = all.iter().copied().filter(|n| n != &c.as_str()).collect();
            let cands_of_c = candidates(&[("t1", &others[..])]);
            let view_c = view_with_nonce(&subs, &cands_of_c, &store, 9);
            let selected_by_c = HashGatedSelection::new(LinkRole::Relay, peer(c), 4)
                .with_symmetric(true)
                .expected_links(&view_c);
            assert_eq!(
                selected_by_self.contains(&(peer(c), topic("t1"))),
                selected_by_c.contains(&(peer("self"), topic("t1"))),
                "reciprocity for {c}",
            );
        }
    }
}

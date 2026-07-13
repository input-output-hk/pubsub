//! The verifiable hash-gated publish-target policy: [`HashGatedPublish`]
//! (feature 015, ADR 0033).

use std::collections::BTreeSet;

use super::PublishStrategy;
use crate::connection_state::LinkRole;
use crate::peer::PeerId;
use crate::strategies::edge::{hash_gated_selection, is_valid_edge, resolve_buckets};
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

/// The verifiable, bucketed publish-target policy (ADR 0033).
///
/// For each joined topic `T`, the policy first evaluates the **M3 trigger**:
/// would any candidate select this node as a relay upstream under the current
/// epoch nonce? That is the *expected* relay downstream, recomputed from the
/// public relay predicate in the inbound direction — deterministic at dial
/// time, no dependence on observed acceptance timing. Only when the expected
/// relay downstream is **empty** (the node's published messages have no relay
/// path into the overlay) does it select targets: candidate `U` is a publishing
/// target iff the **publish** edge predicate holds —
/// `H_publish(nonce, T, self, U) mod B_p == 0` under the publish hash domain,
/// with `B_p = max(1, round(|candidates_T| / publish_degree))`. Expected
/// publish out-degree ≈ `publish_degree`, an independent hash draw from the
/// relay edge set.
///
/// **Trigger residual (documented, ADR 0033)**: observed relay downstream can
/// under-fill the expected set (over-capacity rejections, un-synced peers); a
/// node in that state forms no publishing links until a later heartbeat under a
/// changed epoch — the same under-fill class 005 accepted for the relay degree.
///
/// The B-agreement assumption of the relay seam applies unchanged: both the
/// trigger (relay `B`) and the selection (`B_p`) derive bucket counts from the
/// local candidate count; a pinned [`bucket_override`](Self::with_bucket_override)
/// removes the dependence for the publish side.
pub struct HashGatedPublish {
    self_id: PeerId,
    publish_degree: usize,
    /// The relay degree the *trigger* recomputes the relay predicate with —
    /// must match the relay seam's configuration for the expected-downstream
    /// computation to mirror the candidates' dial decisions.
    relay_degree: usize,
    bucket_override: Option<usize>,
}

impl HashGatedPublish {
    /// Build the policy from already-parsed inputs. `publish_degree` sizes the
    /// publish selection; `relay_degree` parameterises the trigger's
    /// expected-relay-downstream recomputation (it must match the relay seam's
    /// degree). Publish `B_p` is derived per topic from `publish_degree`; use
    /// [`with_bucket_override`](Self::with_bucket_override) to pin it.
    #[must_use]
    pub fn new(self_id: PeerId, publish_degree: usize, relay_degree: usize) -> Self {
        Self {
            self_id,
            publish_degree,
            relay_degree,
            bucket_override: None,
        }
    }

    /// Pin the publish bucket count `B_p` instead of deriving it from the local
    /// candidate count (`--bucket-count`; same semantics as the relay seams,
    /// including the loss of the small-topic `B_p = 1` floor). The trigger's
    /// relay-side bucket count is always derived — it mirrors the candidates'
    /// own dial derivation.
    #[must_use]
    pub fn with_bucket_override(mut self, bucket_override: Option<usize>) -> Self {
        self.bucket_override = bucket_override;
        self
    }

    /// The M3 trigger: whether any candidate on `topic` would select this node
    /// as a relay upstream under the current epoch nonce (the node's *expected*
    /// relay downstream is non-empty).
    fn has_expected_relay_downstream(
        &self,
        view: &NodeView<'_>,
        topic: &TopicId,
        candidates: &BTreeSet<PeerId>,
    ) -> bool {
        // Each candidate D derives its relay B from ITS view's candidate count
        // for the topic. v1 views are the full candidate set, so D's count is
        // the topic's member count minus D itself — the same value this node's
        // own count represents (all members minus self). The counts agree by
        // construction; the derivation mirrors `HashGatedConnection`.
        let buckets = resolve_buckets(None, candidates.len(), self.relay_degree);
        candidates.iter().any(|candidate| {
            is_valid_edge(view.epoch_nonce, topic, candidate, &self.self_id, buckets)
        })
    }
}

impl PublishStrategy for HashGatedPublish {
    fn expected_publish(&self, view: &NodeView<'_>) -> BTreeSet<(PeerId, TopicId)> {
        // The M3 trigger per topic, then the shared selection core under the
        // publish domain (ADR 0033): a node someone will pull from needs no
        // publishing links on that topic.
        let mut expected = hash_gated_selection(
            LinkRole::Publisher,
            &self.self_id,
            self.publish_degree,
            self.bucket_override,
            view,
        );
        expected.retain(|(_, topic)| {
            view.candidates
                .get(topic)
                .is_some_and(|peers| !self.has_expected_relay_downstream(view, topic, peers))
        });
        expected
    }
}

#[cfg(test)]
mod tests {
    use super::HashGatedPublish;
    use crate::strategies::connection::{ConnectionStrategy, HashGatedConnection};
    use crate::strategies::publish::PublishStrategy;
    use crate::strategies::test_support::{
        candidates, downstream, peer, subscriptions, topic, view_with_nonce,
    };

    // Find an epoch nonce under which no candidate selects `self` as an
    // upstream on t1 (the trigger fires), or one under which some candidate
    // does (the trigger holds it back).
    fn nonce_where_trigger(fires: bool, relay_degree: usize) -> u64 {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[(
            "t1",
            &(0..12)
                .map(|i| format!("c{i}"))
                .collect::<Vec<_>>()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()[..],
        )]);
        let store = downstream(&[]);
        (0..10_000u64)
            .find(|nonce| {
                let view = view_with_nonce(&subs, &cands, &store, *nonce);
                // "self" is selected by candidate D iff D's hash-gated dial
                // includes it; mirror via each candidate's HashGatedConnection
                // would need D's own view — recompute directly instead.
                let selected = cands[&topic("t1")].iter().any(|d| {
                    let buckets = crate::strategies::edge::resolve_buckets(
                        None,
                        cands[&topic("t1")].len(),
                        relay_degree,
                    );
                    crate::strategies::edge::is_valid_edge(
                        view.epoch_nonce,
                        &topic("t1"),
                        d,
                        &peer("self"),
                        buckets,
                    )
                });
                selected != fires
            })
            .expect("a nonce with the wanted trigger state exists in the sweep")
    }

    // 015 FR-009b/SC-003: with no expected relay downstream the policy selects
    // ~publish_degree targets via the publish predicate.
    #[test]
    fn trigger_fires_selects_publish_targets() {
        let relay_degree = 3;
        let nonce = nonce_where_trigger(true, relay_degree);
        let subs = subscriptions(&["t1"]);
        let names: Vec<String> = (0..12).map(|i| format!("c{i}")).collect();
        let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let cands = candidates(&[("t1", &name_refs[..])]);
        let store = downstream(&[]);
        let view = view_with_nonce(&subs, &cands, &store, nonce);
        let selected = HashGatedPublish::new(peer("self"), 3, relay_degree).expected_publish(&view);
        assert!(
            !selected.is_empty(),
            "a triggered publisher selects publish targets",
        );
        // Reproducible: the same view yields the same selection.
        let again = HashGatedPublish::new(peer("self"), 3, relay_degree).expected_publish(&view);
        assert_eq!(selected, again, "selection is deterministic");
    }

    // 015 FR-009b: with expected relay downstream present, no publishing links.
    #[test]
    fn trigger_held_selects_nothing() {
        let relay_degree = 3;
        let nonce = nonce_where_trigger(false, relay_degree);
        let subs = subscriptions(&["t1"]);
        let names: Vec<String> = (0..12).map(|i| format!("c{i}")).collect();
        let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let cands = candidates(&[("t1", &name_refs[..])]);
        let store = downstream(&[]);
        let view = view_with_nonce(&subs, &cands, &store, nonce);
        assert!(
            HashGatedPublish::new(peer("self"), 3, relay_degree)
                .expected_publish(&view)
                .is_empty(),
            "an expected relay downstream suppresses publishing links",
        );
    }

    // 015 FR-009/SC-004: the publish selection is an independent draw from the
    // relay selection — for the same view, the publish target set is not the
    // relay expected set re-tagged (over a nonce sweep they must differ at
    // least once).
    #[test]
    fn publish_draw_is_independent_of_relay_draw() {
        let subs = subscriptions(&["t1"]);
        let names: Vec<String> = (0..16).map(|i| format!("c{i}")).collect();
        let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let cands = candidates(&[("t1", &name_refs[..])]);
        let store = downstream(&[]);
        let differs = (0..200u64).any(|nonce| {
            let view = view_with_nonce(&subs, &cands, &store, nonce);
            let publish = HashGatedPublish::new(peer("self"), 4, 4)
                .with_bucket_override(Some(4))
                .expected_publish(&view);
            let relay = HashGatedConnection::new(peer("self"), 4)
                .with_bucket_override(Some(4))
                .expected_relay(&view);
            publish != relay
        });
        assert!(
            differs,
            "publish and relay selections must be independent draws",
        );
    }
}

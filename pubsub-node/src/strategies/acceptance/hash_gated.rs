//! The hash-gated-only inbound-acceptance policy: [`HashGatedAcceptance`]
//! (one-dimensional baseline, ADR 0031).

use std::collections::BTreeSet;

use super::{admit_prelude, Admission, ConnectionAcceptanceStrategy};
use crate::connection_state::LinkKind;
use crate::peer::PeerId;
use crate::strategies::edge::{
    is_valid_edge, is_valid_edge_publisher, is_valid_edge_sym, resolve_buckets,
};
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

/// Accept a verified `Request` iff it is membership-valid and the **verifiable
/// edge predicate** holds under the current epoch nonce — the **hash gate
/// without the cap**, isolating the verifiability dimension for the empirical
/// baseline experiments (ADR 0031).
///
/// Never returns `RejectOverCapacity` (no downstream bound is enforced); a
/// predicate failure is the same silent `RejectIllegitimate` as in the compound
/// [`HashGatedBoundedAcceptance`](super::HashGatedBoundedAcceptance), verified
/// with the same shared predicate the dialer used.
pub struct HashGatedAcceptance {
    self_id: PeerId,
    target_degree: usize,
    bucket_override: Option<usize>,
    kind: LinkKind,
    symmetric: bool,
}

impl HashGatedAcceptance {
    /// Build the policy from already-parsed inputs. `B` is derived per topic
    /// from `target_degree`; use [`with_bucket_override`](Self::with_bucket_override)
    /// to pin it — it must match the dialer's `B`.
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

    /// Verify inbound requests against the **symmetric** edge predicate (M4) —
    /// must be enabled together with the dial side's symmetric mode, or the
    /// two ends disagree and every dial is silently dropped (one CLI flag
    /// drives both).
    #[must_use]
    pub fn with_symmetric(mut self, symmetric: bool) -> Self {
        self.symmetric = symmetric;
        self
    }

    /// Re-target the instance at a link kind (`Relay` is the constructor
    /// default): the kind selects the hash domain the predicate is verified
    /// under and which accepted-link class the prelude scans.
    #[must_use]
    pub fn for_kind(mut self, kind: LinkKind) -> Self {
        self.kind = kind;
        self
    }

    /// Pin the bucket count `B` used to verify the edge predicate (see
    /// [`HashGatedBoundedAcceptance::with_bucket_override`](super::HashGatedBoundedAcceptance::with_bucket_override)
    /// — same semantics, including the loss of the small-topic `B = 1` floor).
    #[must_use]
    pub fn with_bucket_override(mut self, bucket_override: Option<usize>) -> Self {
        self.bucket_override = bucket_override;
        self
    }
}

impl ConnectionAcceptanceStrategy for HashGatedAcceptance {
    fn admit(&self, emitter: &PeerId, topic: &TopicId, view: &NodeView<'_>) -> Admission {
        if let Err(decision) = admit_prelude(self.kind, emitter, topic, view) {
            return decision;
        }
        // Same B-agreement assumption as the compound policy (see
        // `hash_gated_bounded.rs` for the full note): derived B matches the
        // dialer's only while both ends see the same candidate set; the pinned
        // override removes the dependence.
        let candidate_count = view.candidates.get(topic).map_or(0, BTreeSet::len);
        let buckets = resolve_buckets(self.bucket_override, candidate_count, self.target_degree);
        let valid = if self.symmetric {
            is_valid_edge_sym(view.epoch_nonce, topic, emitter, &self.self_id, buckets)
        } else {
            match self.kind {
                LinkKind::Relay => {
                    is_valid_edge(view.epoch_nonce, topic, emitter, &self.self_id, buckets)
                }
                LinkKind::Publisher => is_valid_edge_publisher(
                    view.epoch_nonce,
                    topic,
                    emitter,
                    &self.self_id,
                    buckets,
                ),
            }
        };
        if valid {
            Admission::Accept
        } else {
            Admission::RejectIllegitimate
        }
    }
}

#[cfg(test)]
mod tests {
    use super::HashGatedAcceptance;
    use crate::strategies::acceptance::{Admission, ConnectionAcceptanceStrategy};
    use crate::strategies::edge::{bucket_count, is_valid_edge};
    use crate::strategies::test_support::{
        candidates, downstream, peer, subscriptions, topic, view,
    };
    use std::collections::BTreeMap;

    // Membership failure takes precedence and is a silent RejectMembership.
    #[test]
    fn membership_invalid_is_rejected() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t2", &["a"])]);
        let down = BTreeMap::new();
        let got = HashGatedAcceptance::new(peer("self"), 1).admit(
            &peer("a"),
            &topic("t2"), // not subscribed
            &view(&subs, &cands, &down),
        );
        assert_eq!(got, Admission::RejectMembership);
    }

    // The gate refuses a predicate-failing member (silent RejectIllegitimate)
    // and admits a predicate-passing one — with NO cap: even a topic already
    // holding many downstreams keeps accepting legitimate edges.
    #[test]
    fn gates_on_the_predicate_without_a_cap() {
        let t = topic("t1");
        let names = ["a", "b", "c", "d", "e", "f"]; // 6 candidates
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &names)]);
        let buckets = bucket_count(names.len(), 1); // 6/1 = 6 > 1
        let policy = HashGatedAcceptance::new(peer("self"), 1);

        // Far beyond the compound policy's cap (target_degree=1 ⇒ OC=4): 6 held.
        let heavy = downstream(&[
            ("u", "t1"),
            ("v", "t1"),
            ("w", "t1"),
            ("x", "t1"),
            ("y", "t1"),
            ("z", "t1"),
        ]);
        for n in names {
            let expected = if is_valid_edge(0, &t, &peer(n), &peer("self"), buckets) {
                Admission::Accept // no RejectOverCapacity, ever
            } else {
                Admission::RejectIllegitimate
            };
            assert_eq!(
                policy.admit(&peer(n), &t, &view(&subs, &cands, &heavy)),
                expected,
                "member {n}: gate-only admission must mirror the predicate",
            );
        }
    }

    // The shared prelude: a re-dial of an already-held downstream re-Accepts
    // even when its edge predicate would fail (the half-open-link repair).
    #[test]
    fn already_downstream_peer_is_reaccepted_despite_failing_predicate() {
        let t = topic("t1");
        let names = ["a", "b", "c", "d", "e", "f"];
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &names)]);
        let buckets = bucket_count(names.len(), 1);
        let failing = names
            .iter()
            .map(|n| peer(n))
            .find(|p| !is_valid_edge(0, &t, p, &peer("self"), buckets))
            .expect("some candidate fails the predicate at B=6");
        let held = downstream(&[(&failing.to_string(), "t1")]);
        assert_eq!(
            HashGatedAcceptance::new(peer("self"), 1).admit(
                &failing,
                &t,
                &view(&subs, &cands, &held)
            ),
            Admission::Accept,
        );
    }
}

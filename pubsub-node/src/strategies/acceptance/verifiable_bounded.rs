//! The verifiable, bounded inbound-acceptance policy:
//! [`VerifiableBoundedAcceptance`] (bucketed-pull, ADR 0025).

use std::collections::BTreeSet;

use super::{downstream_scan, is_membership_valid, Admission, ConnectionAcceptanceStrategy};
use crate::peer::PeerId;
use crate::strategies::edge::{accept_cap, is_valid_edge, resolve_buckets};
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

/// Accept a verified `Request` iff it is membership-valid, the **verifiable edge
/// predicate** holds for this interval, and the node is under its per-topic
/// downstream cap; refuse otherwise (ADR 0025).
///
/// The acceptor recomputes the *same* predicate the dialer used
/// (`H(genesis, topic, requester, self, interval) mod B == 0`, `strategies::edge`),
/// so an adversary cannot force an edge the hash does not allow — a predicate
/// failure is a **silent** `RejectIllegitimate`. Over the per-topic cap
/// `OC = ⌈target_degree + c·√target_degree⌉` a legitimate request is refused with
/// `RejectOverCapacity` (an explicit `Rejected`).
pub struct VerifiableBoundedAcceptance {
    genesis: u64,
    self_id: PeerId,
    target_degree: usize,
    cap_buffer: usize,
    bucket_override: Option<usize>,
}

impl VerifiableBoundedAcceptance {
    /// Build the policy from already-parsed inputs (`cap_buffer` is the `c` in
    /// `OC = ⌈target_degree + c·√target_degree⌉`). `B` is derived per topic from
    /// `target_degree`; use [`with_bucket_override`](Self::with_bucket_override)
    /// to pin it — it must match the dialer's `B`.
    #[must_use]
    pub fn new(genesis: u64, self_id: PeerId, target_degree: usize, cap_buffer: usize) -> Self {
        Self {
            genesis,
            self_id,
            target_degree,
            cap_buffer,
            bucket_override: None,
        }
    }

    /// Pin the bucket count `B` used to verify the edge predicate, instead of
    /// deriving it from the local candidate count (`--bucket-count`). Must match
    /// the value the dialer uses; when both ends pin the same `B`, verification
    /// holds by construction regardless of local fold state. Validated `≥ 1` at
    /// build time.
    ///
    /// A pinned `B` replaces the derived value **including the small-topic
    /// `B = 1` floor** — small topics are no longer accept-all under an override.
    #[must_use]
    pub fn with_bucket_override(mut self, bucket_override: Option<usize>) -> Self {
        self.bucket_override = bucket_override;
        self
    }
}

impl ConnectionAcceptanceStrategy for VerifiableBoundedAcceptance {
    fn admit(&self, emitter: &PeerId, topic: &TopicId, view: &NodeView<'_>) -> Admission {
        if !is_membership_valid(emitter, topic, view.subscriptions, view.candidates) {
            return Admission::RejectMembership;
        }
        // An already-accepted (emitter, topic) is re-affirmed idempotently, ahead
        // of the edge and cap checks: a re-dial re-sends Accepted and never trips
        // the cap, so a lost/late Accepted (the AwaitingAccept re-request path)
        // repairs the link rather than stranding a half-open connection — the node
        // fans out to a downstream that the dialer has dropped (005 FR-013).
        // One borrow-only pass yields both downstream facts the policy needs.
        let (already_downstream, downstream_on_topic) =
            downstream_scan(view.downstream, emitter, topic);
        if already_downstream {
            return Admission::Accept;
        }
        // Verify against the same edge predicate the dialer used, with the same
        // bucket count. B-agreement assumption: deriving B locally from the
        // candidate count only matches the dialer's B while both ends see the
        // same set — true in v1 (full candidate set; dials fire after `Synced`),
        // but registry-fold lag or a future discovery layer (the `H_v` caveat in
        // `strategies::edge`) can diverge the counts and silently fail otherwise-
        // legitimate requests. A pinned `bucket_override` removes the dependence
        // (both ends use the same configured B). The emitter is the requester,
        // this node the candidate.
        let candidate_count = view.candidates.get(topic).map_or(0, BTreeSet::len);
        let buckets = resolve_buckets(self.bucket_override, candidate_count, self.target_degree);
        if !is_valid_edge(
            self.genesis,
            topic,
            emitter,
            &self.self_id,
            view.interval,
            buckets,
        ) {
            return Admission::RejectIllegitimate;
        }
        let cap = accept_cap(self.target_degree, self.cap_buffer);
        if downstream_on_topic >= cap {
            Admission::RejectOverCapacity
        } else {
            Admission::Accept
        }
    }
}

#[cfg(test)]
mod tests {
    use super::VerifiableBoundedAcceptance;
    use crate::strategies::acceptance::{Admission, ConnectionAcceptanceStrategy};
    use crate::strategies::edge::{bucket_count, is_valid_edge};
    use crate::strategies::test_support::{
        candidates, downstream, peer, subscriptions, topic, view,
    };
    use std::collections::HashSet;

    // Membership failure takes precedence and is a silent RejectMembership.
    #[test]
    fn membership_invalid_is_rejected() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t2", &["a"])]);
        let down = HashSet::new();
        let got = VerifiableBoundedAcceptance::new(0, peer("self"), 1, 3).admit(
            &peer("a"),
            &topic("t2"), // not subscribed
            &view(&subs, &cands, &down),
        );
        assert_eq!(got, Admission::RejectMembership);
    }

    // A membership-valid request whose edge predicate fails for the interval is
    // RejectIllegitimate (silent) — the acceptor verifies; an adversary cannot
    // force the edge.
    #[test]
    fn predicate_failure_is_rejected_as_illegitimate() {
        let t = topic("t1");
        let names = ["a", "b", "c", "d", "e", "f"]; // 6 candidates
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &names)]);
        let down = HashSet::new();
        let buckets = bucket_count(names.len(), 1); // 6/1 = 6 > 1
        let invalid = names
            .iter()
            .map(|n| peer(n))
            .find(|p| !is_valid_edge(0, &t, p, &peer("self"), 0, buckets))
            .expect("some candidate fails the predicate at B=6");
        let got = VerifiableBoundedAcceptance::new(0, peer("self"), 1, 3).admit(
            &invalid,
            &t,
            &view(&subs, &cands, &down),
        );
        assert_eq!(got, Admission::RejectIllegitimate);
    }

    // A legitimate (predicate-valid) request is Accepted below cap and
    // RejectOverCapacity at cap.
    #[test]
    fn legitimate_request_accepts_below_cap_and_rejects_at_cap() {
        let t = topic("t1");
        let subs = subscriptions(&["t1"]);
        // Pin B=2 (via the override) so ~half of any namespace is legitimate,
        // then find a member that passes the predicate against this acceptor —
        // robust to the exact hash rather than betting on a handful of names.
        let valid_name = (0..10_000)
            .map(|i| format!("cand-{i}"))
            .find(|n| is_valid_edge(0, &t, &peer(n), &peer("self"), 0, 2))
            .expect("some candidate passes the predicate at B=2");
        let cands = candidates(&[("t1", &[valid_name.as_str()])]);
        let valid = peer(&valid_name);
        let policy =
            VerifiableBoundedAcceptance::new(0, peer("self"), 1, 3).with_bucket_override(Some(2));

        // Below cap (3 held, target_degree=1 ⇒ cap 4) ⇒ Accept.
        let below = downstream(&[("x", "t1"), ("y", "t1"), ("z", "t1")]);
        assert_eq!(
            policy.admit(&valid, &t, &view(&subs, &cands, &below)),
            Admission::Accept,
        );

        // At cap (4 held, none of them the requester) ⇒ RejectOverCapacity +
        // (handler sends Rejected).
        let at = downstream(&[("w", "t1"), ("x", "t1"), ("y", "t1"), ("z", "t1")]);
        assert_eq!(
            policy.admit(&valid, &t, &view(&subs, &cands, &at)),
            Admission::RejectOverCapacity,
        );
    }

    // A pinned bucket override is used to verify instead of the derived B: with
    // B=1 every membership-valid request passes the predicate (below cap).
    #[test]
    fn bucket_override_pins_verification() {
        let t = topic("t1");
        let names = ["a", "b", "c", "d", "e", "f"];
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &names)]);
        let down = HashSet::new();
        // Derived B on 6 candidates at target_degree=1 would be 6 (most requests
        // illegitimate); pinned B=1 makes every membership-valid request valid.
        let policy =
            VerifiableBoundedAcceptance::new(0, peer("self"), 1, 3).with_bucket_override(Some(1));
        assert_eq!(
            policy.admit(&peer("a"), &t, &view(&subs, &cands, &down)),
            Admission::Accept,
        );
    }

    // A re-dial of an already-accepted peer is re-affirmed even at cap: the
    // already-held downstream entry short-circuits to Accept, so a lost/late
    // Accepted repairs the link instead of leaving it permanently half-open.
    #[test]
    fn already_downstream_peer_is_reaccepted_at_cap() {
        let t = topic("t1");
        let subs = subscriptions(&["t1"]);
        // 'a' is a member; the short-circuit fires ahead of the edge check, so no
        // dependence on whether 'a' would pass the predicate.
        let cands = candidates(&[("t1", &["a"])]);
        let policy = VerifiableBoundedAcceptance::new(0, peer("self"), 1, 3);

        // At cap (4 held, target_degree=1 ⇒ cap 4) AND 'a' is one of the held
        // downstreams ⇒ the idempotent re-Accept wins over RejectOverCapacity.
        let at_cap_with_a = downstream(&[("a", "t1"), ("x", "t1"), ("y", "t1"), ("z", "t1")]);
        assert_eq!(
            policy.admit(&peer("a"), &t, &view(&subs, &cands, &at_cap_with_a)),
            Admission::Accept,
        );
    }

    // Small topic (≤ target_degree candidates ⇒ B=1) admits every membership-valid
    // request (connect-to-all): the predicate is always true, only the cap bounds.
    #[test]
    fn small_topic_admits_every_member_below_cap() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a", "b"])]); // 2 ≤ target_degree ⇒ B=1
        let down = HashSet::new();
        let got = VerifiableBoundedAcceptance::new(0, peer("self"), 8, 3).admit(
            &peer("a"),
            &topic("t1"),
            &view(&subs, &cands, &down),
        );
        assert_eq!(got, Admission::Accept);
    }
}

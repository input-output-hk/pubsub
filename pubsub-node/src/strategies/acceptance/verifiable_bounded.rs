//! The verifiable, bounded inbound-acceptance policy:
//! [`VerifiableBoundedAcceptance`] (bucketed-pull, ADR 0025).

use std::collections::{BTreeMap, BTreeSet, HashSet};

use super::{is_membership_valid, Admission, ConnectionAcceptanceStrategy};
use crate::peer::PeerId;
use crate::strategies::edge::{accept_cap, bucket_count, is_valid_edge};
use crate::topic::TopicId;

/// Accept a verified `Request` iff it is membership-valid, the **verifiable edge
/// predicate** holds for this interval, and the node is under its per-topic
/// downstream cap; refuse otherwise (ADR 0025).
///
/// The acceptor recomputes the *same* predicate the dialer used
/// (`H(genesis, topic, requester, self, interval) mod B == 0`, `strategies::edge`),
/// so an adversary cannot force an edge the hash does not allow — a predicate
/// failure is a **silent** `RejectIllegitimate`. Over the per-topic cap
/// `OC = ⌈rf + c·√rf⌉` a legitimate request is refused with `RejectOverCapacity`
/// (an explicit `Rejected`).
pub struct VerifiableBoundedAcceptance {
    genesis: u64,
    self_id: PeerId,
    rf: usize,
    cap_buffer: usize,
}

impl VerifiableBoundedAcceptance {
    /// Build the policy from already-parsed inputs (`cap_buffer` is the `c` in
    /// `OC = ⌈rf + c·√rf⌉`).
    #[must_use]
    pub fn new(genesis: u64, self_id: PeerId, rf: usize, cap_buffer: usize) -> Self {
        Self {
            genesis,
            self_id,
            rf,
            cap_buffer,
        }
    }
}

impl ConnectionAcceptanceStrategy for VerifiableBoundedAcceptance {
    fn admit(
        &self,
        emitter: &PeerId,
        topic: &TopicId,
        subscriptions: &BTreeSet<TopicId>,
        candidates: &BTreeMap<TopicId, BTreeSet<PeerId>>,
        downstream: &HashSet<(PeerId, TopicId)>,
        interval: u64,
    ) -> Admission {
        if !is_membership_valid(emitter, topic, subscriptions, candidates) {
            return Admission::RejectMembership;
        }
        // Verify the request against the same edge predicate the dialer used, with
        // the same bucket count (both sides see the topic's full candidate set,
        // each minus itself, so the counts and thus B agree). The emitter is the
        // requester, this node the candidate.
        let candidate_count = candidates.get(topic).map_or(0, BTreeSet::len);
        let buckets = bucket_count(candidate_count, self.rf);
        if !is_valid_edge(
            self.genesis,
            topic,
            emitter,
            &self.self_id,
            interval,
            buckets,
        ) {
            return Admission::RejectIllegitimate;
        }
        let cap = accept_cap(self.rf, self.cap_buffer);
        let downstream_on_topic = downstream.iter().filter(|(_, t)| t == topic).count();
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
    use crate::peer::PeerId;
    use crate::strategies::acceptance::{Admission, ConnectionAcceptanceStrategy};
    use crate::strategies::edge::{bucket_count, is_valid_edge};
    use crate::topic::TopicId;
    use std::collections::{BTreeMap, BTreeSet, HashSet};
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
    fn downstream(entries: &[(&str, &str)]) -> HashSet<(PeerId, TopicId)> {
        entries.iter().map(|(p, t)| (peer(p), topic(t))).collect()
    }

    // Membership failure takes precedence and is a silent RejectMembership.
    #[test]
    fn membership_invalid_is_rejected() {
        let got = VerifiableBoundedAcceptance::new(0, peer("self"), 1, 3).admit(
            &peer("a"),
            &topic("t2"), // not subscribed
            &subscriptions(&["t1"]),
            &candidates(&[("t2", &["a"])]),
            &HashSet::new(),
            0,
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
        let cands = candidates(&[("t1", &names)]);
        let rf = 1usize;
        let buckets = bucket_count(names.len(), rf); // 6/1 = 6 > 1
                                                     // Find a candidate whose edge (candidate -> self) does NOT hold.
        let invalid = names
            .iter()
            .map(|n| peer(n))
            .find(|p| !is_valid_edge(0, &t, p, &peer("self"), 0, buckets))
            .expect("some candidate fails the predicate at B=6");
        let got = VerifiableBoundedAcceptance::new(0, peer("self"), rf, 3).admit(
            &invalid,
            &t,
            &subscriptions(&["t1"]),
            &cands,
            &HashSet::new(),
            0,
        );
        assert_eq!(got, Admission::RejectIllegitimate);
    }

    // A legitimate (predicate-valid) request is Accepted below cap and
    // RejectOverCapacity at cap.
    #[test]
    fn legitimate_request_accepts_below_cap_and_rejects_at_cap() {
        let t = topic("t1");
        let names = ["a", "b", "c", "d", "e", "f"];
        let cands = candidates(&[("t1", &names)]);
        let rf = 1usize; // cap = ceil(1 + 3*1) = 4
        let buckets = bucket_count(names.len(), rf);
        let valid = names
            .iter()
            .map(|n| peer(n))
            .find(|p| is_valid_edge(0, &t, p, &peer("self"), 0, buckets))
            .expect("some candidate passes the predicate at B=6");
        let policy = VerifiableBoundedAcceptance::new(0, peer("self"), rf, 3);

        // Below cap (3 held, cap 4) ⇒ Accept.
        let below = downstream(&[("x", "t1"), ("y", "t1"), ("z", "t1")]);
        assert_eq!(
            policy.admit(&valid, &t, &subscriptions(&["t1"]), &cands, &below, 0),
            Admission::Accept,
        );

        // At cap (4 held) ⇒ RejectOverCapacity + (handler sends Rejected).
        let at = downstream(&[("w", "t1"), ("x", "t1"), ("y", "t1"), ("z", "t1")]);
        assert_eq!(
            policy.admit(&valid, &t, &subscriptions(&["t1"]), &cands, &at, 0),
            Admission::RejectOverCapacity,
        );
    }

    // Small topic (≤ rf candidates ⇒ B=1) admits every membership-valid request
    // (connect-to-all): the predicate is always true, only the cap bounds it.
    #[test]
    fn small_topic_admits_every_member_below_cap() {
        let got = VerifiableBoundedAcceptance::new(0, peer("self"), 8, 3).admit(
            &peer("a"),
            &topic("t1"),
            &subscriptions(&["t1"]),
            &candidates(&[("t1", &["a", "b"])]), // 2 ≤ rf ⇒ B=1
            &HashSet::new(),
            0,
        );
        assert_eq!(got, Admission::Accept);
    }
}

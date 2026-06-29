//! The bounded inbound-acceptance policy: [`BoundedAcceptance`] (feature 005,
//! ADR 0025).

use std::collections::{HashMap, HashSet};

use super::{Admission, ConnectionAcceptanceStrategy};
use crate::peer::PeerId;
use crate::topic::TopicId;

/// Accept membership-valid requests up to a per-topic in-degree bound; refuse
/// further ones for over-capacity.
///
/// Membership validity is the same gate as [`AcceptFromAllCandidates`](super::AcceptFromAllCandidates)
/// (the topic is the node's own and the emitter is a known member); beyond that,
/// a request is accepted only while the node holds fewer than `in_degree`
/// downstream peers on the topic, and refused (`RejectOverCapacity`) once the
/// bound is reached.
pub struct BoundedAcceptance {
    in_degree: usize,
}

impl BoundedAcceptance {
    /// Build the policy with the given per-topic inbound bound.
    #[must_use]
    pub fn new(in_degree: usize) -> Self {
        Self { in_degree }
    }
}

impl ConnectionAcceptanceStrategy for BoundedAcceptance {
    fn admit(
        &self,
        emitter: &PeerId,
        topic: &TopicId,
        subscriptions: &HashSet<TopicId>,
        candidates: &HashMap<TopicId, HashSet<PeerId>>,
        downstream: &HashSet<(PeerId, TopicId)>,
    ) -> Admission {
        let membership_valid = subscriptions.contains(topic)
            && candidates
                .get(topic)
                .is_some_and(|peers| peers.contains(emitter));
        if !membership_valid {
            return Admission::RejectMembership;
        }
        let held_on_topic = downstream.iter().filter(|(_, t)| t == topic).count();
        if held_on_topic >= self.in_degree {
            Admission::RejectOverCapacity
        } else {
            Admission::Accept
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BoundedAcceptance;
    use crate::acceptance::{Admission, ConnectionAcceptanceStrategy};
    use crate::peer::PeerId;
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

    fn downstream(entries: &[(&str, &str)]) -> HashSet<(PeerId, TopicId)> {
        entries.iter().map(|(p, t)| (peer(p), topic(t))).collect()
    }

    // FR-010: below the in-degree bound on the topic ⇒ Accept.
    #[test]
    fn accepts_below_the_bound() {
        let got = BoundedAcceptance::new(2).admit(
            &peer("a"),
            &topic("t1"),
            &subscriptions(&["t1"]),
            &candidates(&[("t1", &["a", "b"])]),
            &downstream(&[("x", "t1")]), // 1 held, bound 2
        );
        assert_eq!(got, Admission::Accept);
    }

    // FR-010/FR-011: at the in-degree bound on the topic ⇒ RejectOverCapacity.
    #[test]
    fn rejects_over_capacity_at_the_bound() {
        let got = BoundedAcceptance::new(2).admit(
            &peer("a"),
            &topic("t1"),
            &subscriptions(&["t1"]),
            &candidates(&[("t1", &["a", "b"])]),
            &downstream(&[("x", "t1"), ("y", "t1")]), // 2 held, bound 2
        );
        assert_eq!(got, Admission::RejectOverCapacity);
    }

    // Only downstream on the *same* topic counts toward the bound.
    #[test]
    fn other_topic_downstream_does_not_count() {
        let got = BoundedAcceptance::new(1).admit(
            &peer("a"),
            &topic("t1"),
            &subscriptions(&["t1"]),
            &candidates(&[("t1", &["a"])]),
            &downstream(&[("x", "t2"), ("y", "t2")]), // both on t2, none on t1
        );
        assert_eq!(got, Admission::Accept);
    }

    // Membership failure takes precedence over the capacity check.
    #[test]
    fn membership_failure_precedes_capacity() {
        let got = BoundedAcceptance::new(0).admit(
            &peer("a"),
            &topic("t2"), // not a subscribed topic
            &subscriptions(&["t1"]),
            &candidates(&[("t2", &["a"])]),
            &HashSet::new(),
        );
        assert_eq!(got, Admission::RejectMembership);
    }

    // An in-degree of zero refuses every membership-valid request.
    #[test]
    fn zero_in_degree_refuses_all() {
        let got = BoundedAcceptance::new(0).admit(
            &peer("a"),
            &topic("t1"),
            &subscriptions(&["t1"]),
            &candidates(&[("t1", &["a"])]),
            &HashSet::new(),
        );
        assert_eq!(got, Admission::RejectOverCapacity);
    }
}

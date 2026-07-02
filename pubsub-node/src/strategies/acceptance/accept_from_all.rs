//! The v1 inbound-acceptance policy: [`AcceptFromAllCandidates`].

use super::{is_membership_valid, Admission, ConnectionAcceptanceStrategy};
use crate::peer::PeerId;
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

/// The v1 acceptance policy: accept every **membership-valid** request — the
/// requested topic is one the node is a member of, and the emitter is a known
/// member of it.
///
/// The exact inbound mirror of `ConnectToAllCandidates`: the "all" is
/// membership-scoped, not unconditional. It never refuses for over-capacity or
/// the edge predicate; the bounded counterpart is
/// [`VerifiableBoundedAcceptance`](super::VerifiableBoundedAcceptance).
pub struct AcceptFromAllCandidates;

impl ConnectionAcceptanceStrategy for AcceptFromAllCandidates {
    fn admit(&self, emitter: &PeerId, topic: &TopicId, view: &NodeView<'_>) -> Admission {
        if is_membership_valid(emitter, topic, view.subscriptions, view.candidates) {
            Admission::Accept
        } else {
            Admission::RejectMembership
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AcceptFromAllCandidates;
    use crate::peer::PeerId;
    use crate::strategies::acceptance::{Admission, ConnectionAcceptanceStrategy};
    use crate::strategies::view::NodeView;
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

    fn admit(emitter: &str, topic_id: &str, subs: &[&str], cands: &[(&str, &[&str])]) -> Admission {
        let subs = subscriptions(subs);
        let cands = candidates(cands);
        let down = HashSet::new();
        let view = NodeView {
            subscriptions: &subs,
            candidates: &cands,
            downstream: &down,
            interval: 0,
        };
        AcceptFromAllCandidates.admit(&peer(emitter), &topic(topic_id), &view)
    }

    // Accept: the topic is the node's own and the emitter is a known member.
    #[test]
    fn accepts_a_member_on_an_own_topic() {
        assert_eq!(
            admit("a", "t1", &["t1"], &[("t1", &["a", "b"])]),
            Admission::Accept
        );
    }

    // RejectMembership: the topic is not one the node is a member of.
    #[test]
    fn rejects_a_topic_the_node_is_not_a_member_of() {
        assert_eq!(
            admit("a", "t2", &["t1"], &[("t2", &["a"])]),
            Admission::RejectMembership,
        );
    }

    // RejectMembership: own topic, but the emitter is not a known member of it.
    #[test]
    fn rejects_a_non_member_emitter() {
        assert_eq!(
            admit("a", "t1", &["t1"], &[("t1", &["b"])]),
            Admission::RejectMembership,
        );
    }

    // RejectMembership: own topic with no discovered candidates at all.
    #[test]
    fn rejects_when_no_candidates_on_the_topic() {
        assert_eq!(admit("a", "t1", &["t1"], &[]), Admission::RejectMembership);
    }
}

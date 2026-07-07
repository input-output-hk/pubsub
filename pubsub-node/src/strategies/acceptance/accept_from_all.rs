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
/// [`HashGatedBoundedAcceptance`](super::HashGatedBoundedAcceptance).
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
    use crate::strategies::acceptance::{Admission, ConnectionAcceptanceStrategy};
    use crate::strategies::test_support::{candidates, peer, subscriptions, topic, view};
    use std::collections::HashSet;

    fn admit(emitter: &str, topic_id: &str, subs: &[&str], cands: &[(&str, &[&str])]) -> Admission {
        let subs = subscriptions(subs);
        let cands = candidates(cands);
        let down = HashSet::new();
        AcceptFromAllCandidates.admit(
            &peer(emitter),
            &topic(topic_id),
            &view(&subs, &cands, &down),
        )
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

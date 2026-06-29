//! The v1 inbound-acceptance policy: [`AcceptFromAllCandidates`].

use std::collections::{HashMap, HashSet};

use super::ConnectionAcceptanceStrategy;
use crate::peer::PeerId;
use crate::topic::TopicId;

/// The v1 acceptance policy: accept every **membership-valid** request — the
/// requested topic is one the node is a member of, and the emitter is a known
/// member of it.
///
/// The exact inbound mirror of `ConnectToAllCandidates`: the "all" is
/// membership-scoped, not unconditional. Discretionary restrictions (degree
/// caps, allowlists) are deferred to later strategies.
pub struct AcceptFromAllCandidates;

impl ConnectionAcceptanceStrategy for AcceptFromAllCandidates {
    fn accepts(
        &self,
        emitter: &PeerId,
        topic: &TopicId,
        subscriptions: &HashSet<TopicId>,
        candidates: &HashMap<TopicId, HashSet<PeerId>>,
    ) -> bool {
        let topic_is_own = subscriptions.contains(topic);
        let emitter_is_member = candidates
            .get(topic)
            .is_some_and(|peers| peers.contains(emitter));
        topic_is_own && emitter_is_member
    }
}

#[cfg(test)]
mod tests {
    use super::AcceptFromAllCandidates;
    use crate::acceptance::ConnectionAcceptanceStrategy;
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

    // Accept: the topic is the node's own and the emitter is a known member.
    #[test]
    fn accepts_a_member_on_an_own_topic() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a", "b"])]);
        assert!(AcceptFromAllCandidates.accepts(&peer("a"), &topic("t1"), &subs, &cands));
    }

    // Reject: the topic is not one the node is a member of.
    #[test]
    fn rejects_a_topic_the_node_is_not_a_member_of() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t2", &["a"])]);
        assert!(!AcceptFromAllCandidates.accepts(&peer("a"), &topic("t2"), &subs, &cands));
    }

    // Reject: own topic, but the emitter is not a known member of it.
    #[test]
    fn rejects_a_non_member_emitter() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["b"])]);
        assert!(!AcceptFromAllCandidates.accepts(&peer("a"), &topic("t1"), &subs, &cands));
    }

    // Reject: own topic with no discovered candidates at all.
    #[test]
    fn rejects_when_no_candidates_on_the_topic() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[]);
        assert!(!AcceptFromAllCandidates.accepts(&peer("a"), &topic("t1"), &subs, &cands));
    }
}

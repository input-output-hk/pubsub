//! The v1 connection-selection policy: [`ConnectToAllCandidates`].

use std::collections::{BTreeMap, BTreeSet};

use super::ConnectionStrategy;
use crate::peer::PeerId;
use crate::topic::TopicId;

/// The v1 connection-selection policy: connect to **every** candidate on
/// **every** topic the node is a member of.
///
/// Self-exclusion is input-borne — the candidate sets the node folds from the
/// subscription registry never contain its own id, so the expected set never
/// does either. This policy maintains the full per-topic mesh; degree limits and
/// the verifiable gate are the bounded policy's job ([`HashGatedConnection`](super::HashGatedConnection)).
pub struct ConnectToAllCandidates;

impl ConnectionStrategy for ConnectToAllCandidates {
    fn expected_upstream(
        &self,
        subscriptions: &BTreeSet<TopicId>,
        candidates: &BTreeMap<TopicId, BTreeSet<PeerId>>,
        _interval: u64,
    ) -> BTreeSet<(PeerId, TopicId)> {
        let mut expected = BTreeSet::new();
        for topic in subscriptions {
            if let Some(peers) = candidates.get(topic) {
                for peer in peers {
                    expected.insert((peer.clone(), topic.clone()));
                }
            }
        }
        expected
    }
}

#[cfg(test)]
mod tests {
    use super::ConnectToAllCandidates;
    use crate::peer::PeerId;
    use crate::strategies::connection::ConnectionStrategy;
    use crate::topic::TopicId;
    use std::collections::{BTreeMap, BTreeSet};
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

    // FR-006..009: v1 policy expects every candidate on every joined topic.
    #[test]
    fn expects_every_candidate_across_joined_topics() {
        let expected = ConnectToAllCandidates.expected_upstream(
            &subscriptions(&["t1", "t2"]),
            &candidates(&[("t1", &["a", "b"]), ("t2", &["c"])]),
            0,
        );
        assert_eq!(
            expected,
            BTreeSet::from([
                (peer("a"), topic("t1")),
                (peer("b"), topic("t1")),
                (peer("c"), topic("t2")),
            ]),
        );
    }

    // A candidate on a topic the node has not joined is not dialed — selection
    // is scoped to the node's own membership.
    #[test]
    fn candidates_on_unjoined_topics_are_ignored() {
        let expected = ConnectToAllCandidates.expected_upstream(
            &subscriptions(&["t1"]),
            &candidates(&[("t1", &["a"]), ("t2", &["b"])]),
            0,
        );
        assert_eq!(expected, BTreeSet::from([(peer("a"), topic("t1"))]));
    }

    // Empty view → empty expected set (no membership, or no candidates).
    #[test]
    fn empty_view_expects_nothing() {
        assert!(ConnectToAllCandidates
            .expected_upstream(&BTreeSet::new(), &BTreeMap::new(), 0)
            .is_empty());
        assert!(ConnectToAllCandidates
            .expected_upstream(&subscriptions(&["t1"]), &BTreeMap::new(), 0)
            .is_empty());
    }

    // Self-exclusion is input-borne: the policy passes through whatever the
    // candidate sets contain, so a self-excluded input yields a self-excluded
    // expected set.
    #[test]
    fn self_exclusion_is_input_borne() {
        // The real fold never inserts self; modelling that, "self" is absent
        // from the candidate input and therefore absent from the output.
        let expected = ConnectToAllCandidates.expected_upstream(
            &subscriptions(&["t1"]),
            &candidates(&[("t1", &["a", "b"])]),
            0,
        );
        assert!(!expected.contains(&(peer("self"), topic("t1"))));
        assert_eq!(expected.len(), 2);
    }
}

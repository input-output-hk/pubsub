//! The v1 connection-selection policy: [`ConnectToAllCandidates`].

use std::collections::BTreeSet;

use super::ConnectionStrategy;
use crate::peer::PeerId;
use crate::strategies::view::NodeView;
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
    fn expected_relay(&self, view: &NodeView<'_>) -> BTreeSet<(PeerId, TopicId)> {
        let mut expected = BTreeSet::new();
        for topic in view.subscriptions {
            if let Some(peers) = view.candidates.get(topic) {
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
    use crate::connection_state::Links;
    use crate::strategies::connection::ConnectionStrategy;
    use crate::strategies::test_support::{candidates, peer, subscriptions, topic, view};
    use std::collections::{BTreeMap, BTreeSet};

    // 005 FR-010: v1 policy expects every candidate on every joined topic.
    #[test]
    fn expects_every_candidate_across_joined_topics() {
        let subs = subscriptions(&["t1", "t2"]);
        let cands = candidates(&[("t1", &["a", "b"]), ("t2", &["c"])]);
        let down = Links::new();
        let expected = ConnectToAllCandidates.expected_relay(&view(&subs, &cands, &down));
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
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a"]), ("t2", &["b"])]);
        let down = Links::new();
        let expected = ConnectToAllCandidates.expected_relay(&view(&subs, &cands, &down));
        assert_eq!(expected, BTreeSet::from([(peer("a"), topic("t1"))]));
    }

    // Empty view → empty expected set (no membership, or no candidates).
    #[test]
    fn empty_view_expects_nothing() {
        let empty_subs = BTreeSet::new();
        let empty_cands = BTreeMap::new();
        let down = Links::new();
        assert!(ConnectToAllCandidates
            .expected_relay(&view(&empty_subs, &empty_cands, &down))
            .is_empty());
        let subs = subscriptions(&["t1"]);
        assert!(ConnectToAllCandidates
            .expected_relay(&view(&subs, &empty_cands, &down))
            .is_empty());
    }

    // Self-exclusion is input-borne: the policy passes through whatever the
    // candidate sets contain, so a self-excluded input yields a self-excluded set.
    #[test]
    fn self_exclusion_is_input_borne() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a", "b"])]);
        let down = Links::new();
        let expected = ConnectToAllCandidates.expected_relay(&view(&subs, &cands, &down));
        assert!(!expected.contains(&(peer("self"), topic("t1"))));
        assert_eq!(expected.len(), 2);
    }
}

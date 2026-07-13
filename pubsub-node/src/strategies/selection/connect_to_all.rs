//! The full-mesh selection policy: [`ConnectToAllCandidates`].

use std::collections::BTreeSet;

use super::LinkSelectionStrategy;
use crate::peer::PeerId;
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

/// Select **every** candidate on **every** topic the node is a member of.
///
/// Self-exclusion is input-borne — the candidate sets the node folds from the
/// subscription registry never contain its own id, so the expected set never
/// does either. On the relay slot this maintains the full per-topic mesh (the
/// v1 default); degree limits and the verifiable gate are
/// [`HashGatedSelection`](super::HashGatedSelection)'s job.
pub struct ConnectToAllCandidates;

impl LinkSelectionStrategy for ConnectToAllCandidates {
    fn expected_links(&self, view: &NodeView<'_>) -> BTreeSet<(PeerId, TopicId)> {
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
    use crate::strategies::selection::LinkSelectionStrategy;
    use crate::strategies::test_support::{
        candidates, downstream, peer, subscriptions, topic, view,
    };
    use std::collections::BTreeSet;

    // 005 FR-010: the full-mesh policy expects every candidate on every joined topic.
    #[test]
    fn expects_every_candidate_across_joined_topics() {
        let subs = subscriptions(&["t1", "t2"]);
        let cands = candidates(&[("t1", &["a", "b"]), ("t2", &["c"])]);
        let store = downstream(&[]);
        let expected = ConnectToAllCandidates.expected_links(&view(&subs, &cands, &store));
        assert_eq!(
            expected,
            BTreeSet::from([
                (peer("a"), topic("t1")),
                (peer("b"), topic("t1")),
                (peer("c"), topic("t2")),
            ]),
        );
    }

    // A topic the node is not a member of contributes nothing, even with candidates.
    #[test]
    fn ignores_topics_the_node_is_not_a_member_of() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a"]), ("t2", &["b"])]);
        let store = downstream(&[]);
        let expected = ConnectToAllCandidates.expected_links(&view(&subs, &cands, &store));
        assert_eq!(expected, BTreeSet::from([(peer("a"), topic("t1"))]));
    }

    // No candidates → empty expectation (a single-node topic dials nobody).
    #[test]
    fn empty_candidates_expect_nothing() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[]);
        let store = downstream(&[]);
        assert!(ConnectToAllCandidates
            .expected_links(&view(&subs, &cands, &store))
            .is_empty());
    }
}

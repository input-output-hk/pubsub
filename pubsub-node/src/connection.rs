//! The connection domain: the upstream-state enum and the connection-selection
//! strategy seam.
//!
//! A node holds logical, per-`(peer, topic)` connections in two roles —
//! upstream (requested; message sources, with an explicit
//! [`UpstreamState`]) and downstream (accepted; fan-out destinations). The
//! connection structures themselves live on the crate-internal node state
//! (`crate::state`); this module owns the vocabulary that names them and the
//! [`ConnectionStrategy`] trait the node consults to decide which upstreams it
//! expects to hold.
//!
//! The types here are inert: they describe connections without establishing
//! any. The transition arms that produce connection effects arrive with the
//! user stories (see `specs/004-connections/tasks.md`).

use std::collections::{HashMap, HashSet};

use crate::peer::PeerId;
use crate::topic::TopicId;

/// The state of an upstream (dialer-side) connection for one `(peer, topic)`.
///
/// An upstream entry is created by the node's own strategy on a setup event in
/// [`AwaitingAccept`](UpstreamState::AwaitingAccept); it advances to
/// [`Active`](UpstreamState::Active) when the peer's `Accepted` arrives.
/// Terminal outcomes are removals, not stored states — there is no
/// closing/rejected variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum UpstreamState {
    /// A `Request` has been sent; the peer's `Accepted` has not yet arrived.
    /// Admits no payload.
    AwaitingAccept,
    /// The peer accepted; payload it forwards on this topic is admitted.
    Active,
}

/// The connection-selection policy a node consults on a setup event.
///
/// `expected_upstream` is **pure and synchronous**: given the node's current
/// view (the topics it is a member of and the per-topic candidate peers it has
/// discovered), it returns the set of upstream `(peer, topic)` connections the
/// node should hold. The node applies the result as a diff — it dials every
/// expected pair it does not already hold `Active`, and never removes an entry
/// on the strength of the strategy alone (selection only adds).
///
/// The trait is the seam future iterations vary (peer sampling, degree caps,
/// topology policies — ROADMAP 006/007); the v1 implementor is
/// [`ConnectToAllCandidates`].
pub trait ConnectionStrategy: Send + Sync {
    /// The expected upstream set given the node's view.
    ///
    /// `subscriptions` is the node's **membership-derived** topic set (the
    /// topics it has joined), not the registration-gated effective filter —
    /// the dial side mirrors the acceptance rule, where topic registration
    /// gates delivery rather than establishment. `candidates` maps each topic
    /// to the peers discovered on it (the node's own id is never present).
    fn expected_upstream(
        &self,
        subscriptions: &HashSet<TopicId>,
        candidates: &HashMap<TopicId, HashSet<PeerId>>,
    ) -> HashSet<(PeerId, TopicId)>;
}

/// The v1 connection-selection policy: connect to **every** candidate on
/// **every** topic the node is a member of.
///
/// Self-exclusion is input-borne — the candidate sets the node folds from the
/// subscription registry never contain its own id, so the expected set never
/// does either. This policy maintains the full per-topic mesh; degree limits
/// and sampling are deferred to later strategies.
pub struct ConnectToAllCandidates;

impl ConnectionStrategy for ConnectToAllCandidates {
    fn expected_upstream(
        &self,
        subscriptions: &HashSet<TopicId>,
        candidates: &HashMap<TopicId, HashSet<PeerId>>,
    ) -> HashSet<(PeerId, TopicId)> {
        let mut expected = HashSet::new();
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
    use super::{ConnectToAllCandidates, ConnectionStrategy};
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

    // FR-006..009: v1 policy expects every candidate on every joined topic.
    #[test]
    fn expects_every_candidate_across_joined_topics() {
        let expected = ConnectToAllCandidates.expected_upstream(
            &subscriptions(&["t1", "t2"]),
            &candidates(&[("t1", &["a", "b"]), ("t2", &["c"])]),
        );
        assert_eq!(
            expected,
            HashSet::from([
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
        );
        assert_eq!(expected, HashSet::from([(peer("a"), topic("t1"))]));
    }

    // Empty view → empty expected set (no membership, or no candidates).
    #[test]
    fn empty_view_expects_nothing() {
        assert!(ConnectToAllCandidates
            .expected_upstream(&HashSet::new(), &HashMap::new())
            .is_empty());
        assert!(ConnectToAllCandidates
            .expected_upstream(&subscriptions(&["t1"]), &HashMap::new())
            .is_empty());
    }

    // Self-exclusion is input-borne: the policy passes through whatever the
    // candidate sets contain, so a self-excluded input yields a self-excluded
    // expected set.
    #[test]
    fn self_exclusion_is_input_borne() {
        // The real fold never inserts self; modelling that, "self" is absent
        // from the candidate input and therefore absent from the output.
        let expected = ConnectToAllCandidates
            .expected_upstream(&subscriptions(&["t1"]), &candidates(&[("t1", &["a", "b"])]));
        assert!(!expected.contains(&(peer("self"), topic("t1"))));
        assert_eq!(expected.len(), 2);
    }
}

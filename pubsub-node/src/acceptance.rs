//! The connection-acceptance domain: the inbound-acceptance decision seam.
//!
//! When a node receives a verified connection `Request`, it must decide whether
//! to accept the peer as a downstream fan-out destination on the requested
//! topic. That decision is made by an injected [`ConnectionAcceptanceStrategy`],
//! the inbound mirror of the dial side's `ConnectionStrategy` (same purity, same
//! `Arc<dyn>`-at-storage shape, same "the trait is the variation point future
//! strategies replace" intent). The handler keeps the mechanics — the drop-log,
//! the idempotent downstream insert, the signed `Accepted` reply — and consults
//! this seam only for the accept/reject *policy*.
//!
//! The v1 implementor is [`AcceptFromAllCandidates`] — accept every membership-
//! valid request, the exact inbound mirror of `ConnectToAllCandidates` (whose
//! "all" is likewise membership-scoped). Discretionary policies — degree caps,
//! allowlists, rate limits — are deferred to later strategies (ROADMAP 006/007);
//! they slot in behind this trait. Registration gates delivery, not acceptance
//! (the S7 pin), so this seam reads the membership-derived view only.

use std::collections::{HashMap, HashSet};

use crate::peer::PeerId;
use crate::topic::TopicId;

/// The inbound connection-acceptance policy a node consults on a verified
/// `Request`.
///
/// `accepts` is **pure and synchronous**: given the requesting `emitter`, the
/// requested `topic`, the node's membership-derived `subscriptions` (the topics
/// it has joined) and per-topic `candidates` (the peers it has discovered, its
/// own id never present), it returns whether the request should be accepted.
///
/// `subscriptions`/`candidates` are the **membership-derived** view, not the
/// registration-gated effective filter — the accept side mirrors the dial side,
/// where topic registration gates delivery rather than establishment (the S7
/// pin). Taking the whole `candidates` map plus `subscriptions` keeps the
/// strategy free to implement degree caps or allowlists later without a
/// signature change — the seam future iterations vary (ROADMAP 006/007). The v1
/// implementor is [`AcceptFromAllCandidates`].
pub trait ConnectionAcceptanceStrategy: Send + Sync {
    /// Whether to accept a verified `Request` from `emitter` on `topic`.
    ///
    /// `subscriptions` is the node's membership-derived topic set; `candidates`
    /// maps each topic to the peers discovered on it (self never present).
    fn accepts(
        &self,
        emitter: &PeerId,
        topic: &TopicId,
        subscriptions: &HashSet<TopicId>,
        candidates: &HashMap<TopicId, HashSet<PeerId>>,
    ) -> bool;
}

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
    use super::{AcceptFromAllCandidates, ConnectionAcceptanceStrategy};
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

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
//! The trait lives here; each concrete policy is its own submodule. The v1
//! implementor is [`AcceptFromAllCandidates`] in [`accept_from_all`] — accept
//! every membership-valid request, the exact inbound mirror of
//! `ConnectToAllCandidates`. Registration gates delivery, not acceptance (the S7
//! pin), so this seam reads the membership-derived view only.

use std::collections::{HashMap, HashSet};

use crate::peer::PeerId;
use crate::topic::TopicId;

mod accept_from_all;
mod bounded;
mod kind;

pub use accept_from_all::AcceptFromAllCandidates;
pub use bounded::BoundedAcceptance;
pub use kind::{AcceptanceStrategyKind, UnknownAcceptanceStrategy};

/// The outcome of an acceptance decision on a verified connection `Request`
/// (feature 005, ADR 0025).
///
/// Replaces the earlier bare `bool` so the handler can distinguish a membership
/// failure (a silent drop — does not leak membership to non-members) from an
/// over-capacity refusal (which sends an explicit `Rejected` so the dialer can
/// back-fill).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Admission {
    /// Accept the emitter as a downstream on the topic (record it; reply `Accepted`).
    Accept,
    /// Refuse: the request is not membership-valid. A silent drop, no reply.
    RejectMembership,
    /// Refuse: the node is at its inbound bound on the topic. Dropped with a
    /// distinct cause and an explicit `Rejected` reply (not misbehaviour).
    RejectOverCapacity,
}

/// The inbound connection-acceptance policy a node consults on a verified
/// `Request`.
///
/// `admit` is **pure and synchronous**: given the requesting `emitter`, the
/// requested `topic`, the node's membership-derived `subscriptions` and per-topic
/// `candidates`, and its current `downstream` set, it returns an [`Admission`].
///
/// `subscriptions`/`candidates` are the **membership-derived** view, not the
/// registration-gated effective filter — the accept side mirrors the dial side,
/// where topic registration gates delivery rather than establishment (the S7
/// pin). The v1 implementor is [`AcceptFromAllCandidates`]; the bounded policy is
/// [`BoundedAcceptance`].
pub trait ConnectionAcceptanceStrategy: Send + Sync {
    /// The admission decision for a verified `Request` from `emitter` on `topic`.
    ///
    /// `subscriptions` is the node's membership-derived topic set; `candidates`
    /// maps each topic to the peers discovered on it (self never present);
    /// `downstream` is the node's current accepted-inbound set, so a policy can
    /// enforce an downstream degree bound.
    fn admit(
        &self,
        emitter: &PeerId,
        topic: &TopicId,
        subscriptions: &HashSet<TopicId>,
        candidates: &HashMap<TopicId, HashSet<PeerId>>,
        downstream: &HashSet<(PeerId, TopicId)>,
    ) -> Admission;
}

/// Whether a verified `Request` from `emitter` on `topic` is membership-valid:
/// the node is subscribed to the topic **and** the emitter is a known candidate
/// (member) of it. Shared by the acceptance policies (the S7 pin: membership
/// gates *acceptance*).
///
/// This is a **subscription-registry** check (topic membership), distinct from
/// **publisher authorization** — the topic-registry concern checked on the
/// dissemination path (`topic_registry::TopicEntry::is_publisher_authorized`).
/// The emitter here is a topic member/subscriber, not a publisher.
pub(crate) fn is_membership_valid(
    emitter: &PeerId,
    topic: &TopicId,
    subscriptions: &HashSet<TopicId>,
    candidates: &HashMap<TopicId, HashSet<PeerId>>,
) -> bool {
    let is_subscribed = subscriptions.contains(topic);
    let is_candidate = candidates
        .get(topic)
        .is_some_and(|peers| peers.contains(emitter));
    is_subscribed && is_candidate
}

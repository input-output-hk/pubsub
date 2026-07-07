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
//! The trait lives here; each concrete policy is its own submodule — the four
//! one-dimensional baselines of the empirical approach (ADR 0031):
//! [`AcceptFromAllCandidates`] (membership only), [`BoundedAcceptance`] (cap
//! only), [`HashGatedAcceptance`] (edge predicate only), and
//! [`HashGatedBoundedAcceptance`] (both — the bucketed-pull compound).
//! Registration gates delivery, not acceptance (the S7 pin), so this seam reads
//! the membership-derived view only.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::peer::PeerId;
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

mod accept_from_all;
mod bounded;
mod hash_gated;
mod hash_gated_bounded;
mod kind;

pub use accept_from_all::AcceptFromAllCandidates;
pub use bounded::BoundedAcceptance;
pub use hash_gated::HashGatedAcceptance;
pub use hash_gated_bounded::HashGatedBoundedAcceptance;
pub use kind::{AcceptanceStrategyKind, UnknownAcceptanceStrategy};

/// The outcome of an acceptance decision on a verified connection `Request`
/// (feature 005, ADR 0025).
///
/// Replaces the earlier bare `bool` so the handler can distinguish the
/// **silent-drop** refusals (membership-invalid, or the verifiable edge predicate
/// failing this interval — neither leaks anything to the requester) from an
/// **over-capacity** refusal (which sends an explicit `Rejected` so the dialer
/// drops its pending upstream).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Admission {
    /// Accept the emitter as a downstream on the topic (record it; reply `Accepted`).
    Accept,
    /// Refuse: the request is not membership-valid (topic unregistered, or the
    /// emitter is not a member). A silent drop, no reply.
    RejectMembership,
    /// Refuse: the verifiable edge predicate does not hold for this interval —
    /// the request is illegitimate (an adversary cannot force an edge the hash
    /// does not allow). A silent drop, no reply (ADR 0024/0025).
    RejectIllegitimate,
    /// Refuse: the node is at its per-topic downstream cap. Dropped with a
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
/// pin). The implementors are the four one-dimensional baselines:
/// [`AcceptFromAllCandidates`], [`BoundedAcceptance`], [`HashGatedAcceptance`],
/// and the compound [`HashGatedBoundedAcceptance`].
pub trait ConnectionAcceptanceStrategy: Send + Sync {
    /// The admission decision for a verified `Request` from `emitter` on `topic`,
    /// given the node's read-only [`NodeView`].
    ///
    /// `emitter`/`topic` are the request; the view supplies the node state a
    /// policy reads — `subscriptions`/`candidates` (membership), `downstream`
    /// (the current inbound set, to count the cap), and `epoch_nonce` (to
    /// recompute the verifiable edge predicate).
    fn admit(&self, emitter: &PeerId, topic: &TopicId, view: &NodeView<'_>) -> Admission;
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
    subscriptions: &BTreeSet<TopicId>,
    candidates: &BTreeMap<TopicId, BTreeSet<PeerId>>,
) -> bool {
    let is_subscribed = subscriptions.contains(topic);
    let is_candidate = candidates
        .get(topic)
        .is_some_and(|peers| peers.contains(emitter));
    is_subscribed && is_candidate
}

/// One pass over the downstream set for the two facts a bounding policy needs:
/// whether `(emitter, topic)` is already an accepted downstream, and how many
/// downstream entries the topic holds. Computed borrow-only (a
/// `HashSet<(PeerId, TopicId)>` lookup would need an owned tuple, i.e. two
/// clones per inbound request) and without a second scan for the cap count.
pub(crate) fn downstream_scan(
    downstream: &HashSet<(PeerId, TopicId)>,
    emitter: &PeerId,
    topic: &TopicId,
) -> (bool, usize) {
    let mut already_downstream = false;
    let mut on_topic = 0;
    for (peer, t) in downstream {
        if t == topic {
            on_topic += 1;
            if peer == emitter {
                already_downstream = true;
            }
        }
    }
    (already_downstream, on_topic)
}

/// The shared refusing-policy prelude, run before any policy-specific check:
/// membership validation, then the idempotent already-downstream re-Accept.
///
/// `Err` is the early decision (`RejectMembership`, or `Accept` for a re-dial of
/// an already-held downstream — ahead of any gate or cap, so a lost/late
/// `Accepted` repairs the link instead of stranding it half-open, 005 FR-013);
/// `Ok(downstream_on_topic)` means "no early decision" and carries the topic's
/// downstream count for a cap check, from the same single scan.
///
/// Every refusing policy calls this first — the invariant lives here once, so a
/// new bounding/gating strategy cannot forget it. (`AcceptFromAllCandidates`
/// needs no prelude: it never refuses a member, so the re-Accept is implied.)
pub(crate) fn admit_prelude(
    emitter: &PeerId,
    topic: &TopicId,
    view: &NodeView<'_>,
) -> Result<usize, Admission> {
    if !is_membership_valid(emitter, topic, view.subscriptions, view.candidates) {
        return Err(Admission::RejectMembership);
    }
    let (already_downstream, downstream_on_topic) =
        downstream_scan(view.downstream, emitter, topic);
    if already_downstream {
        return Err(Admission::Accept);
    }
    Ok(downstream_on_topic)
}

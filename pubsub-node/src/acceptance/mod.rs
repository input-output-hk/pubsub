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

pub use accept_from_all::AcceptFromAllCandidates;

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
/// pin). The v1 implementor is [`AcceptFromAllCandidates`].
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

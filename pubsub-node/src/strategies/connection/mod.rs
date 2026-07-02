//! The connection-selection strategy seam.
//!
//! A node holds logical, per-`(peer, topic)` upstream connections (message
//! sources it dials). This module owns the [`ConnectionStrategy`] trait the node
//! consults to decide which upstreams it expects to hold; the upstream-state
//! vocabulary itself is core domain state in [`crate::connection_state`].
//!
//! The trait lives here; each concrete selection policy is its own submodule:
//! [`ConnectToAllCandidates`] (the v1 full-mesh policy) in [`connect_to_all`],
//! and [`SeededBoundedConnection`] (the seeded, bounded policy, feature 005) in
//! [`seeded_bounded`].

use std::collections::{BTreeMap, BTreeSet};

use crate::peer::PeerId;
use crate::topic::TopicId;

mod connect_to_all;
mod kind;
mod seeded_bounded;

pub use connect_to_all::ConnectToAllCandidates;
pub use kind::{ConnectionStrategyKind, UnknownConnectionStrategy};
pub use seeded_bounded::SeededBoundedConnection;

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
/// topology policies); the v1 implementor is [`ConnectToAllCandidates`], and the
/// seeded, bounded policy is [`SeededBoundedConnection`].
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
        subscriptions: &BTreeSet<TopicId>,
        candidates: &BTreeMap<TopicId, BTreeSet<PeerId>>,
    ) -> BTreeSet<(PeerId, TopicId)>;
}

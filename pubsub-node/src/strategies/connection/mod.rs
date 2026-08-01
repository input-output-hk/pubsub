//! The connection-selection strategy seam.
//!
//! A node holds logical, per-`(peer, topic)` upstream connections (message
//! sources it dials). This module owns the [`ConnectionStrategy`] trait the node
//! consults to decide which upstreams it expects to hold; the upstream-state
//! vocabulary itself is core domain state in [`crate::connection_state`].
//!
//! The trait lives here; each concrete selection policy is its own submodule:
//! [`ConnectToAllCandidates`] (the v1 full-mesh policy) in [`connect_to_all`],
//! and [`HashGatedConnection`] (the verifiable hash-gated policy, feature 005) in
//! [`hash_gated`].

use std::collections::BTreeSet;

use crate::peer::PeerId;
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

mod connect_to_all;
mod hash_gated;
mod kind;
mod none;
mod selection;

pub use connect_to_all::ConnectToAllCandidates;
pub use hash_gated::HashGatedConnection;
pub use kind::{ConnectionStrategyKind, UnknownConnectionStrategy};
pub use none::DialNone;
pub use selection::Selection;

/// The link-selection policy a node consults on a dial event.
///
/// `expected_links` is **pure and synchronous**: given the node's current
/// view (the topics it is a member of and the per-topic candidate peers it has
/// discovered), it returns the set of `(peer, topic)` links the node should
/// have dialed. Which direction the resulting links run is the *instance's*
/// role, not the trait's: the relay instance's picks are dialed as upstream
/// message sources; the publisher instance's picks as standing downstream
/// targets for the node's own publications. The node applies the result as a
/// diff — it dials every expected pair it does not already hold `Active`, and
/// never removes an entry on the strength of the strategy alone (selection
/// only adds).
///
/// The trait is the seam future iterations vary (peer sampling, degree caps,
/// topology policies); the v1 implementor is [`ConnectToAllCandidates`], and the
/// verifiable hash-gated policy is [`HashGatedConnection`].
pub trait ConnectionStrategy: Send + Sync {
    /// The expected dialed-link set given the node's read-only [`NodeView`].
    ///
    /// Reads `view.subscriptions` (the membership-derived topic set — not the
    /// registration-gated effective filter, mirroring the acceptance rule),
    /// the per-topic candidate accessors ([`NodeView::candidates_for`] /
    /// [`NodeView::candidates_len`] — the node's own id excluded at read
    /// time), and `view.epoch_nonce` (the randomness context for the edge
    /// predicate).
    fn expected_links(&self, view: &NodeView<'_>) -> BTreeSet<(PeerId, TopicId)>;
}

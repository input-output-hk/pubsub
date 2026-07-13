//! The publishing-link domain: the publish-target selection seam (feature 015,
//! ADR 0033).
//!
//! A **publishing link** (the M3 S-link) carries only the dialing publisher's
//! own locally-originated messages into the overlay — never relayed traffic.
//! This module owns the [`PublishStrategy`] trait the node consults on the
//! `Heartbeat` dial tick, after the relay dial diff: it returns the
//! `(peer, topic)` publishing targets the node should hold `Out`/`Publisher`
//! links to. The M3 **trigger** lives inside the strategy: targets are selected
//! only for topics where the node has no expected relay downstream (no
//! candidate would select it as an upstream under the current epoch nonce), so
//! a well-connected node forms no publishing links and injection reach stays
//! decoupled from the relay degree.
//!
//! The trait lives here; each concrete policy is its own submodule:
//! [`NoPublishLinks`] (the default — no publishing links, the
//! behaviour-preserving configuration) in [`none`], and [`HashGatedPublish`]
//! (the verifiable hash-gated policy over the publish edge domain) in
//! [`hash_gated`].

use std::collections::BTreeSet;

use crate::peer::PeerId;
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

mod hash_gated;
mod kind;
mod none;

pub use hash_gated::HashGatedPublish;
pub use kind::{PublishStrategyKind, UnknownPublishStrategy};
pub use none::NoPublishLinks;

/// The publish-target selection policy a node consults on a dial tick.
///
/// `expected_publish` is **pure and synchronous**: given the node's read-only
/// [`NodeView`], it returns the set of `(peer, topic)` publishing targets the
/// node should hold. The node applies the result as a diff exactly like the
/// relay side — it dials every expected pair it does not already hold, and
/// never removes an entry on the strength of the strategy alone. Returning an
/// **empty set for a topic with expected relay downstream** is part of the
/// contract (the M3 trigger, ADR 0033): publishing links exist to give a
/// publisher with no relay path an injection route, not to widen reach.
pub trait PublishStrategy: Send + Sync {
    /// The expected `Out`/`Publisher` link set given the node's [`NodeView`].
    fn expected_publish(&self, view: &NodeView<'_>) -> BTreeSet<(PeerId, TopicId)>;
}

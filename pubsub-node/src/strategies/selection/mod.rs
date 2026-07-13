//! The link-selection domain: the one dial-side seam both link roles use
//! (feature 015, ADR 0034 — supersedes the separate connection/publish seams).
//!
//! A node holds outbound links per role — `Relay`/Out picks are its pull
//! forwarders (the M2/M3 `RF` picks), `Publisher`/Out picks its standing
//! initiation links (the M3 `s−1` targets, always established:
//! `formal_spec/hybrid_dissemination/models/m3/README.md`). Which peers to
//! pick is the same *shape* of decision for both, so one trait serves both
//! slots; the transition tags the resulting links with the slot's role. What
//! a link **carries** is not the selection seam's business — that is the
//! origin-aware fan-out seam (the model knob: M3 partitions by role, M4/M5
//! union — ADR 0034).
//!
//! The trait lives here; each concrete policy is its own submodule:
//! [`NoLinks`] (select nothing — the publish-slot default, and an accept-only
//! node on the relay slot), [`ConnectToAllCandidates`] (every candidate on
//! every joined topic), and [`HashGatedSelection`] (the verifiable bucketed
//! policy under the slot's role domain).

use std::collections::BTreeSet;

use crate::peer::PeerId;
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

mod connect_to_all;
mod hash_gated;
mod kind;
mod none;

pub use connect_to_all::ConnectToAllCandidates;
pub use hash_gated::HashGatedSelection;
pub use kind::{LinkSelectionKind, UnknownLinkSelection};
pub use none::NoLinks;

/// The link-selection policy a node consults on a dial tick, one instance per
/// role slot (relay / publish).
///
/// `expected_links` is **pure and synchronous**: given the node's read-only
/// [`NodeView`], it returns the set of outbound `(peer, topic)` links its slot
/// should hold. The node applies the result as a diff — it dials every
/// expected pair it does not already hold, tagged with the slot's role, and
/// never removes an entry on the strength of the strategy alone (selection
/// only adds).
pub trait LinkSelectionStrategy: Send + Sync {
    /// The expected outbound link set for this slot given the [`NodeView`].
    fn expected_links(&self, view: &NodeView<'_>) -> BTreeSet<(PeerId, TopicId)>;
}

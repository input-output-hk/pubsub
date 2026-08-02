//! The fan-out domain: the forwarding-target selection seam.
//!
//! When a node records a dissemination message — one it published or one it
//! received — it forwards that message to its downstream peers on the message's
//! topic. The set of forwarding targets is chosen by an injected
//! [`FanoutStrategy`], the deliberate twin of the connection side's
//! `ConnectionStrategy` (same purity, same `Arc<dyn>`-at-storage shape, same
//! "the trait is the variation point future strategies replace" intent).
//!
//! The trait lives here; each concrete policy is its own submodule.
//! [`ForwardToAll`] in [`forward_to_all`] (the default) forwards every held
//! message over every `Active` downstream link, either kind; [`ForwardToRelays`]
//! in [`forward_to_relays`] is the M3-exclusivity policy — held messages ride
//! relay links only, publisher links carry just the node's own publications.
//! Both apply the split-horizon exclusion and dedup per peer.

use std::collections::BTreeMap;

use crate::connection_state::{LinkKey, LinkState};
use crate::peer::PeerId;
use crate::received::Origin;
use crate::topic::TopicId;

mod forward_to_all;
mod forward_to_relays;
mod kind;

pub use forward_to_all::ForwardToAll;
pub use forward_to_relays::ForwardToRelays;
pub use kind::{FanoutStrategyKind, UnknownFanoutStrategy};

/// The forwarding-target policy a node consults at the record point.
///
/// `targets` is **pure and synchronous**: given the message's `topic`, the
/// node's full `downstream` link map (accepted relay destinations and the
/// node's own publisher dials, distinguished by each key's kind), the recorded
/// message's `origin`, and an optional `exclude` peer, it returns the peers
/// that should receive a forward of the message, **deduplicated per peer** —
/// a peer reachable over both link kinds is returned once.
///
/// `origin` is what lets a policy treat the node's own publications differently
/// from relayed traffic ([`ForwardToRelays`] sends only local-origin messages
/// over publisher links). `exclude` is the split-horizon exclusion: on the
/// **receive** path it is the delivering peer (a node never echoes a message
/// back to the peer it received it from); on the **publish** path it is `None`
/// (a locally-originated message has no delivering peer).
pub trait FanoutStrategy: Send + Sync {
    /// The downstream peers that receive a forward of a message on `topic`.
    ///
    /// `downstream` is the node's complete downstream link map; the strategy
    /// scopes to `topic` and selects link kinds itself. `exclude`, when
    /// present, is the one peer to omit (split-horizon). Target *order* is
    /// unspecified; each peer appears at most once.
    fn targets(
        &self,
        topic: &TopicId,
        downstream: &BTreeMap<LinkKey, LinkState>,
        origin: &Origin,
        exclude: Option<&PeerId>,
    ) -> Vec<PeerId>;
}

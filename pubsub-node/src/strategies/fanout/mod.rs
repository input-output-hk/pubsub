//! The fan-out domain: the forwarding-target selection seam.
//!
//! When a node records a dissemination message — one it published or one it
//! received — it forwards that message to its downstream peers on the message's
//! topic. The set of forwarding targets is chosen by an injected
//! [`FanoutStrategy`], the deliberate twin of the connection side's
//! `ConnectionStrategy` (same purity, same `Arc<dyn>`-at-storage shape, same
//! "the trait is the variation point future strategies replace" intent).
//!
//! The trait lives here; each concrete policy is its own submodule. The v1
//! implementor is [`ForwardToAll`] in [`forward_to_all`] — forward to every
//! downstream peer on the topic, minus the split-horizon exclusion. Degree caps
//! and peer sampling are deferred to later strategies.

use std::collections::HashSet;

use crate::peer::PeerId;
use crate::topic::TopicId;

mod forward_to_all;

pub use forward_to_all::ForwardToAll;

/// The forwarding-target policy a node consults at the record point.
///
/// `targets` is **pure and synchronous**: given the message's `topic`, the
/// node's full `downstream` set (the `(peer, topic)` pairs it has accepted as
/// fan-out destinations), and an optional `exclude` peer, it returns the
/// downstream peers that should receive a forward of the message.
///
/// `exclude` is the split-horizon exclusion: on the **receive** path it is the
/// delivering peer (a node never echoes a message back to the peer it received
/// it from); on the **publish** path it is `None` (a locally-originated message
/// has no delivering peer).
///
/// Taking the whole `downstream` set plus `topic` plus `exclude` keeps the
/// strategy free to implement degree caps or sampling later without a signature
/// change. The v1 implementor is [`ForwardToAll`].
pub trait FanoutStrategy: Send + Sync {
    /// The downstream peers that receive a forward of a message on `topic`.
    ///
    /// `downstream` is the node's complete set of accepted `(peer, topic)`
    /// destinations; the strategy scopes to `topic` itself. `exclude`, when
    /// present, is the one peer to omit (split-horizon). Target *order* is
    /// unspecified.
    fn targets(
        &self,
        topic: &TopicId,
        downstream: &HashSet<(PeerId, TopicId)>,
        exclude: Option<&PeerId>,
    ) -> Vec<PeerId>;
}

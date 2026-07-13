//! The fan-out domain: the forwarding-target selection seam.
//!
//! When a node records a dissemination message — one it published or one it
//! received — it forwards that message to the appropriate links on the
//! message's topic. The set of forwarding targets is chosen by an injected
//! [`FanoutStrategy`], the deliberate twin of the connection side's
//! `ConnectionStrategy` (same purity, same `Arc<dyn>`-at-storage shape, same
//! "the trait is the variation point future strategies replace" intent).
//!
//! Since feature 015 the seam is **origin-aware** (ADR 0033): the strategy
//! receives the message [`Origin`] so publishing links can carry only the
//! node's own locally-originated messages while relaying links carry
//! everything.
//!
//! The trait lives here; each concrete policy is its own submodule. The v1
//! implementor is [`ForwardToAll`] in [`forward_to_all`] — forward to every
//! relay fan-out destination on the topic (minus the split-horizon exclusion),
//! plus every active outbound publishing link for a local origin. Degree caps
//! and peer sampling are deferred to later strategies.

use crate::connection_state::Links;
use crate::peer::PeerId;
use crate::received::Origin;
use crate::topic::TopicId;

mod forward_to_all;

pub use forward_to_all::ForwardToAll;

/// The forwarding-target policy a node consults at the record point.
///
/// `targets` is **pure and synchronous**: given the message's `topic`, the
/// node's full link store, the message's `origin`, and an optional `exclude`
/// peer, it returns the peers that should receive a forward of the message.
///
/// `origin` is what makes publishing links possible (ADR 0033): a
/// `Publisher`/`Out` link is a target **only** for [`Origin::Local`] messages
/// (the node's own publishes), while `Relay`/`In` links are targets for both
/// origins. `exclude` is the split-horizon exclusion: on the **receive** path
/// it is the delivering peer (a node never echoes a message back to the peer it
/// received it from); on the **publish** path it is `None` (a
/// locally-originated message has no delivering peer). The exclusion applies
/// regardless of link role.
///
/// Taking the whole link store plus `topic`/`origin`/`exclude` keeps the
/// strategy free to implement degree caps or sampling later without a signature
/// change. The v1 implementor is [`ForwardToAll`].
pub trait FanoutStrategy: Send + Sync {
    /// The peers that receive a forward of a message on `topic`.
    ///
    /// `links` is the node's complete link store; the strategy scopes to
    /// `topic` and the role × direction cells appropriate for `origin` itself.
    /// `exclude`, when present, is the one peer to omit (split-horizon).
    /// Target *order* is unspecified.
    fn targets(
        &self,
        topic: &TopicId,
        links: &Links,
        origin: &Origin,
        exclude: Option<&PeerId>,
    ) -> Vec<PeerId>;
}

//! The subscription registry — the node-membership "subscription list".
//!
//! A node's topics are defined by its own entry in the subscription list (the
//! source of truth), read via [`SubscriptionRegistry::entry`]. Membership of
//! the topics a node watches is delivered as a push stream via
//! [`SubscriptionRegistry::watch_members`]: the current members of the watched
//! topics replay first (a cold-start burst of [`MembershipEvent::Joined`]),
//! then live deltas follow.
//!
//! The read trait [`SubscriptionRegistry`] is what the node depends on. The
//! write side lives on a separate [`SubscriptionRegistryControl`] trait that
//! models the operator's registration actions — the node never calls it; only
//! the in-memory loader and test harnesses do.
//!
//! This is distinct from the (future) topic registry, which records topic
//! ownership and authorised publishers; the two share no trait.

use std::collections::BTreeSet;

use crate::peer::PeerId;
use crate::topic::TopicId;

mod in_memory;

pub use in_memory::InMemorySubscriptionRegistry;

/// A node's entry in the subscription list — the materialized record
/// [`SubscriptionRegistry::entry`] returns (as distinct from a
/// [`MembershipEvent`], which is a delta).
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionEntry {
    /// The registered node.
    pub node: PeerId,
    /// The topics the node subscribes to.
    pub topics: BTreeSet<TopicId>,
}

/// One membership delta delivered on a [`MembershipWatch`].
///
/// Carries identity and topics only — no network address (endpoints are
/// resolved off-registry) and no deposit/stake.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MembershipEvent {
    /// `node` is present in `topics` (a subset of the watched set). Emitted
    /// during the cold-start replay and for live joins.
    Joined {
        node: PeerId,
        topics: BTreeSet<TopicId>,
    },
    /// `node` changed its topics; `added`/`removed` are already intersected
    /// with the watched set.
    TopicsChanged {
        node: PeerId,
        added: BTreeSet<TopicId>,
        removed: BTreeSet<TopicId>,
    },
    /// `node` left the registry entirely.
    Left { node: PeerId },
}

/// Single-consumer membership stream handle. Mirrors `NetworkHandle`: it owns
/// the receive half, is not `Clone`, and ends its subscription when dropped.
pub struct MembershipWatch {
    rx: tokio::sync::mpsc::UnboundedReceiver<MembershipEvent>,
}

impl MembershipWatch {
    pub(crate) fn new(rx: tokio::sync::mpsc::UnboundedReceiver<MembershipEvent>) -> Self {
        Self { rx }
    }

    /// Receive the next membership event, or `None` once the registry (and all
    /// its senders) is dropped.
    pub async fn recv(&mut self) -> Option<MembershipEvent> {
        self.rx.recv().await
    }

    /// Non-blocking drain of the next currently-available event (test helper).
    #[cfg(test)]
    pub(crate) fn try_next(&mut self) -> Option<MembershipEvent> {
        self.rx.try_recv().ok()
    }
}

/// Typed error for the registry's fallible operations.
///
/// The in-memory implementation does not fail under normal operation; the
/// variant set is intentionally minimal and grows when the on-chain backend
/// (feature 012) introduces real failure modes. File-load failures surface
/// through [`ConfigError`](crate::ConfigError), not this enum.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum SubscriptionRegistryError {
    /// The backing registry was unavailable.
    #[error("subscription registry backend unavailable: {0}")]
    Backend(String),
}

/// Read-only, node-facing view of the subscription list.
///
/// This is the only trait the node depends on (held as
/// `Arc<dyn SubscriptionRegistry>`); the real on-chain reader (feature 012)
/// implements exactly this surface. The write surface is the separate
/// [`SubscriptionRegistryControl`].
#[allow(async_fn_in_trait)] // mirrors the `Network` trait's v1 allowance (ADR 0007)
pub trait SubscriptionRegistry: Send + Sync + 'static {
    /// Open a topic-scoped membership watch: replay current members of
    /// `topics` as a `Joined` cold-start burst, then stream live deltas. The
    /// burst and live deltas form one gap-free, duplicate-free sequence.
    async fn watch_members(
        &self,
        topics: BTreeSet<TopicId>,
    ) -> Result<MembershipWatch, SubscriptionRegistryError>;

    /// Look up a node's own subscription-list entry (`None` if not registered).
    /// A node calls `entry(self_id)` at startup to learn its authoritative
    /// topics.
    async fn entry(
        &self,
        node: PeerId,
    ) -> Result<Option<SubscriptionEntry>, SubscriptionRegistryError>;
}

/// The operator/test write surface, extending [`SubscriptionRegistry`].
///
/// Models the operator's registration transaction. The node never depends on
/// this trait; the in-memory loader and test harnesses drive the registry
/// through it.
#[allow(async_fn_in_trait)]
pub trait SubscriptionRegistryControl: SubscriptionRegistry {
    /// Declaratively set a node's topics (idempotent upsert). A first
    /// registration emits `Joined`; a changed set emits a single
    /// `TopicsChanged`; an unchanged set is a no-op.
    async fn set_topics(
        &self,
        node: PeerId,
        topics: BTreeSet<TopicId>,
    ) -> Result<(), SubscriptionRegistryError>;

    /// Remove a node's entry entirely; observers of its topics see `Left`.
    /// Distinct from `set_topics(node, {})` (which retains an empty entry).
    async fn unregister(&self, node: PeerId) -> Result<(), SubscriptionRegistryError>;
}

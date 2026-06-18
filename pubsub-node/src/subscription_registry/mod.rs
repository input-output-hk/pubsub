//! The subscription registry — the node-membership "subscription list".
//!
//! A node derives **all** of its registry state from a single node-keyed
//! snapshot-plus-live stream opened with [`SubscriptionRegistry::watch`]: the
//! current state is returned as a [`MembershipSnapshot`] — the node's own entry
//! (from which it derives its subscription set, the source of truth for its
//! topics) followed by the members of those topics (its candidate sets) — and
//! live deltas then follow on the returned [`MembershipWatch`].
//!
//! The read trait [`SubscriptionRegistry`] is what the node depends on, and is
//! deliberately just `watch` — no point-read method, since no consumer needs
//! one (the concrete [`InMemorySubscriptionRegistry`] offers an inherent
//! `entry` read-back for tooling/tests). The write side lives on a separate
//! [`SubscriptionRegistryControl`] trait that models the operator's
//! registration actions — the node never calls it; only the in-memory loader
//! and test harnesses do.
//!
//! This is distinct from the (future) topic registry, which records topic
//! ownership and authorised publishers; the two share no trait.

use std::collections::BTreeSet;

use crate::peer::PeerId;
use crate::topic::TopicId;

mod in_memory;
#[cfg(test)]
mod test_support;

pub use in_memory::InMemorySubscriptionRegistry;
#[cfg(test)]
pub(crate) use test_support::MembershipScript;

/// One membership delta delivered on a [`MembershipWatch`].
///
/// Carries identity and topics only — no network address (endpoints are
/// resolved off-registry) and no deposit/stake.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MembershipEvent {
    /// `node` joined `topics` (a subset of the watched set). A **live** join
    /// after the watch's snapshot (the snapshot carries existing members).
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

/// A point-in-time snapshot of the node-scoped membership, returned by
/// [`SubscriptionRegistry::watch`] ahead of the live delta stream: the node's
/// **own** entry first (its id and topics — its subscription set), then the
/// current members of those topics (its candidate sets), each as
/// `(node, topics)`. Folding the snapshot warms the node's subscription and
/// candidate view; the node distinguishes its own id (→ subscriptions) from
/// others (→ candidates).
pub type MembershipSnapshot = Vec<(PeerId, BTreeSet<TopicId>)>;

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
/// This is the only trait the node depends on. An `async fn`/RPITIT trait is
/// not `dyn`-compatible, so the node consumes it generically as `Arc<R>` (the
/// way `Network` is consumed under ADR 0007), not as a trait object; the real
/// on-chain reader (feature 012) is a second generic impl of exactly this
/// surface. The write surface is the separate [`SubscriptionRegistryControl`].
#[allow(async_fn_in_trait)] // mirrors the `Network` trait's v1 allowance (ADR 0007)
pub trait SubscriptionRegistry: Send + Sync + 'static {
    /// Open the node-keyed membership watch: returns a [`MembershipSnapshot`]
    /// scoped to `node`'s own subscription-list entry, plus a [`MembershipWatch`]
    /// streaming subsequent live deltas. The snapshot carries:
    /// - the node's **own** entry — `(node, topics)` — from which the node
    ///   derives its subscription set; then
    /// - the current **members** of those topics — `(other, topics)` (scoped to
    ///   the node's topics) — from which it derives candidate sets.
    ///
    /// Live deltas follow on the watch (members joining/leaving/changing within
    /// the node's topics, and changes to the node's own entry). The node folds
    /// the snapshot then the deltas from empty initial state, distinguishing its
    /// own id (→ subscriptions) from others (→ candidates). The snapshot and the
    /// live stream do not overlap (gap-free, duplicate-free).
    ///
    /// Returns a `Send` future (RPITIT, the `Send`-bounded shape ADR 0007 flags
    /// as the follow-up to `async fn` in traits) because the node-owned reader
    /// awaits it inside a spawned task.
    fn watch(
        &self,
        node: PeerId,
    ) -> impl std::future::Future<
        Output = Result<(MembershipSnapshot, MembershipWatch), SubscriptionRegistryError>,
    > + Send;
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

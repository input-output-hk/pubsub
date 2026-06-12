//! The topic registry — which topics legitimately exist and who may publish.
//!
//! A node derives its topic-registry state from a single **global** push stream
//! opened with [`TopicRegistry::watch`]: the current set of registered topics
//! replays first as a cold-start burst of [`TopicRegistryEvent::Registered`]
//! (each carrying the topic's authorized publisher keys — an empty set meaning
//! the topic is *open* to any publisher), then live deltas follow.
//!
//! The read trait [`TopicRegistry`] is what the node depends on, and is
//! deliberately just `watch` — no point-read, and (unlike the subscription
//! registry's node-keyed `watch(node)`) no scoping argument, since topic
//! legitimacy is a global fact the node folds in full. The write side lives on a
//! separate [`TopicRegistryControl`] trait that models the operator's
//! governance actions — the node never calls it; only the in-memory loader and
//! test harnesses do.
//!
//! This is distinct from the subscription registry (node membership); the two
//! share no trait — different keys, payloads, and readers (per
//! `../docs/node-lifecycle/README.md`). The node-facing projection here is
//! topic + authorized-publishers only; the on-chain contract's governance
//! fields (owners/admins/replication/retention) are feature 012's domain.

use std::collections::BTreeSet;

use crate::crypto::PublicKey;
use crate::topic::TopicId;

mod in_memory;

pub use in_memory::InMemoryTopicRegistry;

/// One topic-registry delta delivered on a [`TopicRegistryWatch`].
///
/// Carries a topic id and its authorized publisher keys only — no governance
/// fields (owners/admins/replication/retention are off-registry here). An empty
/// `publishers` set means the topic is **open** (any publisher accepted).
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TopicRegistryEvent {
    /// `topic` is a legitimately-registered topic; `publishers` are its
    /// authorized keys (empty ⇒ open). Emitted during the cold-start replay
    /// and for live registrations.
    Registered {
        topic: TopicId,
        publishers: BTreeSet<PublicKey>,
    },
    /// `topic`'s authorized-publisher set changed by the given diff.
    PublishersChanged {
        topic: TopicId,
        added: BTreeSet<PublicKey>,
        removed: BTreeSet<PublicKey>,
    },
    /// `topic` is no longer a registered topic.
    Removed { topic: TopicId },
}

/// Single-consumer topic-registry stream handle. Mirrors `MembershipWatch` /
/// `NetworkHandle`: it owns the receive half, is not `Clone`, and ends its
/// subscription when dropped.
pub struct TopicRegistryWatch {
    rx: tokio::sync::mpsc::UnboundedReceiver<TopicRegistryEvent>,
}

impl TopicRegistryWatch {
    pub(crate) fn new(rx: tokio::sync::mpsc::UnboundedReceiver<TopicRegistryEvent>) -> Self {
        Self { rx }
    }

    /// Receive the next topic-registry event, or `None` once the registry (and
    /// all its senders) is dropped.
    pub async fn recv(&mut self) -> Option<TopicRegistryEvent> {
        self.rx.recv().await
    }

    /// Non-blocking drain of the next currently-available event (test helper).
    #[cfg(test)]
    pub(crate) fn try_next(&mut self) -> Option<TopicRegistryEvent> {
        self.rx.try_recv().ok()
    }
}

/// Typed error for the topic registry's fallible operations.
///
/// The in-memory implementation does not fail under normal operation; the
/// variant set is intentionally minimal and grows when the on-chain backend
/// (feature 012) introduces real failure modes. File-load failures surface
/// through [`ConfigError`](crate::ConfigError), not this enum.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum TopicRegistryError {
    /// The backing registry was unavailable.
    #[error("topic registry backend unavailable: {0}")]
    Backend(String),
}

/// Read-only, node-facing view of the topic registry.
///
/// This is the only trait the node depends on. An `async fn`/RPITIT trait is
/// not `dyn`-compatible, so the node consumes it generically as `Arc<T>` (the
/// way `Network` and `SubscriptionRegistry` are), not as a trait object; the
/// real on-chain reader (feature 012) is a second generic impl of exactly this
/// surface. The write surface is the separate [`TopicRegistryControl`].
pub trait TopicRegistry: Send + Sync + 'static {
    /// Open the **global** topic-registry watch through which a node derives its
    /// topic-registry state from a single event stream.
    ///
    /// Unlike the subscription registry's node-keyed `watch(node)`, this takes
    /// no scoping argument: topic legitimacy is global, so the node folds the
    /// whole registry. On open it replays a cold-start burst of
    /// [`Registered`](TopicRegistryEvent::Registered) — one per currently-
    /// registered topic, carrying its authorized publishers — then streams live
    /// deltas (`Registered`/`PublishersChanged`/`Removed`). The burst and live
    /// deltas form one gap-free, duplicate-free sequence.
    ///
    /// Returns a `Send` future (RPITIT, the `Send`-bounded shape ADR 0007 flags
    /// as the follow-up to `async fn` in traits) because the node-owned reader
    /// awaits it inside a spawned task.
    fn watch(
        &self,
    ) -> impl std::future::Future<Output = Result<TopicRegistryWatch, TopicRegistryError>> + Send;
}

/// The operator/test write surface, extending [`TopicRegistry`].
///
/// Models the operator's governance transactions. The node never depends on
/// this trait; the in-memory loader and test harnesses drive the registry
/// through it. (Governance authorization — owner/admin gating — is the on-chain
/// contract's concern, deferred to feature 012; this mock surface is
/// permissionless.)
#[allow(async_fn_in_trait)]
pub trait TopicRegistryControl: TopicRegistry {
    /// Declaratively set a topic's authorized publishers (idempotent upsert). A
    /// first registration emits `Registered`; a changed publisher set emits a
    /// single `PublishersChanged`; an unchanged set is a no-op. An empty
    /// `publishers` set registers the topic **open**.
    async fn set_topic(
        &self,
        topic: TopicId,
        publishers: BTreeSet<PublicKey>,
    ) -> Result<(), TopicRegistryError>;

    /// Remove a topic's entry entirely; observers see `Removed`. Distinct from
    /// `set_topic(topic, {})` (which registers/retains the topic as open).
    async fn remove_topic(&self, topic: TopicId) -> Result<(), TopicRegistryError>;
}

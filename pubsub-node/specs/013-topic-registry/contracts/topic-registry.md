# Contract: Topic Registry + Node Surface Delta (013)

**Date**: 2026-06-11 | **Plan**: [../plan.md](../plan.md)

This feature **adds** public surface (the topic-registry module) and **changes** one existing public signature (`Node::new`). This file records the exact additions/changes so the post-implementation analyze pass can verify them against `src/lib.rs` and the module sources (constitution: spec fidelity is verified against code).

## A. New public surface — the topic-registry module

New module `src/topic_registry/`, re-exported from `lib.rs`:

```rust
pub use topic_registry::{
    InMemoryTopicRegistry, TopicRegistry, TopicRegistryControl, TopicRegistryError,
    TopicRegistryEvent, TopicRegistryWatch,
};
```

### Traits — read (node-facing) vs control (operator/test)

```rust
pub trait TopicRegistry: Send + Sync + 'static {  // read-only; what Node depends on; 012 implements this
    // The SINGLE, GLOBAL stream of topic-registry state (no scoping argument —
    // unlike SubscriptionRegistry::watch(node)). RPITIT with an explicit `Send`
    // bound: the node-owned reader awaits it in a spawned task.
    fn watch(&self)
        -> impl std::future::Future<Output = Result<TopicRegistryWatch, TopicRegistryError>> + Send;
}

#[allow(async_fn_in_trait)]
pub trait TopicRegistryControl: TopicRegistry {  // operator/test write surface; node never depends on it
    async fn set_topic(&self, topic: TopicId, publishers: BTreeSet<PublicKey>) -> Result<(), TopicRegistryError>;
    async fn remove_topic(&self, topic: TopicId) -> Result<(), TopicRegistryError>;
}
```

There is **no point-read** and the watch takes **no argument**: the node folds all registered topics from the global stream. `TopicRegistry` and `SubscriptionRegistry` (008) MUST NOT share a trait (distinct on-chain artifacts).

`Node` is constructed **generically** over the read trait — `Node::new<N: Network, R: SubscriptionRegistry, T: TopicRegistry>(…, topic_registry: Arc<T>)` — so it has no write methods in scope. (It is `Arc<T>`, not `Arc<dyn TopicRegistry>`: `async fn`/RPITIT traits aren't `dyn`-compatible.) `InMemoryTopicRegistry` implements **both** traits; tests/operator-sim hold the concrete `Arc<InMemoryTopicRegistry>`.

| Item | Shape | Contract |
|---|---|---|
| `TopicRegistryEvent` | `#[non_exhaustive] enum { Registered { topic, publishers }, PublishersChanged { topic, added, removed }, Removed { topic } }` | topic id + authorized publisher keys (`BTreeSet<PublicKey>`); empty publishers ⇒ open; no governance fields |
| `TopicRegistryWatch` | `struct` (not `Clone`); drain via `recv().await -> Option<TopicRegistryEvent>` | single-consumer; global cold-start `Registered` burst then live deltas; gap-free/duplicate-free; ends on drop |
| `TopicRegistryError` | `#[non_exhaustive] enum`; `Error + Debug + Display` | minimal now; grows with the on-chain backend (012) |
| `InMemoryTopicRegistry` | `pub struct`, private internals | `::new()`; `::from_file(path) -> Result<Self, ConfigError>`; shareable via `Arc` |

The on-chain decode/governance types (012) MUST remain module-internal and MUST NOT appear here (spec FR-003).

## B. Changed public surface — `Node`

| Method | Before (008 on `main`) | After (013) |
|---|---|---|
| `Node::new` | `async fn new<N: Network, R: SubscriptionRegistry>(PeerId, NodeConfig, Arc<N>, Arc<dyn Verifier>, Arc<R>) -> Result<Self, NodeError>` | `async fn new<N: Network, R: SubscriptionRegistry, T: TopicRegistry>(PeerId, NodeConfig, Arc<N>, Arc<dyn Verifier>, Arc<R>, Arc<T>) -> Result<Self, NodeError>` — **adds the topic registry generically** (`Arc<T>`, *not* `Arc<dyn>`); spawns a node-owned reader calling `watch()` — `registered_topics` converges as the cold-start burst drains. No fail-fast on an empty registry. |
| `Node::effective_subscriptions` | — | **new**: `fn effective_subscriptions(&self) -> Vec<TopicId>` — sync lock-and-clone snapshot of `subscriptions ∩ registered_topics` (the actual accept-filter) |
| `Node::subscriptions` | `fn subscriptions(&self) -> Vec<TopicId>` | **unchanged** — still the membership-derived (declared) set; distinct from `effective_subscriptions` |
| `Node::candidates` / `Node::peers` / others | — | unchanged (`send`, `id`, `events`, `spawn_producer`, `received_messages`, `Drop`) |

The node holds **no** write methods for either registry; both are read-only at the type level.

## C. Changed public surface — event & error & crypto

| Item | Change |
|---|---|
| `Event` (`event.rs`) | **add** `TopicRegistryUpdate(TopicRegistryEvent)` variant (enum stays `#[non_exhaustive]`). |
| `ConfigError` (`error.rs`) | **add** `DuplicateTopicEntry(String)` + `InvalidPublisherKey(String)` variants (topic-registry file load failures). `NodeError` **unchanged**. |
| `PublicKey` (`crypto/mod.rs`) | **add** `Ord, PartialOrd` to the derive list (additive; enables `BTreeSet<PublicKey>`). No behavioral change; existing `PublicKey` API byte-identical otherwise. |

## D. Crate-internal additions — MUST NOT appear in the public API

| Item | Visibility | Check |
|---|---|---|
| `NodeState.registered_topics` field | `pub(crate)`/private on `NodeState` | `NodeState` not re-exported |
| `handle_topic_registry_update` | private to `state.rs` | not reachable from `tests/` |
| accept-path `registered?`/`authorized?` checks | private in `handle_signed_message` | exercised via `received_messages()` / `effective_subscriptions()`, not directly |
| TOML topic entry type + hex decode | module-internal to `topic_registry::in_memory` | not in `lib.rs` `pub use` |

## E. Verification procedure (post-implementation analyze pass)

1. `git diff main -- src/lib.rs` shows the new `mod topic_registry;` + the six new `pub use` items (registry, control, error, event, watch, in-memory impl), and **no** unintended re-export of internals.
2. `Node::new` call sites (`main.rs`, all `tests/`) compile against the new signature; every delivery test registers the topics it sends on (else messages drop as `topic_not_registered`); `main.rs` constructs the topic registry via `from_file`.
3. `TopicRegistry::watch` takes **no** node/topic argument (global); `set_topic`/`remove_topic` live on `TopicRegistryControl` only; `TopicRegistry` and `SubscriptionRegistry` share no trait.
4. `Node::effective_subscriptions` returns `subscriptions ∩ registered_topics`; `Node::subscriptions` is byte-identical to `main` (still the declared set).
5. `handle_signed_message` performs the registered? and authorized? checks **before** `verifier.verify(...)`; `grep` confirms ordering; drop causes `topic_not_registered` + `publisher_not_authorized` exist (operator UX, not test-asserted).
6. `grep -n "pub " src/topic_registry/in_memory.rs` shows the impl + `new`/`from_file` public but the internals (`Inner`, the TOML decode type, the hex helper) private; `handle_topic_registry_update` is private in `state.rs`.
7. `PublicKey` gains only `Ord, PartialOrd` (no other derive/API change). `BTreeSet<PublicKey>` is used in `TopicRegistryEvent` + `NodeState.registered_topics`.
8. The topic-validity invariant test (SC-003), authorized-publisher test (SC-005), no-regression test (SC-010), and the multi-node two-registry integration test (SC-008) pass; registry-module tests pass without instantiating `Node`.

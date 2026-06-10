# Contract: Subscription Registry + Node Surface Delta (008)

**Date**: 2026-06-10 | **Plan**: [../plan.md](../plan.md)

This feature **adds** public surface (the registry module) and **changes** one existing public signature (`Node::new`). This file records the exact additions/changes so the post-implementation analyze pass can verify them against `src/lib.rs` and the module sources (constitution: spec fidelity is verified against code).

## A. New public surface — the registry module

New module `src/subscription_registry/`, re-exported from `lib.rs`:

```rust
pub use subscription_registry::{
    InMemorySubscriptionRegistry, SubscriptionEvent, SubscriptionRegistry,
    SubscriptionRegistryError, SubscriptionWatch,
};
```

### Trait `SubscriptionRegistry`

```rust
#[allow(async_fn_in_trait)] // mirrors the Network trait's v1 allowance (ADR 0007)
pub trait SubscriptionRegistry: Send + Sync + 'static {
    async fn set_interest(&self, node: PeerId, topics: BTreeSet<TopicId>) -> Result<(), SubscriptionRegistryError>;
    async fn unregister(&self, node: PeerId) -> Result<(), SubscriptionRegistryError>;
    async fn subscribe(&self, topics: BTreeSet<TopicId>) -> Result<SubscriptionWatch, SubscriptionRegistryError>;
    async fn interests_of(&self, node: PeerId) -> Result<Option<BTreeSet<TopicId>>, SubscriptionRegistryError>;
}
```

| Item | Shape | Contract |
|---|---|---|
| `SubscriptionEvent` | `#[non_exhaustive] enum { Joined { node, topics }, TopicsChanged { node, added, removed }, Left { node } }` | identity + interest only; no address, no deposit |
| `SubscriptionWatch` | `struct` (not `Clone`); drain via `recv().await -> Option<SubscriptionEvent>` | single-consumer; cold-start `Joined` burst then live deltas; gap-free/duplicate-free; ends on drop |
| `SubscriptionRegistryError` | `#[non_exhaustive] enum`; `Error + Debug + Display` | minimal now; grows with the on-chain backend (012) |
| `InMemorySubscriptionRegistry` | `pub struct`, private internals | `::new()`; `::from_file(path) -> Result<Self, _>`; shareable via `Arc` |

The on-chain decode/serialization types (012) MUST remain module-internal and MUST NOT appear here (spec FR-003).

## B. Changed public surface — `Node`

| Method | Before (004 on `main`) | After (008) |
|---|---|---|
| `Node::new` | `async fn new<N: Network>(PeerId, NodeConfig, initial_subscriptions: HashSet<TopicId>, Arc<N>, Arc<dyn Verifier>) -> Result<Self, NodeError>` | `async fn new<N: Network>(PeerId, NodeConfig, Arc<N>, Arc<dyn Verifier>, Arc<dyn SubscriptionRegistry>) -> Result<Self, NodeError>` — **drops `initial_subscriptions`** (interests now from `interests_of(self_id)`); **adds the registry**; fails fast with a registration-not-found `NodeError` when the node has no entry |
| `Node::candidates` | — | **new**: `fn candidates(&self, topic: &TopicId) -> Vec<PeerId>` — sync lock-and-clone snapshot of the per-topic candidate set, self-excluded |
| `Node::peers` | `fn peers(&self) -> &[BasicPeerDescriptor]` | **unchanged** — config bootstrap list, distinct from `candidates` |
| other methods | — | unchanged (`send`, `id`, `events`, `spawn_producer`, `received_messages`, `subscriptions`, `subscribe`, `unsubscribe`, `Drop`) |

`subscribe`/`unsubscribe` remain **sync** (ADR 0012); this feature does not make them async or registry-writing — the node is read-only.

## C. Changed public surface — config & event & error

| Item | Change |
|---|---|
| `NodeConfig` (`config.rs`) | **remove** the `subscribed_topics` field (the node's interests come from the registry, ADR 0013). `[[peers]]` bootstrap entries retained. |
| `Event` (`event.rs`) | **add** `SubscriptionUpdate(SubscriptionEvent)` variant (enum stays `#[non_exhaustive]`). |
| `NodeError` (`error.rs`) | **add** a registration-not-found variant (fail-fast, spec FR-018); enum exhaustiveness per the crate's per-feature-error convention. |

## D. Crate-internal additions — MUST NOT appear in the public API

| Item | Visibility | Check |
|---|---|---|
| `NodeState.candidates` field + `candidates_snapshot` | `pub(crate)` / private on `NodeState` | `NodeState` not re-exported |
| `handle_subscription_update` | private to `state.rs` | not reachable from `tests/` |
| TOML subscription-list entry type | module-internal to `subscription_registry::in_memory` | not in `lib.rs` `pub use` |

## E. Verification procedure (post-implementation analyze pass)

1. `git diff main -- src/lib.rs` shows the new `mod subscription_registry;` + the five new `pub use` items, and **no** unintended re-export of internals.
2. `Node::new` call sites (`main.rs`, all `tests/`) compile against the new signature; no caller passes `initial_subscriptions`; `main.rs` constructs the registry via `from_file`.
3. `NodeConfig` no longer has `subscribed_topics` (grep `config.rs` + any TOML fixtures/templates).
4. `Node::candidates` returns the self-excluded, interest-scoped set; `Node::peers` is byte-identical to `main`.
5. `grep -n "pub " src/subscription_registry/in_memory.rs` shows the impl public but the entry type and fields private; `handle_subscription_update` is private in `state.rs`.
6. The source-of-truth invariant test (SC-007) and the multi-node integration test (SC-008) pass; registry-module tests pass without instantiating `Node`.

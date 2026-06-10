# Contract: Subscription Registry + Node Surface Delta (008)

**Date**: 2026-06-10 | **Plan**: [../plan.md](../plan.md)

This feature **adds** public surface (the registry module) and **changes** one existing public signature (`Node::new`). This file records the exact additions/changes so the post-implementation analyze pass can verify them against `src/lib.rs` and the module sources (constitution: spec fidelity is verified against code).

## A. New public surface — the registry module

New module `src/subscription_registry/`, re-exported from `lib.rs`:

```rust
pub use subscription_registry::{
    InMemorySubscriptionRegistry, MembershipEvent, MembershipWatch, SubscriptionRegistry,
    SubscriptionRegistryControl, SubscriptionRegistryError,
};
```

### Traits — read (node-facing) vs control (operator/test)

```rust
pub trait SubscriptionRegistry: Send + Sync + 'static {  // read-only; what Node depends on; 012 implements this
    // The SINGLE node-keyed stream the node derives ALL of its registry state from.
    // RPITIT with an explicit `Send` bound: the node-owned reader awaits it in a
    // spawned task (the `Send`-bounded follow-up ADR 0007 flags to `async fn` in traits).
    fn watch(&self, node: PeerId)
        -> impl std::future::Future<Output = Result<MembershipWatch, SubscriptionRegistryError>> + Send;
}

#[allow(async_fn_in_trait)]
pub trait SubscriptionRegistryControl: SubscriptionRegistry {  // operator/test write surface; node never depends on it
    async fn set_topics(&self, node: PeerId, topics: BTreeSet<TopicId>) -> Result<(), SubscriptionRegistryError>;
    async fn unregister(&self, node: PeerId) -> Result<(), SubscriptionRegistryError>;
}
```

There is **no point-read** on either trait: the node derives its own topics from the head `Joined` of `watch`'s cold-start burst, 010 reads membership via `watch`, and 012 is the impl — so `entry`/`SubscriptionEntry` were removed entirely. Registry-module tests assert on the watch stream, not a read-back.

`Node` is constructed **generically** over the read trait — `Node::new<N: Network, R: SubscriptionRegistry>(…, registry: Arc<R>)` — so it has no write methods in scope. (It is `Arc<R>`, not `Arc<dyn SubscriptionRegistry>`: `async fn` traits aren't `dyn`-compatible, so the registry is consumed generically exactly as `Network` is, per ADR 0007's allowance.) `InMemorySubscriptionRegistry` implements **both** traits; tests/operator-sim hold the concrete `Arc<InMemorySubscriptionRegistry>` to drive writes.

| Item | Shape | Contract |
|---|---|---|
| `MembershipEvent` | `#[non_exhaustive] enum { Joined { node, topics }, TopicsChanged { node, added, removed }, Left { node } }` | identity + topics only; no address, no deposit. The watch's head `Joined { node, topics }` carries the watcher's **own** id + topics (the removed `entry`'s role) |
| `MembershipWatch` | `struct` (not `Clone`); drain via `recv().await -> Option<MembershipEvent>` | single-consumer; node-keyed cold-start `Joined` burst (own entry first, then scoped members) then live deltas; gap-free/duplicate-free; ends on drop |
| `SubscriptionRegistryError` | `#[non_exhaustive] enum`; `Error + Debug + Display` | minimal now; grows with the on-chain backend (012) |
| `InMemorySubscriptionRegistry` | `pub struct`, private internals | `::new()`; `::from_file(path) -> Result<Self, _>`; shareable via `Arc` |

The on-chain decode/serialization types (012) MUST remain module-internal and MUST NOT appear here (spec FR-003).

## B. Changed public surface — `Node`

| Method | Before (004 on `main`) | After (008) |
|---|---|---|
| `Node::new` | `async fn new<N: Network>(PeerId, NodeConfig, initial_subscriptions: HashSet<TopicId>, Arc<N>, Arc<dyn Verifier>) -> Result<Self, NodeError>` | `async fn new<N: Network, R: SubscriptionRegistry>(PeerId, NodeConfig, Arc<N>, Arc<dyn Verifier>, Arc<R>) -> Result<Self, NodeError>` — **drops `initial_subscriptions`**; **adds the registry generically** (`Arc<R>`, *not* `Arc<dyn>` — `async fn` traits aren't `dyn`-compatible; mirrors `Network`'s `Arc<N>`); seeds `NodeState` with an **empty** subscription set and spawns a node-owned reader that calls `watch(self_id)` — topics + candidates converge as the cold-start burst drains. **No fail-fast**: a node with no entry constructs cleanly and stays at empty derived state (FR-018 relaxed) |
| `Node::candidates` | — | **new**: `fn candidates(&self, topic: &TopicId) -> Vec<PeerId>` — sync lock-and-clone snapshot of the per-topic candidate set, self-excluded |
| `Node::peers` | `fn peers(&self) -> &[BasicPeerDescriptor]` | **unchanged** — config bootstrap list, distinct from `candidates` |
| other methods | — | unchanged (`send`, `id`, `events`, `spawn_producer`, `received_messages`, `subscriptions`, `subscribe`, `unsubscribe`, `Drop`) |

`subscribe`/`unsubscribe` remain **sync** (ADR 0012); this feature does not make them async or registry-writing — the node is read-only.

## C. Changed public surface — config & event & error

| Item | Change |
|---|---|
| `NodeConfig` (`config.rs`) | **remove** the `subscribed_topics` field (the node's topics come from the registry, ADR 0013). `[[peers]]` bootstrap entries retained. |
| `Event` (`event.rs`) | **add** `MembershipUpdate(MembershipEvent)` variant (enum stays `#[non_exhaustive]`). |
| `NodeError` (`error.rs`) | **unchanged** — no new variant. With topics derived from the stream rather than a startup lookup, construction no longer fails on a missing entry (FR-018 relaxed), so there is no registration-not-found error. |

## D. Crate-internal additions — MUST NOT appear in the public API

| Item | Visibility | Check |
|---|---|---|
| `NodeState.candidates` field + `candidates_snapshot` | `pub(crate)` / private on `NodeState` | `NodeState` not re-exported |
| `handle_membership_update` | private to `state.rs` | not reachable from `tests/` |
| TOML subscription-list entry type | module-internal to `subscription_registry::in_memory` | not in `lib.rs` `pub use` |

## E. Verification procedure (post-implementation analyze pass)

1. `git diff main -- src/lib.rs` shows the new `mod subscription_registry;` + the six new `pub use` items (registry, control, error, event, watch, in-memory impl — **no** `SubscriptionEntry`), and **no** unintended re-export of internals.
2. `Node::new` call sites (`main.rs`, all `tests/`) compile against the new signature; no caller passes `initial_subscriptions`; `main.rs` constructs the registry via `from_file`.
3. `NodeConfig` no longer has `subscribed_topics` (grep `config.rs` + any TOML fixtures/templates).
4. `Node::candidates` returns the self-excluded, topic-scoped set; `Node::peers` is byte-identical to `main`.
5. `grep -n "pub " src/subscription_registry/in_memory.rs` shows the impl + `new`/`from_file` public but the internals (`Inner`, `Subscriber`, the TOML decode type) private; `handle_membership_update` is private in `state.rs`.
6. The source-of-truth invariant test (SC-007) and the multi-node integration test (SC-008) pass; registry-module tests pass without instantiating `Node`.

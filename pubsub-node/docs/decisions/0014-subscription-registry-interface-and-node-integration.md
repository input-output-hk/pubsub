# ADR 0014: Subscription registry interface and node integration

**Status**: Accepted
**Date**: 2026-06-10
**Feature**: 008-node-registry
**Source**: `specs/008-node-registry/{spec,plan,data-model}.md`; `specs/event-loop-and-registry-contract.md` §2/§3/§5; ADR 0007 (Network actor-handle), ADR 0011/0012 (004 pure core + lifecycle), ADR 0013 (source of truth); `IMPLEMENTATION_NOTES.md` N-007; `../docs/node-lifecycle/{README,joining}.md`.

## Context

Feature 008 is the in-memory **subscription registry** (the node-membership "subscription list"). It needs (a) a published interface the node and tests consume, and (b) integration with feature 004's now-merged pure core (`apply`/`NodeState`/`Effect`). Both are structural per Principle III: the trait surface is what feature 010 (sampler) and 012 (on-chain reader) build against; the seam variant and the `Node::new` shape touch already-merged code (004) and existing callers; reversing any of them is not a local rewrite. ADR 0013 already fixed the *source-of-truth* question (subscription list, not config); this ADR fixes the *interface and wiring*.

## Decision

### 1. Two traits — read (node-facing) and control (operator/test) — + event + watch (mirror the Network actor-handle, ADR 0007)

The node-facing trait is **read-only**; the write surface is a separate trait extending it. The node depends only on the read trait, so it has no write methods in scope; the real 012 chain reader implements only the read trait (on-chain writes are transactions, not a reader call); and the domain interface stays free of write/test signatures (`/speckit-analyze` finding F3).

```rust
pub trait SubscriptionRegistry: Send + Sync + 'static {        // read-only; Node depends on this; 012 implements it
    // Node-keyed watch: the SINGLE method, and the single stream from which the
    // node derives ALL of its registry state. Returns a Send future (RPITIT)
    // because the node-owned reader awaits it inside a spawned task.
    fn watch(&self, node: PeerId)
        -> impl std::future::Future<Output = Result<MembershipWatch, SubscriptionRegistryError>> + Send;
}

// No point-read anywhere: nothing consumes one (the node derives its topics from
// `watch`; 010 reads via `watch`; 012 is the impl). Tests assert on the head
// `Joined { node, topics }` of a watch's cold-start burst instead.

pub trait SubscriptionRegistryControl: SubscriptionRegistry {  // operator/test write surface; node never depends on it
    async fn set_topics(&self, node: PeerId, topics: BTreeSet<TopicId>) -> Result<(), SubscriptionRegistryError>;
    async fn unregister(&self, node: PeerId) -> Result<(), SubscriptionRegistryError>;
}

#[non_exhaustive]
pub enum MembershipEvent {
    Joined { node: PeerId, topics: BTreeSet<TopicId> },
    TopicsChanged { node: PeerId, added: BTreeSet<TopicId>, removed: BTreeSet<TopicId> },
    Left { node: PeerId },
}
```

`MembershipWatch` is single-consumer (not `Clone`, owns an unbounded `mpsc` receiver, ends on drop) — the `NetworkHandle` shape (ADR 0007). `watch(node)` is **node-keyed**: it scopes the stream to `node`'s own subscription-list entry, and on open replays a single cold-start burst of `Joined` events — the node's **own** entry first (`Joined { node, own_topics }`, from which the node derives its subscription set), then the current **members** of those topics (`Joined { member, scoped_topics }`, from which it derives candidate sets) — before streaming live deltas. The node folds the whole stream from empty initial state, distinguishing its own id from others. This single-stream model **supersedes** the earlier split of `entry(self_id)` (self-lookup) + `watch_members(topics)` (membership): the node no longer does a separate point-read to learn its topics — it learns them from the stream, so it can start with an empty subscription set and an empty accept-filter and converge as the burst drains. The point-read `entry`/`SubscriptionEntry` is therefore **removed entirely** (not just demoted): no consumer needs it, and tests assert on the head `Joined { node, topics }` of the cold-start burst — the event-stream carrier of a node's own id + topics. No end-of-snapshot boundary marker (the enum is `#[non_exhaustive]`; add `SnapshotComplete` when 010 needs warmth). A future `SubscriptionEntry` (with deposit/identity keys) can return when a consumer first needs a materialized record (012). Events carry identity + topics only — no address (off-chain), no deposit (deferred).

### 2. Seam variant + handler

The node consumes the registry through one new `Event` variant: `Event::MembershipUpdate(MembershipEvent)`, with a named `handle_membership_update(&mut NodeState, MembershipEvent) -> Vec<Effect>` dispatched by one line in `apply` (the ADR 0011 named-handler convention). **Renamed from the `RegistryUpdate` placeholder** ADR 0011 and CLAUDE.md anticipated — now that there are two registries, `MembershipUpdate` disambiguates from the topic registry. This rename touches the shared seam name: it needs a heads-up to the 004 author and a one-line update to ADR 0011's illustrative comment and the CLAUDE.md SpecKit block when 008 lands.

### 3. Candidate set in `NodeState`, distinct from config `peers`

`NodeState` gains `candidates: HashMap<TopicId, HashSet<PeerId>>`, folded by `handle_membership_update`. The handler **branches on `node == self_id`**: an event about the node itself updates `subscriptions` (its accept-filter); an event about any other node updates `candidates` (`Joined` adds, `TopicsChanged` adds/removes, `Left` removes). This self-branch is what makes the single `watch` stream the source of truth for *both* kinds of state — the node is never its own candidate because its own-id events are routed to `subscriptions`, not `candidates`. A public `Node::candidates(&TopicId) -> Vec<PeerId>` getter exposes a snapshot (the `received_messages()` lock-and-clone pattern). This **resolves N-007** for the 008 side: the candidate set is the peer data that enters `NodeState` (it is mutated by a transition, so it is state); the static config `[[peers]]` bootstrap list stays a `Node` shell field, untouched — the two are distinct sources (`joining.md` connects to bootstrap nodes *and separately* filters the subscription list).

### 4. `Node::new` sources interests from the registry; node is read-only

`Node::new` **drops `initial_subscriptions`** and **adds the registry generically** — `Node::new<N: Network, R: SubscriptionRegistry>(…, registry: Arc<R>)`, *not* `Arc<dyn SubscriptionRegistry>`. (An `async fn` trait is not `dyn`-compatible; the registry is therefore consumed generically exactly as `Network` is via `Arc<N>` under ADR 0007's `async_fn_in_trait` allowance. The real chain reader, 012, is a second generic impl, not a trait object.) It seeds `NodeState` with an **empty** subscription set, spawns the node-owned reader producer (which calls `watch(self_id)`), and returns — it does **not** block on a startup point-read. The node's subscriptions and candidates then converge as the reader drains the cold-start burst onto the event loop. Because the topic set is now learned from the stream rather than a startup `entry` lookup, **construction no longer fails fast on a missing entry**: a node whose id has no subscription-list entry constructs cleanly and simply stays at empty derived state (the "registered but not yet present / initializing" posture; FR-018 is relaxed accordingly, the reader logs at `error` if the watch cannot be opened). 002's `subscribed_topics` config field is **removed**. The node issues **no** registry writes — `set_topics`/`unregister` are for the `from_file` loader and test harnesses (operator stand-ins). The node-local `subscribe`/`unsubscribe` mutators are **removed** ([ADR 0015](0015-node-has-no-local-subscription-mutators.md)): with the subscription set now registry-derived, a node-local writer to it would be a second, non-authoritative source — runtime topic changes arrive on the `watch` stream instead.

## Consequences

- The registry module is independently testable without the node loop; the fold is testable as a pure state machine (contract §5).
- **Public API change**: `Node::new`'s signature changes and `NodeConfig.subscribed_topics` is removed — `main.rs` and existing `tests/` callers are updated in the same feature. The candidate set adds `Node::candidates`; `Node::peers` is unchanged.
- Clean 012 swap: `from_file` → chain reader; `watch` → on-chain reads/subscriptions; the node, `apply`, and the fold (subscriptions + candidates) are untouched.
- The seam stays minimal (one variant + one handler + one producer), per the contract §3 ownership split; whoever merges the 008 arm against `apply` is exhaustiveness-checked by the compiler.
- **Subscriptions are derived asynchronously**: a freshly constructed node starts with an empty accept-filter and converges once the cold-start burst drains. Send-then-observe tests must wait for convergence (the `await_subscriptions` harness helper) rather than assuming topics are set the instant `Node::new` returns. The watched topic set is the node's own topics *at watch time*; full runtime re-scoping on an own-topic change is deferred to 012.

## Alternatives considered

- **A side `TopicPeerView` outside `apply`** (the deleted `docs/registry-node-contract.md` sketch): rejected — bypasses the pure core and the agreed event-queue seam; the fold belongs in `apply`/`NodeState`.
- **Merge the candidate set into the config `peers` field**: rejected — conflates the bootstrap set with topic-derived membership and would break the future dialer's bootstrap contract (N-007).
- **Keep the `RegistryUpdate` variant name**: rejected — ambiguous now that the topic registry is a separate artifact; `MembershipUpdate` names what it carries.
- **Make `subscribe`/`unsubscribe` async, registry-writing**: rejected — the node is read-only (ADR 0013), and 004 (ADR 0012) deliberately kept them sync. (The local mutators were subsequently **removed** entirely — [ADR 0015](0015-node-has-no-local-subscription-mutators.md) — once the subscription set became registry-derived: a second, non-authoritative writer to that set was a split-brain hazard.)
- **Node self-seeds its registration on startup**: rejected — see ADR 0013 (circular; makes the node a writer).

## Sources

- `specs/008-node-registry/spec.md` — FR-001..021, SC-001..009, Clarifications.
- `specs/event-loop-and-registry-contract.md` — §2 (push read model), §3 (seam ownership), §5 (test strategy).
- ADR 0007 (Network handle actor pattern this watch mirrors), ADR 0011/0012 (the 004 pure core + sync mutators + lifecycle), ADR 0013 (source of truth).
- `IMPLEMENTATION_NOTES.md` N-007 (peers placement, revisit at 008/005).
- `../docs/node-lifecycle/{README,joining}.md` — subscription list vs topic registry; endpoints off-chain; node read-only at runtime.

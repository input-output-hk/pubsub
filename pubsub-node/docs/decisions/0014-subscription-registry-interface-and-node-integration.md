# ADR 0014: Subscription registry interface and node integration

**Status**: Accepted
**Date**: 2026-06-10
**Feature**: 008-node-registry
**Source**: `specs/008-node-registry/{spec,plan,data-model}.md`; `specs/event-loop-and-registry-contract.md` §2/§3/§5; ADR 0007 (Network actor-handle), ADR 0011/0012 (004 pure core + lifecycle), ADR 0013 (source of truth); `IMPLEMENTATION_NOTES.md` N-007; `../docs/node-lifecycle/{README,joining}.md`.

## Context

Feature 008 is the in-memory **subscription registry** (the node-membership "subscription list"). It needs (a) a published interface the node and tests consume, and (b) integration with feature 004's now-merged pure core (`apply`/`NodeState`/`Effect`). Both are structural per Principle III: the trait surface is what feature 010 (sampler) and 012 (on-chain reader) build against; the seam variant and the `Node::new` shape touch already-merged code (004) and existing callers; reversing any of them is not a local rewrite. ADR 0013 already fixed the *source-of-truth* question (subscription list, not config); this ADR fixes the *interface and wiring*.

## Decision

### 1. Trait + event + watch (mirror the Network actor-handle, ADR 0007)

```rust
pub trait SubscriptionRegistry: Send + Sync + 'static {
    async fn set_interest(&self, node: PeerId, topics: BTreeSet<TopicId>) -> Result<(), SubscriptionRegistryError>;
    async fn unregister(&self, node: PeerId) -> Result<(), SubscriptionRegistryError>;
    async fn watch_members(&self, topics: BTreeSet<TopicId>) -> Result<MembershipWatch, SubscriptionRegistryError>;
    async fn entry(&self, node: PeerId) -> Result<Option<SubscriptionEntry>, SubscriptionRegistryError>;
}

#[non_exhaustive]
pub enum MembershipEvent {
    Joined { node: PeerId, topics: BTreeSet<TopicId> },
    TopicsChanged { node: PeerId, added: BTreeSet<TopicId>, removed: BTreeSet<TopicId> },
    Left { node: PeerId },
}

/// A node's entry in the subscription list (the materialized record `entry` returns).
#[non_exhaustive]
pub struct SubscriptionEntry {
    pub node: PeerId,
    pub topics: BTreeSet<TopicId>,
    // future (012): deposit, identity keys, …
}
```

`MembershipWatch` is single-consumer (not `Clone`, owns an unbounded `mpsc` receiver, ends on drop), replays current members as a `Joined` cold-start burst then streams live deltas — the `NetworkHandle` shape (ADR 0007). No end-of-snapshot boundary marker (the enum is `#[non_exhaustive]`; add `SnapshotComplete` when 010 needs warmth). `entry` is the self-lookup that lets the node learn its own authoritative interests before opening a membership watch (enforces ADR 0013). Events carry identity + interest only — no address (off-chain), no deposit (deferred).

### 2. Seam variant + handler

The node consumes the registry through one new `Event` variant: `Event::MembershipUpdate(MembershipEvent)`, with a named `handle_membership_update(&mut NodeState, MembershipEvent) -> Vec<Effect>` dispatched by one line in `apply` (the ADR 0011 named-handler convention). **Renamed from the `RegistryUpdate` placeholder** ADR 0011 and CLAUDE.md anticipated — now that there are two registries, `MembershipUpdate` disambiguates from the topic registry. This rename touches the shared seam name: it needs a heads-up to the 004 author and a one-line update to ADR 0011's illustrative comment and the CLAUDE.md SpecKit block when 008 lands.

### 3. Candidate set in `NodeState`, distinct from config `peers`

`NodeState` gains `candidates: HashMap<TopicId, HashSet<PeerId>>`, folded by `handle_membership_update` (`Joined` adds, `TopicsChanged` adds/removes, `Left` removes), with the node's own `PeerId` excluded **locally** in the fold. A public `Node::candidates(&TopicId) -> Vec<PeerId>` getter exposes a snapshot (the `received_messages()` lock-and-clone pattern). This **resolves N-007** for the 008 side: the candidate set is the peer data that enters `NodeState` (it is mutated by a transition, so it is state); the static config `[[peers]]` bootstrap list stays a `Node` shell field, untouched — the two are distinct sources (`joining.md` connects to bootstrap nodes *and separately* filters the subscription list).

### 4. `Node::new` sources interests from the registry; node is read-only

`Node::new` **drops `initial_subscriptions`** and **adds `registry: Arc<dyn SubscriptionRegistry>`**. At startup it calls `entry(self_id)`; `None` ⇒ **fail fast** with a registration-not-found `NodeError` (no empty-interest fallback); otherwise it seeds `NodeState.subscriptions` from the returned set, `watch_members` on those topics, and spawns the node-owned reader producer. 002's `subscribed_topics` config field is **removed**. The node issues **no** registry writes — `set_interest`/`unregister` are for the `from_file` loader and test harnesses (operator stand-ins). `subscribe`/`unsubscribe` stay synchronous (ADR 0012), unchanged by this feature.

## Consequences

- The registry module is independently testable without the node loop; the fold is testable as a pure state machine (contract §5).
- **Public API change**: `Node::new`'s signature changes and `NodeConfig.subscribed_topics` is removed — `main.rs` and existing `tests/` callers are updated in the same feature. The candidate set adds `Node::candidates`; `Node::peers` is unchanged.
- Clean 012 swap: `from_file` → chain reader; `entry`/`watch_members` → on-chain reads; the node, `apply`, and the candidate-set fold are untouched.
- The seam stays minimal (one variant + one handler + one producer), per the contract §3 ownership split; whoever merges the 008 arm against `apply` is exhaustiveness-checked by the compiler.
- The node's interest set is fixed at startup (spec Clarifications); runtime self-interest changes are deferred to 012.

## Alternatives considered

- **A side `TopicPeerView` outside `apply`** (the deleted `docs/registry-node-contract.md` sketch): rejected — bypasses the pure core and the agreed event-queue seam; the fold belongs in `apply`/`NodeState`.
- **Merge the candidate set into the config `peers` field**: rejected — conflates the bootstrap set with interest-derived membership and would break the future dialer's bootstrap contract (N-007).
- **Keep the `RegistryUpdate` variant name**: rejected — ambiguous now that the topic registry is a separate artifact; `MembershipUpdate` names what it carries.
- **Make `subscribe`/`unsubscribe` async, registry-writing**: rejected — the node is read-only (ADR 0013), and 004 (ADR 0012) deliberately kept them sync.
- **Node self-seeds its registration on startup**: rejected — see ADR 0013 (circular; makes the node a writer).

## Sources

- `specs/008-node-registry/spec.md` — FR-001..021, SC-001..009, Clarifications.
- `specs/event-loop-and-registry-contract.md` — §2 (push read model), §3 (seam ownership), §5 (test strategy).
- ADR 0007 (Network handle actor pattern this watch mirrors), ADR 0011/0012 (the 004 pure core + sync mutators + lifecycle), ADR 0013 (source of truth).
- `IMPLEMENTATION_NOTES.md` N-007 (peers placement, revisit at 008/005).
- `../docs/node-lifecycle/{README,joining}.md` — subscription list vs topic registry; endpoints off-chain; node read-only at runtime.

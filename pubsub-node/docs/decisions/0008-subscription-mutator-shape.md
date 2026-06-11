# ADR 0008: Subscription mutator shape — sync `&self` mutators with interior mutability, linearizable

**Status**: Superseded (in part) by [ADR 0015](0015-node-has-no-local-subscription-mutators.md)
**Date**: 2026-05-30
**Feature**: 002-topic-subscription-filtering
**Source**: `specs/002-topic-subscription-filtering/research.md` §3 and §8

> **Superseded by [ADR 0015](0015-node-has-no-local-subscription-mutators.md).** The *runtime mutator surface* this ADR shapes — `Node::subscribe`/`unsubscribe` + `SubscribeOutcome`/`UnsubscribeOutcome` — was **removed** once feature 008 made the subscription list the single source of truth (ADR 0013) and had the node derive its subscription set from the `watch` stream (ADR 0014). A node no longer mutates its own subscriptions; runtime changes are operator/registry actions that arrive on the membership stream. The concurrency reasoning below (sync `&self`, interior mutability, linearizability) remains of historical interest but no longer describes a live API.

## Context

Feature 002 adds a mutable per-Node subscription set (a
`HashSet<TopicId>`) plus a runtime API to mutate it:

- `subscribe(topic)` — add to the set; report whether the call mutated.
- `unsubscribe(topic)` — remove from the set; report whether the call
  mutated.
- `subscriptions()` — snapshot the current set.

The Node is already async — its receive task lives behind
`tokio::spawn` (ADR 0006) and `Node::send` is `async fn` (forwards
through the `NetworkHandle` from ADR 0007). The receive task also
needs to consult the subscription set on every inbound delivery, in
order to filter off-topic messages (FR-004) and emit the FR-011 drop
log on misses. Both the external mutator surface and the internal
receive-path read must see a single, coherent state under FR-015's
linearizability contract.

Four structural decisions are entangled here, each independently
reversible only with caller-side ripple:

1. **Sync `fn` vs `async fn` for the mutator surface.** `async fn`
   would propagate `.await` and Send/Sync trait bounds into every
   caller; sync `fn` keeps the call site one-liners and matches
   FR-006's "synchronous, in-memory mutators" wording.
2. **`&self` vs `&mut self`.** The Node is shared between the
   externally-visible caller surface and the background recv task; the
   recv task holds an `Arc::clone` of the same lock-protected state.
   `&mut self` is incompatible with that sharing pattern — it would
   force the caller into `Arc<Mutex<Node>>` externally, redundant with
   the interior lock.
3. **Interior-mutability primitive.** The receive-path read, the
   mutator API, and the snapshot getter all touch the same `HashSet`;
   FR-015 requires linearizability across all three. `std::sync::Mutex`
   serializes the three operations trivially; the critical section is
   pure CPU work (a `HashSet::contains` / `insert` / `remove` / `clone`)
   so blocking the executor across an `await` boundary is not a concern.
4. **Linearizability as the normative contract.** FR-015 promises
   that any subsequent operation after a mutator returns observes the
   post-mutation state. The mutex's acquire-release memory ordering on
   the same `Arc<Mutex<…>>` gives that happens-before guarantee.

Reversing any of these (e.g., switching to `async fn`, exposing the
lock through `Arc<Mutex<Node>>`, or swapping in a lock-free primitive)
would touch every caller of `subscribe` / `unsubscribe` /
`subscriptions` and the recv-task body — i.e., an external-interface
change with ripple. The Constitution's Principle III "structural
decision" trigger fires, so the decision is recorded as an ADR.

## Decision

`Node::subscribe` and `Node::unsubscribe` are **synchronous `fn`** on
**`&self`**, with the subscription set held behind an
**`Arc<Mutex<HashSet<TopicId>>>`** — the same primitive shape ADR 0006
established for `received: Arc<Mutex<Vec<ReceivedDelivery>>>`. The
mutex serializes:

- The receive task's per-delivery `HashSet::contains` membership check
  (FR-004).
- The external mutator API's `HashSet::insert` / `HashSet::remove`
  (FR-006).
- The snapshot getter's `HashSet::clone()` under the lock (FR-013).

This is the only synchronization primitive required to satisfy
FR-015's linearizability contract at v1 scale.

Concretely:

```rust
impl Node {
    pub fn subscribe(&self, topic: TopicId) -> SubscribeOutcome { ... }
    pub fn unsubscribe(&self, topic: TopicId) -> UnsubscribeOutcome { ... }
    pub fn subscriptions(&self) -> Vec<TopicId> { ... }
}
```

`SubscribeOutcome` and `UnsubscribeOutcome` are two-variant closed
enums (`Added` / `AlreadyPresent`; `Removed` / `NotSubscribed`); no
`Result` wrapping at this iteration because no failure mode exists
under FR-006 (no I/O, no registry lookup, no persistence).

## Consequences

- The public mutator surface is one-liner-friendly:
  `node.subscribe(topic_id);` — no `.await` propagation, no `Send +
  Sync` bounds bleeding into caller signatures, no exclusive-borrow
  ripple from `&mut self`.
- The recv task and external callers share a single
  `Arc<Mutex<HashSet<TopicId>>>` — when both `subscriptions` and
  `received` must be acquired (the receive-path read followed by a
  push-to-received), the project-wide lock-acquisition order is
  `subscriptions` first, `received` second. Only the recv task ever
  holds both, and only briefly; external callers of `subscribe` /
  `unsubscribe` never touch `received`, and external callers of
  `received_messages` never touch `subscriptions`. No deadlock risk
  in practice; the convention is documented in
  `data-model.md` §3 for the discipline.
- The critical section is microseconds — a `HashSet::contains` or
  `insert` or `remove` or `clone` of a set bounded by the spec's
  in-practice 0–3 topics. Blocking the executor for that duration is
  preferable to a tokio `Mutex` round-trip's allocation + future
  poll on every receive.
- Future failure modes (registry validation in feature 008,
  persistence I/O in a future persistence ADR) wrap the return type
  as `Result<Outcome, Error>`. That wrapping is a follow-on ADR —
  **not** a revision of 0008. The Outcome enums stay closed; failure
  variants ride in the `Err` arm, not as new Outcome variants.

## Alternatives considered

- **`async fn subscribe(&self, …)`**: rejected. The body is a brief
  lock acquire + HashSet operation + release; no scheduling-aware
  work, no I/O, no `await` inside the critical section. `async fn`
  would propagate Send/Sync trait bounds and a Future allocation into
  every call site for no benefit. FR-006 explicitly specifies
  "synchronous, in-memory mutators".
- **`&mut self` mutators**: rejected. The Node is shared between the
  recv task and external callers via `Arc<Node>` (or equivalent).
  `&mut self` requires exclusive access, which is incompatible with
  the sharing pattern. Forcing callers into `Arc<Mutex<Node>>`
  externally is redundant with the interior lock and undoes ADR 0006's
  precedent.
- **`tokio::sync::Mutex` instead of `std::sync::Mutex`**: rejected.
  The critical section is pure CPU work — no `await` inside. Holding
  a `std::sync::Mutex` across a few hundred nanoseconds is cheaper
  than a Future round-trip on every receive. 001 already uses
  `std::sync::Mutex` for `received`; consistency wins.
- **`std::sync::RwLock<HashSet<TopicId>>`**: rejected. The read path
  (receive-side filter) and the write path (subscribe / unsubscribe)
  execute at comparable frequencies in 002's workload; RwLock
  optimises for many-readers-one-writer, which isn't the shape here.
  Adds API complexity (separate read / write guards) without a
  throughput benefit at v1 scale.
- **Lock-free** (`arc-swap`, `crossbeam-epoch`, `evmap`, …): rejected.
  Contradicts the Constitution's "Justified dependencies" rule (a new
  dep with its own ADR slot); the workload doesn't motivate it;
  FR-015's linearizability is harder to argue for non-mutex
  primitives.
- **Per-topic locks / sharded** (`Arc<Mutex<…>>` per topic): rejected.
  002's scale is tiny (US2 exercises 3 topics; subscription-set sizes
  of 0–3); coarse-grained locking is fine, and sharded locks would
  complicate the `subscriptions()` snapshot semantics (atomic across
  all shards is non-trivial).
- **Free-standing functions over the `Arc<Mutex<HashSet<…>>>`**
  (exposing the lock in the public API): rejected as ergonomic
  backslide — the lock is an implementation detail; surfacing it
  forces every caller to learn the locking discipline.

## Sources

- `specs/002-topic-subscription-filtering/research.md` §3 — full
  rationale for the sync `&self` shape.
- `specs/002-topic-subscription-filtering/research.md` §8 — ADR slot
  summary with the boundary against follow-on ADRs.
- ADR 0006 — receive-task model that motivates the
  `Arc<Mutex<…>>` shared-state pattern.
- ADR 0007 — NetworkHandle actor pattern that underlies the
  async surface 002 layers onto without modification.

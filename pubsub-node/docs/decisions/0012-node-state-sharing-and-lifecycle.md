# ADR 0012: Node state sharing and event-loop lifecycle — `Arc<Mutex<NodeState>>`, spawn-in-constructor

**Status**: Accepted
**Date**: 2026-06-09
**Feature**: 004-node-event-loop
**Source**: `specs/event-loop-and-registry-contract.md` §1.2, §1.4; `specs/004-node-event-loop/research.md` R4–R6

## Context

ADR 0011 makes `NodeState` the single explicit state value and `apply` its only
event-driven mutator. Two questions the seam contract explicitly left open for this
feature's plan (§1.4) follow immediately:

1. **How is `NodeState` shared** between the event loop (writer) and the public getters
   (`received_messages`, `subscriptions`) — and what happens to the synchronous
   `subscribe`/`unsubscribe` mutators (ADR 0008) now that their state lives inside
   `NodeState`?
2. **Who owns the event loop's lifetime** — is it spawned inside `Node::new` (the node as a
   self-contained handle) or driven by the caller?

Both are structural: the sharing choice decides whether every getter and mutator in the
public API stays synchronous or becomes async (an external-interface change rippling to
every caller and test), and the lifecycle choice decides the ownership contract every
consumer of `Node` relies on.

## Decision

1. **Sharing: `Arc<Mutex<NodeState>>`** — one `std::sync::Mutex` around the whole state
   value, replacing 001–003's two separate mutexes (`received`, `subscriptions`).
   - The event loop is the **sole event-driven writer**: it locks, calls `apply`, unlocks.
   - Public getters lock-and-clone synchronously — the 003 API and its linearizability
     guarantee carry over with one lock instead of two (the per-event critical section
     remains pure CPU work: set lookup, signature verify, vector push).
   - The two-lock acquisition-order discipline from ADR 0008's consequences dissolves:
     there is only one lock.
2. **`subscribe`/`unsubscribe`: stay synchronous public methods** (ADR 0008's surface,
   unchanged), now thin lock-takers delegating to `NodeState::subscribe`/`unsubscribe`
   where the logic and its inline logging live (testable in the pure core). The invariant
   reads: *`NodeState` is mutated only under its mutex — event-driven transitions via
   `apply`, control-plane subscription changes via its own methods.*
   > **Superseded by [ADR 0015](0015-node-has-no-local-subscription-mutators.md).** Feature 008 made the subscription list the source of truth (ADR 0013) and the subscription set registry-derived (ADR 0014), so the local `subscribe`/`unsubscribe` mutators were **removed**. The revised invariant: *`NodeState` is mutated only under its mutex, exclusively via `apply`* — the subscription set is folded from the node's own entry on the membership stream, not from a control-plane method.
   - Subscriptions in this feature are config-seeded and effectively static (mutated only
     in tests); topics from config are assumed already registry-confirmed (spec
     Assumptions). When registry-driven subscription-update events arrive through the
     queue (feature 008 and beyond, riding the existing `Event::RegistryUpdate` seam),
     these sync methods are the expected deprecation candidates — that transition will be
     its own ADR, not a revision of this one.
3. **Lifecycle: spawn-in-constructor, drop-abort** — `Node::new` spawns the event loop and
   the first producer (`network_mailbox_loop`); `Drop` aborts the loop and every producer.
   This is the seam commit's existing contract, retained deliberately: `Node` is an
   interactive handle whose getters/mutators are called *while* the loop runs, so the loop
   cannot be the caller's foreground task.

## Consequences

- The entire 003 public surface — sync getters, sync mutators, outcome enums, `Drop`
  teardown — survives the refactor byte-for-byte; no caller or integration test changes
  (spec SC-001/SC-004).
- Getters observe state through the same mutex `apply` writes through: a snapshot taken
  after a mutator returns reflects it (linearizability, spec FR-006), preserved by
  acquire-release on the single lock.
- One lock for the whole state means the event loop briefly excludes getters during each
  `apply`; at this feature's scale (in-memory ops, 0–3 topics) that is the same cost
  profile ADR 0008 already accepted.
- The node remains self-contained: own it, use it, drop it — no caller obligation to drive
  a future, no leaked tasks (spec FR-011/SC-005).
- 008's registry reader attaches via `spawn_producer` and inherits the same drop-abort
  ownership with no new mechanism.
- If a future feature needs `apply` to run without the shell (e.g. simulation/replay), the
  pure core already supports it — purity lives in `NodeState`/`apply` (ADR 0011), not in
  this sharing choice.

## Alternatives considered

- **Event loop owns `NodeState` outright; getters via query events + `oneshot` replies**:
  rejected. Every getter becomes `async` and eventually-consistent (ordered behind the
  queue), `oneshot` machinery per read, the 003 API and test suite break — purchasing a
  "single owner, no shared lock" property that matters under contention this workload does
  not have. `apply` is byte-identical under either choice, so no purity is gained.
- **Event-sourced `subscribe`/`unsubscribe`** (`Event::Subscribe { topic, reply }` through
  `apply`): rejected for this feature. The protocol is epochal — the future dialer reads
  the current subscription set on epoch tick; `subscribe` never emits effects, so routing
  it through the effect executor buys nothing, and the sync outcome return would be lost
  to a `oneshot`. Re-opened by design when registry-driven updates land (see Decision 2).
- **Caller-driven loop** (`Node::new` returns `(Node, impl Future)`, or `node.run_loop()`):
  rejected. The caller must remember to drive the future or the node is silently inert;
  drop-abort ownership gets ambiguous (the handle no longer owns the task); and the
  actor-style variant re-opens the rejected query-channel getters. Spawn-in-constructor
  matches 001–003 ergonomics and the seam commit's contract.
- **Two locks retained** (`received` + `subscriptions` separate, as on `main`): rejected —
  keeps the cross-lock ordering discipline and scatters the state ADR 0011 exists to
  consolidate; one struct, one lock, one writer story.
- **`tokio::sync::Mutex` / `RwLock` / lock-free**: rejected for the same reasons as
  ADR 0008 — the critical section is brief pure-CPU work with no `await` inside;
  `std::sync::Mutex` consistency wins.

## Sources

- `specs/event-loop-and-registry-contract.md` §1.2 (shell shape), §1.4 (the two open
  decisions delegated to this feature).
- `specs/004-node-event-loop/spec.md` — FR-004, FR-006, FR-009, FR-011; Assumptions.
- `specs/004-node-event-loop/research.md` — R4, R5, R6.
- ADR 0006 (receive-task model), ADR 0008 (subscription mutator surface — retained),
  ADR 0011 (the pure core this ADR shares and drives).

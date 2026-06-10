# Implementation Plan: Node Event-Loop Refactor

**Branch**: `004-node-event-loop` | **Date**: 2026-06-09 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/004-node-event-loop/spec.md`; shared seam
contract from [`../event-loop-and-registry-contract.md`](../event-loop-and-registry-contract.md)
(this feature is **Feature A** of that contract); design decisions resolved in pre-plan
discussion between the maintainer and the implementation agent (consolidated in
[research.md](./research.md)).

## Summary

Behavior-preserving refactor of the node's consumer side. The seam commit on `main` already
landed `Event`, `EventQueue`, `Node::events()`, `Node::spawn_producer`, the single consumer
loop, and drop-aborted producers — but the 003 message-handling logic still runs **inline in
the consumer loop** against scattered `Arc<Mutex<…>>` fields. This feature extracts that logic
into a crate-internal pure core:

- **`NodeState`** — one explicit struct holding the node's mutable state (identity,
  subscription set, received deliveries, verifier), replacing the separate
  `Arc<Mutex<Vec<ReceivedDelivery>>>` / `Arc<Mutex<HashSet<TopicId>>>` fields.
- **`apply(&mut NodeState, Event) -> Vec<Effect>`** — the single pure, synchronous
  state-transition function; dispatches each `Event` variant to a named handler function.
- **`Effect`** — the transition's outbound-command output type, shipped **uninhabited**
  (the node only ingests pre-connection); the signature is locked now so 004-connections
  (`ForwardTo`/`Dial`/`Close`) and 008's `RegistryUpdate` arm slot in without reshaping
  the contract.
- The shell (`Node`) keeps the queue, the spawned event loop (now `apply` + vacuous effect
  execution), producer ownership, and the unchanged public API: sync lock-and-clone getters
  and sync `subscribe`/`unsubscribe` delegating to `NodeState` methods.

All observable 002/003 behavior is preserved; the existing integration suite passes
unmodified (spec SC-001). No new public API is added (spec SC-004, Clarifications).

## Technical Context

**Language/Version**: Rust, edition 2021, `rust-version = 1.75` (unchanged)

**Primary Dependencies**: `tokio` (runtime, mpsc queue, task ownership — ADR 0001),
`tracing` (operator logs — ADR 0003). No new dependencies; no `Cargo.toml` change.

**Storage**: N/A (in-memory state only)

**Testing**: `cargo test` — existing integration suite under `tests/` (unchanged, the parity
gate) plus new in-module unit tests for the pure core (`src/state.rs`); `proptest` available
if a property formulation is natural, not required by this feature.

**Target Platform**: same as 001–003 (local development hosts; in-process `InMemoryNetwork`)

**Project Type**: single Rust crate — library + thin CLI binary

**Performance Goals**: none introduced; refactor must not change the asymptotics of the
receive path (same per-event work, one lock acquisition per event instead of up to two)

**Constraints**: behavioral parity with 002/003 (spec FR-001..007); pure core must be
exercisable with no async runtime (spec FR-008, SC-002); crate-internal core, no new public
API (spec Clarifications, SC-004); seam items already public on `main` (`Event`,
`EventQueue`, `events()`, `spawn_producer`) are kept as-is

**Scale/Scope**: reshape of `src/node.rs` (385 lines) into shell + new `src/state.rs` pure
core; no other module's behavior changes; ~2 ADRs; existing 9 integration-test files
untouched

## Constitution Check

*GATE: evaluated before Phase 0; re-evaluated after Phase 1 design — both pass.*

- **I. Correctness Over Optimization** — ✅ Every behavior in this plan traces to: spec.md
  FR-001..016 (parity + structure), `specs/event-loop-and-registry-contract.md` §1/§3/§5
  (the seam and test strategy), ADR 0011 / ADR 0012 (structural decisions, authored with
  this plan), and prior ADRs 0006–0008 (receive task, network handle, subscription mutator
  shape) whose decisions this refactor preserves or supersedes explicitly.
- **II. Test-Driven for Correctness Claims** — ✅ The feature adds **no new protocol
  claims**; the protocol-behavior guarantees (topic filtering, signature verification) are
  already articulated by the existing 002/003 test suite, which stays green at every
  checkpoint — the tests precede the implementation by construction. The new pure-core unit
  tests re-articulate the same guarantees at the state-machine level and are written
  **before** the corresponding logic is extracted (red is not expected — parity means they
  pass against ported logic — but the tests-before-code ordering is preserved in task
  ordering). `/speckit-tasks` MUST order state-machine test tasks before extraction tasks.
- **III. Document Structural Decisions as ADRs** — ✅ Two ADRs authored with this plan:
  - **ADR 0011** `docs/decisions/0011-pure-state-transition-core.md` — `NodeState` +
    pure `apply` + uninhabited `Effect`, named per-variant handlers, ambient-logging
    carve-out.
  - **ADR 0012** `docs/decisions/0012-node-state-sharing-and-lifecycle.md` —
    `Arc<Mutex<NodeState>>` sharing (loop = sole event-driven writer, getters lock-and-clone),
    spawn-in-constructor + drop-abort lifecycle; rejected query-channel and caller-driven
    `run_loop()` alternatives; sync `subscribe`/`unsubscribe` retention (extends ADR 0008)
    with the 008+ deprecation path.
- **IV. Specifications as Ambiguity Detectors** — ✅ No spec ambiguity encountered during
  planning. One deliberate, documented deviation-of-emphasis from the contract doc's
  illustrative sketch (it shows `pub` items; the clarified spec makes the core crate-internal)
  is recorded in ADR 0011 — the contract's *normative* seam (§3) is unaffected.
- **V. Specifications Are Read-Only** — ✅ No edits to `pubsub/docs/` or
  `pubsub/formal_spec/`. `specs/event-loop-and-registry-contract.md` is a tracked
  workstream doc in this crate (agent-editable), but this plan does not need to change it.

**Engineering Standards applied**: logs are operator UX — pure-core tests assert on state
and returned effects, never log content; log emission moves with the logic unchanged
(ADR 0011's ambient-effect carve-out). Operator-facing strings unchanged and remain
implementation-neutral. Parse at the edge — untouched; `Node::new` continues to take
already-parsed values. Forward-compatible interface — the `-> Vec<Effect>` signature and
uninhabited `Effect` are justified by named ROADMAP consumers (004-connections fan-out/dial;
008's `RegistryUpdate` arm), per the contract doc, not speculative generality.

## Project Structure

### Documentation (this feature)

```text
specs/004-node-event-loop/
├── spec.md              # /speckit-specify output (+ clarifications)
├── plan.md              # This file
├── research.md          # Phase 0: consolidated pre-plan decisions
├── data-model.md        # Phase 1: NodeState / Event / Effect model
├── quickstart.md        # Phase 1: how to exercise the pure core + queue plumbing
├── contracts/
│   └── public-surface.md  # Phase 1: public-API-unchanged contract + crate-internal core
├── checklists/
│   └── requirements.md  # Spec quality checklist (complete)
└── tasks.md             # /speckit-tasks output (NOT created by /speckit-plan)
```

ADRs (authored with this plan, live outside the feature dir):

```text
docs/decisions/
├── 0011-pure-state-transition-core.md
└── 0012-node-state-sharing-and-lifecycle.md
```

### Source Code (repository root)

```text
src/
├── state.rs        # NEW — crate-internal pure core:
│                   #   pub(crate) struct NodeState { self_id, subscriptions, received, verifier }
│                   #   pub(crate) enum Effect {}              (#[non_exhaustive], uninhabited)
│                   #   pub(crate) fn apply(&mut NodeState, Event) -> Vec<Effect>
│                   #   fn handle_message_received(...) -> Vec<Effect>   (named per-variant handler)
│                   #   NodeState::{subscribe, unsubscribe, subscriptions_snapshot, received_snapshot}
│                   #   #[cfg(test)] mod tests — synchronous state-machine unit tests
├── node.rs         # RESHAPED — async shell only:
│                   #   Node { handle, peers, state: Arc<Mutex<NodeState>>, events, event_loop, producers }
│                   #   event loop: recv → apply → execute effects (vacuous match until 004-connections)
│                   #   named producer fn network_mailbox_loop(queue, rx) replacing the inline closure
│                   #   public surface unchanged: new/send/id/peers/events/spawn_producer/
│                   #     received_messages/subscriptions/subscribe/unsubscribe + Drop (abort loop+producers)
├── event.rs        # UNCHANGED — Event, EventQueue (the seam, already on main)
├── lib.rs          # `mod state;` added (NOT re-exported); pub use list unchanged
├── config.rs       # UNCHANGED
├── crypto/         # UNCHANGED
├── error.rs        # UNCHANGED
├── main.rs         # UNCHANGED (Node::new signature preserved)
├── message.rs      # UNCHANGED
├── network.rs      # UNCHANGED
├── peer.rs         # UNCHANGED
├── received.rs     # UNCHANGED — ReceivedDelivery
└── topic.rs        # UNCHANGED

tests/              # UNCHANGED — all 9 integration files are the parity gate (SC-001)
```

**Structure Decision**: one new crate-internal module `src/state.rs` holds the entire pure
core plus its synchronous unit tests (in-module, per the crate-internal clarification);
`src/node.rs` shrinks to the async shell. `Effect` lives in `state.rs` beside `apply`
(crate-internal, uninhabited) rather than in the public `event.rs`, keeping the public seam
exactly as `main` has it. The `Node`'s duplicate `verifier` field (currently
`#[allow(dead_code)]`) is removed — the verifier's canonical owner becomes `NodeState`.

## Design Notes (decision record pointers)

All decided pre-plan and consolidated in [research.md](./research.md); structural rationale
in ADR 0011 / ADR 0012:

1. **Pure core, crate-internal** — `NodeState` + `apply` + named per-variant handlers
   (`handle_message_received`); US2 tests are in-module unit tests. (ADR 0011)
2. **`Effect` uninhabited, signature locked** — `#[non_exhaustive] pub(crate) enum Effect {}`;
   shell's effect-execution match is `match effect {}` (vacuous) until 004-connections.
   (ADR 0011)
3. **Ambient-logging carve-out** — `apply` is pure w.r.t. state + protocol effects; inline
   `tracing` calls (`message_dropped`, subscription events) move with the logic and are not
   modeled as effects nor asserted in tests. (ADR 0011)
4. **State sharing** — `Arc<Mutex<NodeState>>`; event loop is the sole event-driven writer;
   getters lock-and-clone synchronously. Query-channel alternative rejected. (ADR 0012)
5. **`subscribe`/`unsubscribe`** — stay sync public methods delegating to `NodeState`
   methods; event-sourcing rejected (epochal protocol: dialers read subscription state on
   tick; subscribe never emits effects). Registry-driven events (008+) are the expected
   eventual replacement. (ADR 0012, extending ADR 0008)
6. **Lifecycle** — event loop spawned in `Node::new`; `Drop` aborts loop + producers
   (seam behavior preserved). Caller-driven `run_loop()` rejected. (ADR 0012)
7. **Producers as named async fns** — `network_mailbox_loop` replaces the inline closure in
   `Node::new`; future producers (008 registry reader) follow the same convention.

## Complexity Tracking

No constitution violations; table omitted.

# Tasks: Node Event-Loop Refactor

**Input**: Design documents from `specs/004-node-event-loop/`

**Prerequisites**: plan.md, spec.md (FR-001..016, SC-001..005, US1–US3), research.md (R1–R9),
data-model.md, contracts/public-surface.md, quickstart.md, ADR 0011, ADR 0012

**Tests**: MANDATORY ordering per Constitution Principle II and the plan's Constitution
Check — the state-machine test tasks (T003) precede the logic port (T004) and the shell
switch (T005); T003's tests fail before T004 lands (the handler stub records nothing).
**No new integration tests; `tests/` stays untouched** (SC-001; IMPLEMENTATION_NOTES N-006).

**ADRs**: No new structural decisions expected — ADR 0011 / ADR 0012 were authored at plan
time. If task execution surfaces a new structural choice, stop and author the ADR first
(Constitution Principle III).

**Execution order note**: phases are listed by story priority (US1 = P1 parity gate), but
**execution follows the green-checkpoint sequence**: Foundational + US2 (pure core) → US1
(shell switch; parity gate) → US3 (seam tidy-up). The parity gate (US1) cannot run before
the core it switches to exists; US1 remains the *acceptance* MVP — it is simply verified
second. Each checkpoint commit leaves `cargo fmt && cargo build && cargo clippy
--all-targets && cargo test` green (Constitution: green checkpoints; saved convention:
fmt in every sweep).

## Format: `[ID] [P?] [Story] Description`

---

## Phase 1: Setup

**Purpose**: Record the parity baseline this refactor must preserve.

- [X] T001 Verify baseline green on branch `004-node-event-loop`: run `cargo fmt --check && cargo build && cargo clippy --all-targets && cargo test` at the crate root; confirm all 9 integration files under `tests/` pass. This is the SC-001 baseline — `tests/` is not edited by any subsequent task.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The crate-internal pure-core module every story builds on (plan §Structure Decision; ADR 0011).

- [X] T002 Create `src/state.rs` skeleton and wire the module: `pub(crate) struct NodeState { self_id: PeerId, subscriptions: HashSet<TopicId>, received: Vec<ReceivedDelivery>, verifier: Arc<dyn Verifier> }` with `NodeState::new(self_id, initial_subscriptions, verifier)` and snapshot accessors (`received_snapshot()`, `subscriptions_snapshot()`); `#[non_exhaustive] pub(crate) enum Effect {}` (uninhabited — rustdoc states variants arrive with the connection model, no FR citations in rustdoc); `pub(crate) fn apply(state: &mut NodeState, event: Event) -> Vec<Effect>` dispatching via `match` to a private `handle_message_received(state, from, message) -> Vec<Effect>` stub that returns `Vec::new()` without recording anything (T003's tests must fail against it). Add non-pub `mod state;` to `src/lib.rs` with a temporary `#[allow(dead_code)]` and a `// removed in shell-switch task (T005/T006)` comment. Source `//` comments may cite FR-008/FR-013. Compiles green; shell untouched.

**Checkpoint**: pure-core skeleton exists, unused by the shell; suite still green.

---

## Phase 3: User Story 2 — Pure synchronous state machine (Priority: P2) — **executed first**

**Goal**: The message-handling logic exercisable as a synchronous state machine — no async runtime, no channels, no tasks (FR-008; SC-002).

**Independent Test**: in-module unit tests in `src/state.rs` compile and pass with zero tokio/async machinery (spec US2 Independent Test, crate-internal per Clarifications).

### Tests for User Story 2 (MANDATORY — written first, MUST fail before T004)

- [X] T003 [US2] Write `#[cfg(test)] mod tests` in `src/state.rs`: synchronous state-machine tests constructing `NodeState` (with `crypto::mock` verifier) and applying scripted `Vec<Event>` sequences, asserting after **each** `apply` on both resulting state and returned effects (contract doc §5 primary approach). Cover: accept on subscribed topic + valid signature (delivery appended in order — FR-001); drop off-topic (state unchanged — FR-002, US2-AS2); drop invalid signature on subscribed topic (state unchanged — FR-003, using a rejecting/mismatched-key mock verifier); effects list empty on every path (FR-013); empty-subscription-set node drops everything (spec Edge Cases); determinism — same initial state + same sequence twice ⇒ identical final state (US2-AS3); `NodeState::subscribe`/`unsubscribe` outcome pairs (Added/AlreadyPresent, Removed/NotSubscribed — FR-004). **No log assertions** (constitution: logs are operator UX). Tests fail against T002's stub. Source comments may cite FRs; test names read behaviorally.

### Implementation for User Story 2

- [X] T004 [US2] Port the 002/003 logic from the consumer loop in `src/node.rs:114–178` into `src/state.rs` to make T003 pass: `handle_message_received` applies the topic filter **before** signature verification (FR-003 ordering; cheap check first), pushes `ReceivedDelivery { from, message }` on accept, and carries the `tracing` emissions verbatim (debug `recv`; info `message_dropped` with `cause = "topic_not_subscribed"` / `"invalid_signature"` and existing fields — ambient-effect carve-out, ADR 0011); `NodeState::subscribe`/`unsubscribe` carry the insert/remove + outcome logic and their `topic_subscribed`/`topic_unsubscribed`/noop tracing events from `src/node.rs:295–359`. The shell still runs its old inline logic (unchanged this task). **Checkpoint commit #1**: fmt + build + clippy + test green; suite untouched.

**Checkpoint**: pure core complete and unit-tested; shell not yet switched.

---

## Phase 4: User Story 1 — Identical messaging behavior (Priority: P1) 🎯 acceptance MVP

**Goal**: The shell runs on the pure core with **zero observable change** (FR-001..007, FR-015, FR-016 parity; SC-001/SC-004).

**Independent Test**: the unchanged 002/003 integration suite passes without modification (spec US1 Independent Test).

### Implementation for User Story 1

- [X] T005 [US1] Switch the shell in `src/node.rs`: replace the `received: Arc<Mutex<Vec<ReceivedDelivery>>>` and `subscriptions: Arc<Mutex<HashSet<TopicId>>>` fields with a single `state: Arc<Mutex<NodeState>>` (ADR 0012); `Node::new` builds `NodeState::new(...)` from its existing parameters — construction ordering preserved: network registration still precedes any spawn (FR-016 no-leak-on-failure; research R9); the event-loop body becomes lock → `apply` → execute effects via `for effect in effects { match effect {} }` (vacuous — uninhabited `Effect`); `received_messages()`, `subscriptions()`, `subscribe()`, `unsubscribe()` become thin lock-takers delegating to `NodeState` (public signatures and outcomes unchanged — contracts §A); delete the now-dead inline filter/verify/log block from the loop. Remove the temporary `#[allow(dead_code)]` on `mod state` in `src/lib.rs`. (Node's `#[allow(dead_code)] verifier` field is removed in T007, per the checkpoint slicing.)
- [X] T006 [US1] Parity gate: run the full untouched suite — `cargo fmt && cargo build && cargo clippy --all-targets && cargo test`; all 9 integration files pass **unmodified** (SC-001) and T003's unit tests stay green. Spot-check linearizability behavior via the existing `topic_runtime.rs` runtime-subscription tests (FR-006). **Checkpoint commit #2.**

**Checkpoint**: refactor functionally complete; US1 + US2 delivered; parity proven.

---

## Phase 5: User Story 3 — One extension seam (Priority: P3)

**Goal**: Producers and event kinds attach at the seam in the named-function shape 008 will mirror (FR-010/012; SC-003; research R7/R9).

### Implementation for User Story 3

- [ ] T007 [US3] In `src/node.rs`: extract the inline network-producer closure in `Node::new` (currently `src/node.rs:201–209`) into a named `async fn network_mailbox_loop(queue: EventQueue, rx: UnboundedReceiver<RoutingFrame>)`, registered through the same `spawn_producer` call (mechanism unchanged — contracts §A); remove `Node`'s `#[allow(dead_code)] verifier` field (NodeState is the canonical owner — research R9).
- [ ] T008 [US3] Verify the seam and visibility contract against code (contracts §B + §C steps 1–2): `git -C /Users/ezequiel/IOG/CBU/pubsub/pubsub diff main -- pubsub-node/src/lib.rs` shows only the `mod state;` addition (no `pub use` change); `grep -n "pub " src/state.rs` shows `pub(crate)` items only; `src/event.rs` untouched (`git diff main -- pubsub-node/src/event.rs` empty); `events()`/`spawn_producer` signatures unchanged; review the `Drop` impl in the `src/node.rs` diff vs `main` — abort-loop-and-producers behavior unchanged (FR-011/SC-005, contracts §A Drop row; analysis C2). Note on US3's Independent Test (analysis C1): no no-op-producer test is written in this feature — the producer mechanism is exercised indirectly on every integration test via the network mailbox (itself registered through `spawn_producer`), and **008's registry reader is the implicit tester**: it is exactly the "additional producer" US3 describes, so 008's integration tests realize US3-AS1 a-posteriori. **Checkpoint commit #3.**

**Checkpoint**: all three stories delivered; seam in the documented shape for 008.

---

## Phase 6: Polish & Cross-Cutting

- [ ] T009 [P] Rustdoc pass on `src/node.rs` and `src/state.rs`: `Node`'s docs describe the loop→transition structure in stable library terms; `state.rs` module docs address crate-internal readers; **no FR/spec citations in rustdoc** (constitution: implementation-neutral operator/library strings; FR refs live in `//` source comments only).
- [ ] T010 [P] Walk `specs/004-node-event-loop/quickstart.md` end-to-end: parity sweep command, the in-module test pattern matches what T003 actually wrote (update quickstart snippets if names drifted), queue-level pattern still works as documented.
- [ ] T011 Final full sweep (`cargo fmt && cargo build && cargo clippy --all-targets && cargo test`) and self-check the remaining contracts §C items ahead of the formal `/speckit-analyze` round (which records findings in this feature's `analysis.md` per the constitution's analysis-ledger rule). Final commit.

---

## Dependencies & Execution Order

```text
T001 (baseline)
  └─ T002 (skeleton)
       └─ T003 (US2 tests, fail) ─ T004 (US2 port, tests pass)  ← checkpoint commit #1
            └─ T005 (US1 shell switch) ─ T006 (US1 parity gate) ← checkpoint commit #2
                 └─ T007 (US3 producer + field) ─ T008 (US3 seam check) ← checkpoint commit #3
                      └─ T009 [P] / T010 [P] ─ T011 (final)     ← final commit
```

- Strictly sequential through T008 — every task edits or gates `src/node.rs`/`src/state.rs`/`src/lib.rs`, so there is no intra-story parallelism (single crate, overlapping files, one developer).
- T009 and T010 are the only [P] pair (different files: src rustdoc vs. quickstart walk).
- Story completion order: **US2 → US1 → US3** (see Execution order note); acceptance priority order remains US1 > US2 > US3.

## Implementation Strategy

- **Checkpoint = commit**: four commits total (T004, T006, T008, T011), each leaving the repo green with `tests/` untouched — bisectable per the constitution's logical-increments rule.
- **MVP**: checkpoint commit #2 (pure core + parity-proven shell) is the mergeable core of the feature; #3 is the seam tidy-up 008 mirrors; polish closes documentation and §C self-checks.
- **Stop-the-line rule**: if any task forces a change inside `tests/`, a new public item, a log event rename, or a new structural decision — stop; that contradicts spec FR-007/FR-013/SC-001/SC-004 or Principle III and needs maintainer review, not a workaround.

## Notes

- No new integration tests; the construction-failure test is deferred to 004-connections (IMPLEMENTATION_NOTES N-006).
- `Event::RegistryUpdate` is NOT pre-created — variant + payload + arm are Feature B's (008) per the seam contract §3 ownership split (FR-014).
- Operator-facing strings (log events, CLI/stderr text) keep their exact names and fields — contracts §A list; review-enforced, not test-enforced.

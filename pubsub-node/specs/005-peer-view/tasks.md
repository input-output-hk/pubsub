---

description: "Task list for feature 005 — seeded bounded connection-selection and acceptance strategies"
---

# Tasks: Seeded bounded connection-selection and acceptance strategies

**Input**: Design documents from `specs/005-peer-view/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R7), data-model.md, contracts/ (strategy-traits, connection-control)

**Tests**: MANDATORY. Correctness/protocol-behaviour claims (determinism FR-003/SC-001, bound FR-001/SC-002, unbiasedness FR-007/SC-004, acceptance + explicit rejection FR-010/FR-011, rejection dropping the pending upstream FR-014) — designated **critical** in plan.md, so test tasks precede implementation and MUST fail first (Constitution II).

**ADRs**: 0024 (seeded PRNG bounded selection over ordered inputs; SHA-256 only as the PRNG-seed KDF), 0025 (acceptance-seam return evolution + `ConnectionAction::Rejected`). Numbers provisional (next free after 0023) — coordinate with the refactor branch.

**Note (PR-73 simplification)**: the earlier sticky-failed-set / rejection-counter / candidates-minus-failed / `ConnectionSetup`-driven back-fill machinery was dropped. The dialer's reaction to a `Rejected` is now minimal — remove the matching pending `AwaitingAccept` upstream only. Retry-to-a-minimum (back-fill) is deferred to a future strategy family (`BackfillingSeededBoundedConnection` / `RetryingSeededBoundedConnection`), out of scope for 005. Completed tasks below keep their `[X]` and IDs but describe what actually shipped.

**Dependency (coordination, not a hard block)**: the broader determinism/purity refactor (strategies-as-`apply`-arguments, deterministic scheduling, decouple flag) is the co-developing architect's separate workstream. This feature does **not** block on it: it applies **ordered structures (`BTreeSet`/`BTreeMap`) to its own state within this PR** and keeps strategy objects pure (it may retain the current strategy injection and migrate later). Tasks marked **[coordinate]** touch files/shapes the parallel refactor also touches (strategy injection sites, `NodeState`) — sync to avoid conflicting edits; they are not gated on the refactor merging.

**Organization**: by user story. US1 (bounded selection) is the MVP; US2 adds bounded acceptance + `Rejected` (dialer drops the matching pending upstream only — no retry/back-fill); US3 validates seed variety + unbiasedness.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: parallelizable (different files, no incomplete dependency)
- **[Story]**: US1/US2/US3 (user-story phases only)

## Path Conventions

Single Rust project: sources under `pubsub-node/src/`, tests under `pubsub-node/tests/`. Paths repo-relative.

---

## Phase 1: Setup

- [X] T001 ADR 0024 (seeded bounded selection: seeded PRNG sampling over ordered inputs — `ChaCha20Rng` partial Fisher–Yates, SHA-256 only as the PRNG-seed KDF vs `DefaultHasher`, per-network seed / per-node derivation) in `pubsub-node/docs/decisions/0024-seeded-bounded-selection.md`. (The sticky-failed-set + `ConnectionSetup` back-fill originally captured here was dropped in the PR-73 simplification; retry-to-a-minimum is deferred to a future strategy family.)
- [X] T002 ADR 0025 (acceptance-seam evolution `bool → Admission` + current-downstream input; `ConnectionAction::Rejected` + minimal dialer handling) in `pubsub-node/docs/decisions/0025-acceptance-seam-and-rejected-action.md`
- [ ] T003 [P] Coordinate with the co-developing architect to avoid conflicting edits on shared files (strategy injection sites, `NodeState`) and align ordered-structure type choices; record the agreed types in `specs/005-peer-view/research.md` (R6). Not a gate — 005 proceeds with its own ordered structures and current strategy injection.
- [X] T004 [P] Test scaffolding: extend `ConnectionScript` with a `rejected` step and add bounded-node builder helpers (construct with `SeededBoundedConnection`/`BoundedAcceptance` + seed/upstream degree/downstream degree) in `pubsub-node/tests/common/mod.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**⚠️ CRITICAL**: must complete before US1/US2.

- [X] T005 Add the seeded PRNG sampler (`ChaCha20Rng` partial Fisher–Yates via `partial_shuffle` over the ordered candidate set; the 32-byte seed derived by SHA-256 as a KDF over length-prefixed `(tag, seed, self_id, topic)`, re-seeded per call) in `pubsub-node/src/strategies/connection/seeded_bounded.rs` — refactor-agnostic
- [X] T006 **[coordinate]** `handle_connection_setup` selects straight over `candidates` in `pubsub-node/src/state.rs`. (The originally planned `failed_upstream` `BTreeSet` + rejection counter and the candidates-minus-failed diff were dropped in the PR-73 simplification — no new `NodeState` state was added.)

**Checkpoint**: PRNG sampler + `handle_connection_setup` selecting over `candidates` in place.

---

## Phase 3: User Story 1 — Reproducible bounded upstream selection (Priority: P1) 🎯 MVP

**Goal**: a node selects ≤ upstream degree upstream peers per topic by seeded PRNG sampling; same seed + membership reproduces an identical selection.

**Independent Test**: candidates > upstream degree under seed s; rebuilt under s the selection is identical and ≤ upstream degree per topic. (Acceptance still accept-all here.)

### Tests for User Story 1 (write first; MUST fail) ⚠️

- [X] T007 [P] [US1] Unit tests for `SeededBoundedConnection` in `pubsub-node/src/strategies/connection/seeded_bounded.rs`: exactly `upstream_degree` when candidates exceed it (FR-001), all when ≤ bound (FR-002), identical output across iteration orders / repeated calls (FR-003), order-independent determinism from the fixed PRNG over the ordered candidate set (FR-008), per-node variety by `self_id` (FR-005), and the **default seed 0** path produces a deterministic, repeatable selection when no seed is supplied (FR-004)
- [X] T008 [P] [US1] **[coordinate]** Integration test in `pubsub-node/tests/connections.rs`: an N-node network under seed s forms a partial topology; rebuilt under s it is identical; upstream ≤ upstream degree (SC-001, SC-002)

### Implementation for User Story 1

- [X] T009 [US1] Implement `SeededBoundedConnection { seed, self_id, upstream_degree }` (impl `ConnectionStrategy`, using the T005 helper) and re-export it from `pubsub-node/src/lib.rs` — in `pubsub-node/src/strategies/connection/seeded_bounded.rs` — refactor-agnostic
- [X] T010 [US1] Parse seed + upstream degree at the edge and select the bounded vs unbounded selection strategy in `pubsub-node/src/config.rs` and `pubsub-node/src/main.rs`
- [X] T011 [US1] **[coordinate]** Supply the selection strategy (with `self_id`) at node construction (current injection; align with the refactor's eventual argument shape) in `pubsub-node/src/node.rs`

**Checkpoint**: US1 functional — bounded, reproducible selection with accept-all acceptance.

---

## Phase 4: User Story 2 — Bounded acceptance, explicit rejection (Priority: P2)

**Goal**: accept inbound up to downstream degree; over capacity send an explicit `Rejected` (not misbehaviour); on the dialer a `Rejected` drops the matching pending upstream only — no retry/back-fill (deferred to a future strategy family), so the realized upstream degree may settle below target.

**Independent Test**: drive a node past its downstream degree → exactly downstream degree accepted, the rest dropped with the over-capacity cause + `Rejected`, no severance. On the dialer, a `Rejected` removes the matching `AwaitingAccept` and does nothing further; the realized degree may under-fill.

### Tests for User Story 2 (write first; MUST fail) ⚠️

- [X] T012 [P] [US2] Unit tests for `BoundedAcceptance`/`Admission` in `pubsub-node/src/strategies/acceptance/bounded.rs`: `RejectMembership` when not membership-valid, `RejectOverCapacity` at/above downstream degree, `Accept` below (FR-010)
- [X] T013 [P] [US2] **[coordinate]** Integration test in `pubsub-node/tests/connections.rs`: node at downstream degree drops the extra request with the over-capacity cause, sends `Rejected`, records no downstream entry, emits no `Misbehaved`/`Terminated` (FR-011)
- [X] T014 [P] [US2] **[coordinate]** Integration test in `pubsub-node/tests/connections.rs`: an explicit `Rejected` removes the matching pending `AwaitingAccept` upstream and produces no further effects (no retry/back-fill); the realized upstream degree may settle below target (under-fill) (FR-014, FR-015). (The originally planned sticky-failed / next-`ConnectionSetup` re-selection / rejection-counter assertions were dropped in the PR-73 simplification.)

### Implementation for User Story 2

- [X] T015 [US2] Evolve `ConnectionAcceptanceStrategy`: `accepts -> bool` → `admit(...) -> Admission { Accept, RejectMembership, RejectOverCapacity }` taking the current downstream view; map `AcceptFromAllCandidates`; re-export `Admission` from `lib.rs` — in `pubsub-node/src/strategies/acceptance/mod.rs` (+ `accept_from_all.rs`)
- [X] T016 [US2] Implement `BoundedAcceptance { downstream_degree }` (re-export from `lib.rs`); parse downstream degree at the edge and select bounded vs unbounded in `pubsub-node/src/strategies/acceptance/bounded.rs`, `config.rs`, `main.rs`
- [X] T017 [US2] Add `ConnectionAction::Rejected { topic }` in `pubsub-node/src/message.rs`; amend `handle_connection_request` so `RejectOverCapacity` logs `downstream_capacity_reached` + sends `Rejected`, `RejectMembership` stays a silent drop, in `pubsub-node/src/state.rs`
- [X] T018 [US2] **[coordinate]** Add `handle_connection_rejected` (remove the matching `AwaitingAccept` upstream only; `unsolicited_reject` drop otherwise) in `pubsub-node/src/state.rs`. (The originally planned `failed_upstream` insert + rejection-counter increment were dropped in the PR-73 simplification.)
- [X] T019 [US2] Superseded by the PR-73 simplification: no explicit-rejection-count getter is exposed. Observability is the upstream/downstream snapshots only (FR-016); the `rejections_received` `NodeState` getter and its `Node` passthrough getter were removed.

**Checkpoint**: US1 + US2 work independently; bounded acceptance and explicit rejection observable (via the upstream/downstream snapshots).

---

## Phase 5: User Story 3 — Seed-varied, identity-unbiased selection (Priority: P3)

**Goal**: distinct seeds yield differing selections; over a seed sweep no candidate is systematically preferred.

**Independent Test**: two seeds → differing selections for candidates > upstream degree; per-candidate frequency over ≥1,000 seeds uniform within tolerance.

### Tests for User Story 3 (write first; MUST fail) ⚠️

- [X] T020 [P] [US3] Distinct-seed divergence test in `pubsub-node/src/strategies/connection/seeded_bounded.rs` (or `tests/`): two seeds → differing selections for candidates > upstream degree (SC-003)
- [X] T021 [P] [US3] Seed-sweep uniformity test (≥1,000 fixed seeds; per-candidate frequency within tolerance / chi-square gate p < 0.001 per research R5) as a fixed seeded loop (research R5: proptest or seeded loop) in `pubsub-node/src/strategies/connection/seeded_bounded.rs`

### Implementation for User Story 3

- [X] T022 [US3] If the sweep reveals bias, adjust the sampling (T005 PRNG sampler) so selection is unbiased (FR-007); otherwise record that the PRNG (partial Fisher–Yates) already satisfies SC-004 — in `pubsub-node/src/strategies/connection/seeded_bounded.rs`

**Checkpoint**: all three stories validated.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [X] T023 [P] Record deferred items in `pubsub-node/specs/IMPLEMENTATION_NOTES.md`: the experiment/testing framework, dynamic re-selection on membership change + epochal rotation, golden nodes (push-based M2), bounded/seeded fan-out, any timeout/no-response re-dial; and resolve the N-007 "revisit at 005 (PeerView)" pointer — note candidates serve as the peer view (no `PeerView` abstraction added)
- [X] T024 [P] Regression check for SC-005: with bounded params absent, the existing full-mesh dissemination/connection suites stay green (no new code path engaged)
- [X] T025 Run the `specs/005-peer-view/quickstart.md` validation end-to-end
- [X] T026 `/speckit-analyze` consistency pass; record findings in `specs/005-peer-view/analysis.md`
- [X] T027 Two-phase strategy construction (ADR 0028, FR-019): in `src/strategies/config.rs` add per-seam params (`ConnectionParams`, `AcceptanceParams`) + `StrategyConfigError` + the aggregate two-phase builder (`NodeStrategies` / `NodeStrategiesBuilder` — phase 1 holds the resolved kinds, phase 2 `build(&ConnectionParams, &AcceptanceParams)`); give `ConnectionStrategyKind`/`AcceptanceStrategyKind` a fallible `build(&SeamParams)` that validates only its seam's required params; refactor `main.rs` to one aggregate build that maps the error once (no per-strategy validation, repetition, or branching at the edge). Unit tests for each kind's build.
- [X] T028 Module grouping (ADR 0029): move all strategy policy under `src/strategies/` (`connection`/`acceptance`/`fanout` seams + `config`); extract connection lifecycle state (`UpstreamState`, `test_support`) to a core `src/connection_state.rs` (`Admission` stays with the acceptance seam). Re-point `lib.rs` re-exports while preserving public names; update `node.rs`/`state.rs` import paths. Move-only, behaviour-preserving — full suite + clippy + fmt green.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no code dependencies. T003 is coordination only — it does **not** gate the **[coordinate]** tasks; they proceed with 005's own ordered structures and current strategy injection.
- **Foundational (Phase 2)**: T005 self-contained; T006 **[coordinate]** (touches `NodeState`). Blocks US1/US2.
- **US1 (Phase 3)**: depends on Foundational. MVP. T009/T010 self-contained; T008/T011 **[coordinate]** (touch shared files).
- **US2 (Phase 4)**: depends on Foundational; shares the PRNG sampler. T015/T016/T017 author the types; T018 **[coordinate]** wires the dialer transition (drop the matching pending upstream), which needs `Rejected` (T017) first.
- **US3 (Phase 5)**: depends on US1's selection (T009). Independent of US2.
- **Polish (Phase 6)**: after the desired stories.

### Within Each Story

- Tests first and FAIL before implementation (critical feature).
- US2: the seam change (T015) precedes `BoundedAcceptance` (T016); `Rejected` (T017) precedes the dialer's rejection handler (T018).

### Parallel Opportunities

- T003, T004 in Setup.
- Test groups: T007 (US1); T012–T014 (US2); T020–T021 (US3) — parallel within each story.
- Self-contained pieces (T005, T009, T015 types, T017 type) have no coordination concern; **[coordinate]** wiring/integration touch files the parallel refactor also edits — sync to avoid conflicts (not blocked on it).

---

## Parallel Example: User Story 2 tests

```bash
Task: "Unit test BoundedAcceptance/Admission in src/strategies/acceptance/bounded.rs"   # T012
Task: "Integration test over-capacity → Rejected, no severance"               # T013
Task: "Integration test Rejected drops the matching pending upstream only"    # T014
```

---

## Implementation Strategy

### MVP First (US1)

Setup → Foundational → US1 → validate: bounded, reproducible selection with accept-all. Demonstrable on its own.

### Incremental Delivery

1. Self-contained core (PRNG sampler + `SeededBoundedConnection` + `BoundedAcceptance`/`Admission` + `Rejected` type + their unit tests) — no coordination needed.
2. The wiring + integration tests (**[coordinate]**) — land independently using 005's ordered structures + current injection, syncing with the parallel refactor to avoid conflicts. Completes US1 then US2.
3. US3 seed-sweep validation.

### Notes

- [P] = different files, no incomplete dependency.
- Verify each test fails before implementing.
- Green checkpoint per commit (Constitution).
- Assertions read getters/snapshots, never log strings.

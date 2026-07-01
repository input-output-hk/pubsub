---

description: "Task list for feature 005 — seeded bounded connection-selection and acceptance strategies"
---

# Tasks: Seeded bounded connection-selection and acceptance strategies

**Input**: Design documents from `specs/005-peer-view/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R7), data-model.md, contracts/ (strategy-traits, connection-control)

**Tests**: MANDATORY. Correctness/protocol-behaviour claims (determinism FR-003/SC-001, bound FR-001/SC-002, unbiasedness FR-007/SC-004, acceptance + explicit rejection FR-010/FR-011, sticky back-fill FR-014) — designated **critical** in plan.md, so test tasks precede implementation and MUST fail first (Constitution II).

**ADRs**: 0024 (seeded bounded selection + stable digest), 0025 (acceptance-seam return evolution + `ConnectionAction::Rejected`). Numbers provisional (next free after 0023) — coordinate with the refactor branch.

**Dependency (coordination, not a hard block)**: the broader determinism/purity refactor (strategies-as-`apply`-arguments, deterministic scheduling, decouple flag) is the co-developing architect's separate workstream. This feature does **not** block on it: it applies **ordered structures (`BTreeSet`/`BTreeMap`) to its own state within this PR** and keeps strategy objects pure (it may retain the current strategy injection and migrate later). Tasks marked **[coordinate]** touch files/shapes the parallel refactor also touches (strategy injection sites, `NodeState`) — sync to avoid conflicting edits; they are not gated on the refactor merging.

**Organization**: by user story. US1 (bounded selection) is the MVP; US2 adds bounded acceptance + `Rejected` + sticky back-fill; US3 validates seed variety + unbiasedness.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: parallelizable (different files, no incomplete dependency)
- **[Story]**: US1/US2/US3 (user-story phases only)

## Path Conventions

Single Rust project: sources under `pubsub-node/src/`, tests under `pubsub-node/tests/`. Paths repo-relative.

---

## Phase 1: Setup

- [X] T001 ADR 0024 (seeded bounded selection: keyed-hash ranking, stable SHA-256 digest vs `DefaultHasher`, per-network seed / per-node derivation, sticky failed-set + `ConnectionSetup` back-fill) in `pubsub-node/docs/decisions/0024-seeded-bounded-selection.md`
- [X] T002 ADR 0025 (acceptance-seam evolution `bool → Admission` + current-downstream input; `ConnectionAction::Rejected` + dialer failed-mark) in `pubsub-node/docs/decisions/0025-acceptance-seam-and-rejected-action.md`
- [ ] T003 [P] Coordinate with the co-developing architect to avoid conflicting edits on shared files (strategy injection sites, `NodeState`) and align ordered-structure type choices; record the agreed types in `specs/005-peer-view/research.md` (R6). Not a gate — 005 proceeds with its own ordered structures and current strategy injection.
- [X] T004 [P] Test scaffolding: extend `ConnectionScript` with a `rejected` step and add bounded-node builder helpers (construct with `SeededBoundedSelection`/`BoundedAcceptance` + seed/upstream degree/downstream degree) in `pubsub-node/tests/common/mod.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**⚠️ CRITICAL**: must complete before US1/US2.

- [X] T005 Add the deterministic keyed-hash ranking helper (stable SHA-256 over length-prefixed `(seed, self_id, topic, candidate_id)`, lowest-k, `candidate_id` tie-break) in `pubsub-node/src/connection/seeded_bounded.rs` — refactor-agnostic
- [X] T006 **[coordinate]** Add `failed_upstream` (`BTreeSet<(PeerId, TopicId)>`) + a rejection counter to `NodeState`, and select over the **viable** view (`candidates` minus `failed_upstream`) in `handle_connection_setup` (no behaviour change while empty) in `pubsub-node/src/state.rs`

**Checkpoint**: ranking helper + viable-candidate diff in place.

---

## Phase 3: User Story 1 — Reproducible bounded upstream selection (Priority: P1) 🎯 MVP

**Goal**: a node selects ≤ upstream degree upstream peers per topic by seeded deterministic ranking; same seed + membership reproduces an identical selection.

**Independent Test**: candidates > upstream degree under seed s; rebuilt under s the selection is identical and ≤ upstream degree per topic. (Acceptance still accept-all here.)

### Tests for User Story 1 (write first; MUST fail) ⚠️

- [X] T007 [P] [US1] Unit tests for `SeededBoundedSelection` in `pubsub-node/src/connection/seeded_bounded.rs`: exactly `upstream_degree` when candidates exceed it (FR-001), all when ≤ bound (FR-002), identical output across iteration orders / repeated calls (FR-003), deterministic `candidate_id` tie-break (FR-008), per-node variety by `self_id` (FR-005), and the **default seed 0** path produces a deterministic, repeatable selection when no seed is supplied (FR-004)
- [X] T008 [P] [US1] **[coordinate]** Integration test in `pubsub-node/tests/connections.rs`: an N-node network under seed s forms a partial topology; rebuilt under s it is identical; upstream ≤ upstream degree (SC-001, SC-002)

### Implementation for User Story 1

- [X] T009 [US1] Implement `SeededBoundedSelection { seed, self_id, upstream_degree }` (impl `ConnectionStrategy`, using the T005 helper) and re-export it from `pubsub-node/src/lib.rs` — in `pubsub-node/src/connection/seeded_bounded.rs` — refactor-agnostic
- [X] T010 [US1] Parse seed + upstream degree at the edge and select the bounded vs unbounded selection strategy in `pubsub-node/src/config.rs` and `pubsub-node/src/main.rs`
- [X] T011 [US1] **[coordinate]** Supply the selection strategy (with `self_id`) at node construction (current injection; align with the refactor's eventual argument shape) in `pubsub-node/src/node.rs`

**Checkpoint**: US1 functional — bounded, reproducible selection with accept-all acceptance.

---

## Phase 4: User Story 2 — Bounded acceptance, explicit rejection, sticky back-fill (Priority: P2)

**Goal**: accept inbound up to downstream degree; over capacity send an explicit `Rejected` (not misbehaviour); a rejected dial marks the peer failed (sticky) and the next `ConnectionSetup` back-fills the next-ranked candidate.

**Independent Test**: drive a node past its downstream degree → exactly downstream degree accepted, the rest dropped with the over-capacity cause + `Rejected`, no severance. On the dialer, after a `Rejected`, re-invoke `ConnectionSetup` → next-ranked candidate dialed; exhaustion → under-fill.

### Tests for User Story 2 (write first; MUST fail) ⚠️

- [X] T012 [P] [US2] Unit tests for `BoundedAcceptance`/`Admission` in `pubsub-node/src/acceptance/bounded.rs`: `RejectMembership` when not membership-valid, `RejectOverCapacity` at/above downstream degree, `Accept` below (FR-010)
- [X] T013 [P] [US2] **[coordinate]** Integration test in `pubsub-node/tests/connections.rs`: node at downstream degree drops the extra request with the over-capacity cause, sends `Rejected`, records no downstream entry, emits no `Misbehaved`/`Terminated` (FR-011)
- [X] T014 [P] [US2] **[coordinate]** Integration test in `pubsub-node/tests/connections.rs`: an explicit `Rejected` marks the peer failed (sticky — never re-dialed this run); the next `ConnectionSetup` re-selects the next-ranked candidate over the viable set; candidate exhaustion settles at under-fill; the rejection counter increments (FR-014, FR-015, FR-016)

### Implementation for User Story 2

- [X] T015 [US2] Evolve `ConnectionAcceptanceStrategy`: `accepts -> bool` → `admit(...) -> Admission { Accept, RejectMembership, RejectOverCapacity }` taking the current downstream view; map `AcceptFromAllCandidates`; re-export `Admission` from `lib.rs` — in `pubsub-node/src/acceptance/mod.rs` (+ `accept_from_all.rs`)
- [X] T016 [US2] Implement `BoundedAcceptance { downstream_degree }` (re-export from `lib.rs`); parse downstream degree at the edge and select bounded vs unbounded in `pubsub-node/src/acceptance/bounded.rs`, `config.rs`, `main.rs`
- [X] T017 [US2] Add `ConnectionAction::Rejected { topic }` in `pubsub-node/src/message.rs`; amend `handle_connection_request` so `RejectOverCapacity` logs `downstream_capacity_reached` + sends `Rejected`, `RejectMembership` stays a silent drop, in `pubsub-node/src/state.rs`
- [X] T018 [US2] **[coordinate]** Add `handle_connection_rejected` (remove `AwaitingAccept`, insert into `failed_upstream` sticky, increment the rejection counter; `unsolicited_reject` drop otherwise) in `pubsub-node/src/state.rs`
- [X] T019 [US2] Add the explicit-rejection-count getter in `pubsub-node/src/node.rs`

**Checkpoint**: US1 + US2 work independently; bounded acceptance, explicit rejection, sticky back-fill observable.

---

## Phase 5: User Story 3 — Seed-varied, identity-unbiased selection (Priority: P3)

**Goal**: distinct seeds yield differing selections; over a seed sweep no candidate is systematically preferred.

**Independent Test**: two seeds → differing selections for candidates > upstream degree; per-candidate frequency over ≥1,000 seeds uniform within tolerance.

### Tests for User Story 3 (write first; MUST fail) ⚠️

- [X] T020 [P] [US3] Distinct-seed divergence test in `pubsub-node/src/connection/seeded_bounded.rs` (or `tests/`): two seeds → differing selections for candidates > upstream degree (SC-003)
- [X] T021 [P] [US3] Seed-sweep uniformity test (≥1,000 fixed seeds; per-candidate frequency within tolerance / chi-square gate p < 0.001 per research R5) as a fixed seeded loop (research R5: proptest or seeded loop) in `pubsub-node/src/connection/seeded_bounded.rs`

### Implementation for User Story 3

- [X] T022 [US3] If the sweep reveals bias, adjust the ranking-key composition (T005 helper) so selection is unbiased (FR-007); otherwise record that the digest choice already satisfies SC-004 — in `pubsub-node/src/connection/seeded_bounded.rs`

**Checkpoint**: all three stories validated.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [X] T023 [P] Record deferred items in `pubsub-node/specs/IMPLEMENTATION_NOTES.md`: the experiment/testing framework, dynamic re-selection on membership change + epochal rotation, golden nodes (push-based M2), bounded/seeded fan-out, any timeout/no-response re-dial; and resolve the N-007 "revisit at 005 (PeerView)" pointer — note candidates serve as the peer view (no `PeerView` abstraction added)
- [X] T024 [P] Regression check for SC-005: with bounded params absent, the existing full-mesh dissemination/connection suites stay green (no new code path engaged)
- [X] T025 Run the `specs/005-peer-view/quickstart.md` validation end-to-end
- [X] T026 `/speckit-analyze` consistency pass; record findings in `specs/005-peer-view/analysis.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no code dependencies. T003 is coordination only — it does **not** gate the **[coordinate]** tasks; they proceed with 005's own ordered structures and current strategy injection.
- **Foundational (Phase 2)**: T005 self-contained; T006 **[coordinate]** (touches `NodeState`). Blocks US1/US2.
- **US1 (Phase 3)**: depends on Foundational. MVP. T009/T010 self-contained; T008/T011 **[coordinate]** (touch shared files).
- **US2 (Phase 4)**: depends on Foundational; shares the ranking helper. T015/T016/T017 author the types; T018 **[coordinate]** wires the dialer transition; back-fill (T018) needs `Rejected` (T017) first.
- **US3 (Phase 5)**: depends on US1's selection (T009). Independent of US2.
- **Polish (Phase 6)**: after the desired stories.

### Within Each Story

- Tests first and FAIL before implementation (critical feature).
- US2: the seam change (T015) precedes `BoundedAcceptance` (T016); `Rejected` (T017) precedes the dialer's failed-mark (T018).

### Parallel Opportunities

- T003, T004 in Setup.
- Test groups: T007 (US1); T012–T014 (US2); T020–T021 (US3) — parallel within each story.
- Self-contained pieces (T005, T009, T015 types, T017 type) have no coordination concern; **[coordinate]** wiring/integration touch files the parallel refactor also edits — sync to avoid conflicts (not blocked on it).

---

## Parallel Example: User Story 2 tests

```bash
Task: "Unit test BoundedAcceptance/Admission in src/acceptance/bounded.rs"   # T012
Task: "Integration test over-capacity → Rejected, no severance"               # T013
Task: "Integration test sticky back-fill via ConnectionSetup re-invocation"   # T014
```

---

## Implementation Strategy

### MVP First (US1)

Setup → Foundational → US1 → validate: bounded, reproducible selection with accept-all. Demonstrable on its own.

### Incremental Delivery

1. Self-contained core (ranking + `SeededBoundedSelection` + `BoundedAcceptance`/`Admission` + `Rejected` type + their unit tests) — no coordination needed.
2. The wiring + integration tests (**[coordinate]**) — land independently using 005's ordered structures + current injection, syncing with the parallel refactor to avoid conflicts. Completes US1 then US2.
3. US3 seed-sweep validation.

### Notes

- [P] = different files, no incomplete dependency.
- Verify each test fails before implementing.
- Green checkpoint per commit (Constitution).
- Assertions read getters/snapshots, never log strings.

---

description: "Task list for feature 005 — verifiable hash-gated connection-selection and bounded acceptance"
---

# Tasks: Verifiable hash-gated connection-selection and bounded acceptance

**Input**: Design documents from `specs/005-peer-view/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R7), data-model.md, contracts/ (strategy-traits, connection-control)

**Tests**: MANDATORY. Correctness/protocol-behaviour claims (determinism/verifiability FR-002/SC-001/SC-002, degree ≈ `RF` FR-001/FR-003/SC-004, uniformity SC-003, acceptance + explicit rejection FR-007/FR-008, rejection dropping the pending upstream FR-009) — designated **critical** in plan.md, so test tasks precede implementation and MUST fail first (Constitution II).

**ADRs**: 0024 (verifiable hash-gated selection: the shared `strategies::edge` predicate + `bucket_count`/`accept_cap`; SHA-256, not `DefaultHasher`), 0025 (acceptance-seam return evolution + acceptor verifies + `ConnectionAction::Rejected`), 0030 (heartbeat interval + shared edge predicate module). Also 0028 (two-phase construction) + 0029 (`strategies/` module grouping).

**Note (redesign 2026-07-02 → bucketed-pull)**: the mechanism moved from seeded-PRNG sampling to the **verifiable hash-bucket predicate** (`docs/extensions/bucketed-pull.md`). The earlier seeded/degree-parameter machinery (`SeededBoundedConnection`/`BoundedAcceptance`, `--seed`/`--upstream-degree`/`--downstream-degree`, `ChaCha20` sampling, sticky-failed-set / rejection-counter / back-fill) is superseded. The dialer's reaction to a `Rejected` is minimal — remove the matching pending `AwaitingAccept` upstream only; retry/back-fill is deferred to a future strategy family, out of scope for 005. Completed tasks below keep their `[X]` and IDs but describe what actually shipped.

**Dependency (coordination, not a hard block)**: the broader determinism/purity refactor (strategies-as-`apply`-arguments, deterministic scheduling, decouple flag) is the co-developing architect's separate workstream. This feature does **not** block on it: it keeps **ordered structures (`BTreeSet`/`BTreeMap`) on its own state within this PR** and keeps strategy objects pure (it retains the current strategy injection and migrates later). Tasks marked **[coordinate]** touch files/shapes the parallel refactor also touches (strategy injection sites, `NodeState`) — sync to avoid conflicting edits; they are not gated on the refactor merging.

**Organization**: by user story. US1 (verifiable hash-gated selection) is the MVP; US2 adds verifiable bounded acceptance + `Rejected` (dialer drops the matching pending upstream only — no retry/back-fill); US3 validates adversary-resistance (no amplification) + uniformity.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: parallelizable (different files, no incomplete dependency)
- **[Story]**: US1/US2/US3 (user-story phases only)

## Path Conventions

Single Rust project: sources under `pubsub-node/src/`, tests under `pubsub-node/tests/`. Paths repo-relative.

---

## Phase 1: Setup

- [X] T001 ADR 0024 (verifiable hash-gated selection: the shared `strategies::edge` predicate — SHA-256 over a length-prefixed canonical encoding, not `DefaultHasher` — `bucket_count = max(1, round(candidates/RF))` with a **fixed** `RF`, `accept_cap = ⌈RF + c·√RF⌉`, and the small-topic `B=1` connect-to-all floor) in `pubsub-node/docs/decisions/0024-seeded-bounded-selection.md`. (Superseded the earlier seeded-PRNG design; retry-to-a-minimum is deferred to a future strategy family.)
- [X] T002 ADR 0025 (acceptance-seam evolution `bool → Admission { Accept, RejectMembership, RejectIllegitimate, RejectOverCapacity }` + current-downstream + interval input + the acceptor verifying the same predicate; `ConnectionAction::Rejected` on over-capacity only + minimal dialer handling) in `pubsub-node/docs/decisions/0025-acceptance-seam-and-rejected-action.md`
- [X] T002b ADR 0030 (heartbeat interval + shared edge predicate: `Event::ConnectionSetup` → `Event::Heartbeat { interval }`, `NodeState.interval` fold, interval threaded through both trait methods; the `strategies::edge` module both seams consult) in `pubsub-node/docs/decisions/0030-heartbeat-interval-and-edge-predicate.md`
- [ ] T003 [P] Coordinate with the co-developing architect to avoid conflicting edits on shared files (strategy injection sites, `NodeState`) and align ordered-structure type choices; record the agreed types in `specs/005-peer-view/research.md` (R6). Not a gate — 005 proceeds with its own ordered structures and current strategy injection.
- [X] T004 [P] Test scaffolding: extend `ConnectionScript` with a `rejected` step; verifiable nodes construct via the existing `node_with_strategy` helper with `HashGatedConnection`/`VerifiableBoundedAcceptance` (`genesis`/`rf`/`cap_buffer`) in `pubsub-node/tests/common/mod.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**⚠️ CRITICAL**: must complete before US1/US2.

- [X] T005 Add the shared verifiable edge predicate module `strategies::edge` — `is_valid_edge(genesis, topic, requester, candidate, interval, buckets)` (SHA-256 over a domain-separated, length-prefixed canonical encoding, leading bytes mod `buckets`; `buckets <= 1` always true), `bucket_count(candidates_len, rf) = max(1, round(len/rf))`, `accept_cap(rf, c) = ⌈rf + c·√rf⌉` — in `pubsub-node/src/strategies/edge.rs` — refactor-agnostic
- [X] T006 **[coordinate]** Rename `Event::ConnectionSetup` → `Event::Heartbeat { interval }` (`event.rs`); add `NodeState.interval` folded in `handle_heartbeat`, which then selects over `candidates` at that interval, in `pubsub-node/src/state.rs`. (No failed-set / rejection counter was added.)

**Checkpoint**: edge predicate module + `handle_heartbeat` folding the interval and selecting over `candidates` in place.

---

## Phase 3: User Story 1 — Verifiable hash-gated upstream selection (Priority: P1) 🎯 MVP

**Goal**: a node forms its per-topic upstream edges from the verifiable hash-bucket predicate; expected out-degree ≈ the fixed `RF`; same genesis + membership + interval reproduces an identical selection; small topics (`B=1`) connect to all.

**Independent Test**: candidates > `RF` under genesis g + interval i; rebuilt under g/i the selection is identical, and degree tracks `RF`. With ≤ ~`RF` candidates, all are selected (`B=1`). (Acceptance still accept-all here.)

### Tests for User Story 1 (write first; MUST fail) ⚠️

- [X] T007 [P] [US1] Unit tests for `HashGatedConnection` in `pubsub-node/src/strategies/connection/hash_gated.rs`: expected degree tracks `RF` on a large candidate set (FR-001/FR-003), all selected when `≤ ~RF` candidates / `B=1` (FR-003), identical output across iteration orders / repeated calls (FR-002), per-node variety by `self_id` (FR-005), and the **default genesis 0** path produces a deterministic, repeatable selection (FR-004). (Edge-predicate determinism/directionality/`1/B` density are unit-tested in `strategies/edge.rs`.)
- [X] T008 [P] [US1] **[coordinate]** Integration test in `pubsub-node/tests/connections.rs`: an N-node network under genesis g at interval i forms a partial topology; rebuilt under g/i it is identical; per-topic degree tracks `RF` (SC-001, SC-004)

### Implementation for User Story 1

- [X] T009 [US1] Implement `HashGatedConnection { genesis, self_id, rf }` (impl `ConnectionStrategy` with the interval arg, using the T005 `strategies::edge` predicate) and re-export it from `pubsub-node/src/lib.rs` — in `pubsub-node/src/strategies/connection/hash_gated.rs` — refactor-agnostic
- [X] T010 [US1] Parse `--genesis` + `--rf` at the edge and select the hash-gated vs unbounded selection strategy in `pubsub-node/src/config.rs` and `pubsub-node/src/main.rs`
- [X] T011 [US1] **[coordinate]** Supply the selection strategy (with `self_id`) at node construction (current injection; align with the refactor's eventual argument shape) in `pubsub-node/src/node.rs`

**Checkpoint**: US1 functional — verifiable, reproducible selection with accept-all acceptance.

---

## Phase 4: User Story 2 — Verifiable bounded acceptance, explicit rejection (Priority: P2)

**Goal**: on a verified `Request`, accept iff membership-valid ∧ the same edge predicate holds this interval ∧ under `OC = ⌈RF + c·√RF⌉`; membership OR predicate failure is a silent drop; over capacity of a legitimate request send an explicit `Rejected` (not misbehaviour); on the dialer a `Rejected` drops the matching pending upstream only — no retry/back-fill, so the realized upstream degree may settle below `RF`.

**Independent Test**: a predicate-valid, membership-valid request under cap → accepted; one whose predicate fails this interval → silently dropped; drive past `OC` legitimate requests → the extra dropped with the over-capacity cause + `Rejected`, no severance; an unregistered-topic / non-member request → silently dropped. On the dialer, a `Rejected` removes the matching `AwaitingAccept` and does nothing further; the realized degree may under-fill.

### Tests for User Story 2 (write first; MUST fail) ⚠️

- [X] T012 [P] [US2] Unit tests for `VerifiableBoundedAcceptance`/`Admission` in `pubsub-node/src/strategies/acceptance/verifiable_bounded.rs`: `RejectMembership` when not membership-valid, `RejectIllegitimate` when the edge predicate fails this interval, `RejectOverCapacity` at/above `OC`, `Accept` below (all four, FR-007); small-topic (`B=1`) admits every member below cap (FR-003)
- [X] T013 [P] [US2] **[coordinate]** Integration test in `pubsub-node/tests/connections.rs`: node at `OC` drops the extra legitimate request with the over-capacity cause, sends `Rejected`, records no downstream entry, emits no `Misbehaved`/`Terminated`; a predicate-failing request is silently dropped (FR-007/FR-008)
- [X] T014 [P] [US2] **[coordinate]** Integration test in `pubsub-node/tests/connections.rs`: an explicit `Rejected` removes the matching pending `AwaitingAccept` upstream and produces no further effects (no retry/back-fill); the realized upstream degree may settle below `RF` (under-fill) (FR-009, SC-007)

### Implementation for User Story 2

- [X] T015 [US2] Evolve `ConnectionAcceptanceStrategy`: `accepts -> bool` → `admit(..., interval) -> Admission { Accept, RejectMembership, RejectIllegitimate, RejectOverCapacity }` taking the current downstream view + interval; map `AcceptFromAllCandidates` (`Accept`/`RejectMembership` only); re-export `Admission` from `lib.rs` — in `pubsub-node/src/strategies/acceptance/mod.rs` (+ `accept_from_all.rs`)
- [X] T016 [US2] Implement `VerifiableBoundedAcceptance { genesis, self_id, rf, cap_buffer }` (recomputes the edge predicate to **verify**, caps downstream at `OC = accept_cap(rf, cap_buffer)`; re-export from `lib.rs`); parse `--genesis`/`--rf`/`--cap-buffer` at the edge and select verifiable vs unbounded in `pubsub-node/src/strategies/acceptance/verifiable_bounded.rs`, `config.rs`, `main.rs`
- [X] T017 [US2] Add `ConnectionAction::Rejected { topic }` in `pubsub-node/src/message.rs`; amend `handle_connection_request` so `RejectOverCapacity` logs `downstream_capacity_reached` + sends `Rejected`, `RejectMembership`/`RejectIllegitimate` stay silent drops (distinct causes `membership_validation_failed` / `illegitimate_request`), in `pubsub-node/src/state.rs`
- [X] T018 [US2] **[coordinate]** Add `handle_connection_rejected` (remove the matching `AwaitingAccept` upstream only; `unsolicited_reject` drop otherwise) in `pubsub-node/src/state.rs`. (No `failed_upstream` insert / rejection counter — retry/back-fill deferred to a future strategy family.)
- [X] T019 [US2] No explicit-rejection-count getter is exposed. Observability is the upstream/downstream snapshots only (FR-013); no rejection-count `NodeState`/`Node` getter.

**Checkpoint**: US1 + US2 work independently; verifiable bounded acceptance and explicit rejection observable (via the upstream/downstream snapshots).

---

## Phase 5: User Story 3 — Adversary cannot exhaust connection slots (Priority: P3)

**Goal**: an id spamming requests cannot occupy more of a victim's downstream slots than its `1/B` hash share; over an interval/genesis sweep the predicate is pseudo-uniform (no candidate systematically preferred).

**Independent Test**: for a fixed genesis + interval, the accepted fraction from a single id matches the `1/B` density and predicate-failing requests are all dropped; per-candidate (or per-interval) frequency over ≥1,000 intervals uniform within tolerance.

### Tests for User Story 3 (write first; MUST fail) ⚠️

- [X] T020 [P] [US3] No-amplification test: a single id's accepted fraction at a victim is bounded by the `1/B` density; predicate-failing requests are all dropped (SC-005) — in `pubsub-node/tests/connections.rs` (or a `strategies::edge` unit test enumerating accepted requests)
- [X] T021 [P] [US3] Predicate uniformity test (sweep of ≥1,000 intervals/genesis values on a fixed candidate set with `B>1`; accepted fraction ≈ `1/B`, chi-square gate p < 0.001 per research R5) as a fixed seeded loop in `pubsub-node/src/strategies/edge.rs` (`edge_density_approximates_one_over_buckets`)

### Implementation for User Story 3

- [X] T022 [US3] Record that the SHA-256 hash-bucket predicate is pseudo-uniform by construction, satisfying SC-003/SC-005 (the `1/B` density holds without adjustment) — in `pubsub-node/src/strategies/edge.rs`

**Checkpoint**: all three stories validated.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [X] T023 [P] Record deferred items in `pubsub-node/specs/IMPLEMENTATION_NOTES.md`: the experiment/testing framework, discovery/view sampling (`H_v`), periodic heartbeats + cross-interval rotation/teardown, the real unbiasable beacon, the incentive/chain layer (deposits/sybil bound/slashing/on-chain identity), golden nodes (push-based M2); and resolve the N-007 "revisit at 005 (PeerView)" pointer — note candidates serve as the peer view (no `PeerView` abstraction added)
- [X] T024 [P] Regression check for SC-006: with the verifiable params absent, the existing full-mesh dissemination/connection suites stay green (no new code path engaged)
- [X] T025 Run the `specs/005-peer-view/quickstart.md` validation end-to-end
- [X] T026 `/speckit-analyze` consistency pass; record findings in `specs/005-peer-view/analysis.md`
- [X] T027 Two-phase strategy construction (ADR 0028, FR-015): in `src/strategies/config.rs` add per-seam params (`ConnectionParams { self_id, genesis, rf }`, `AcceptanceParams { self_id, genesis, rf, cap_buffer }`) + `StrategyConfigError` + the aggregate two-phase builder (`NodeStrategies` / `NodeStrategiesBuilder` — phase 1 holds the resolved kinds, phase 2 `build(&ConnectionParams, &AcceptanceParams)`); give `ConnectionStrategyKind`/`AcceptanceStrategyKind` a fallible `build(&SeamParams)` that validates only its seam's required params (`hash-gated`/`verifiable-bounded` require `rf`); refactor `main.rs` to one aggregate build that maps the error once (no per-strategy validation, repetition, or branching at the edge). Unit tests for each kind's build.
- [X] T028 Module grouping (ADR 0029): move all strategy policy under `src/strategies/` (`connection`/`acceptance`/`fanout` seams + `config`); extract connection lifecycle state (`UpstreamState`, `test_support`) to a core `src/connection_state.rs` (`Admission` stays with the acceptance seam). Re-point `lib.rs` re-exports while preserving public names; update `node.rs`/`state.rs` import paths. Move-only, behaviour-preserving — full suite + clippy + fmt green.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no code dependencies. T003 is coordination only — it does **not** gate the **[coordinate]** tasks; they proceed with 005's own ordered structures and current strategy injection.
- **Foundational (Phase 2)**: T005 (`strategies::edge`) self-contained; T006 **[coordinate]** (touches `event.rs`/`NodeState`). Blocks US1/US2.
- **US1 (Phase 3)**: depends on Foundational. MVP. T009/T010 self-contained; T008/T011 **[coordinate]** (touch shared files).
- **US2 (Phase 4)**: depends on Foundational; shares the `strategies::edge` predicate. T015/T016/T017 author the types; T018 **[coordinate]** wires the dialer transition (drop the matching pending upstream), which needs `Rejected` (T017) first.
- **US3 (Phase 5)**: depends on US1's selection (T009) + the edge predicate (T005). Independent of US2.
- **Polish (Phase 6)**: after the desired stories.

### Within Each Story

- Tests first and FAIL before implementation (critical feature).
- US2: the seam change (T015) precedes `VerifiableBoundedAcceptance` (T016); `Rejected` (T017) precedes the dialer's rejection handler (T018).

### Parallel Opportunities

- T003, T004 in Setup.
- Test groups: T007 (US1); T012–T014 (US2); T020–T021 (US3) — parallel within each story.
- Self-contained pieces (T005 edge predicate, T009, T015 types, T017 type) have no coordination concern; **[coordinate]** wiring/integration touch files the parallel refactor also edits — sync to avoid conflicts (not blocked on it).

---

## Parallel Example: User Story 2 tests

```bash
Task: "Unit test VerifiableBoundedAcceptance/Admission in src/strategies/acceptance/verifiable_bounded.rs"   # T012
Task: "Integration test over-capacity → Rejected, no severance; predicate failure silently dropped"           # T013
Task: "Integration test Rejected drops the matching pending upstream only"                                     # T014
```

---

## Implementation Strategy

### MVP First (US1)

Setup → Foundational → US1 → validate: verifiable, reproducible selection with accept-all. Demonstrable on its own.

### Incremental Delivery

1. Self-contained core (`strategies::edge` predicate + `HashGatedConnection` + `VerifiableBoundedAcceptance`/`Admission` + `Rejected` type + `Heartbeat` rename + their unit tests) — no coordination needed.
2. The wiring + integration tests (**[coordinate]**) — land independently using 005's ordered structures + current injection, syncing with the parallel refactor to avoid conflicts. Completes US1 then US2.
3. US3 no-amplification + uniformity validation.

### Notes

- [P] = different files, no incomplete dependency.
- Verify each test fails before implementing.
- Green checkpoint per commit (Constitution).
- Assertions read getters/snapshots, never log strings.

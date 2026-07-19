# Tasks: Deterministic experiments framework

**Input**: Design documents from `/specs/016-experiments-framework/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: This feature carries a correctness claim (the instrument's numbers
must be right), so Constitution Principle II applies: for the plan's
designated **critical set** — driver delivery semantics, wave-canonicalisation
determinism, propagation-graph extraction + Kosaraju/condensation/goodness,
the accounting identity, and the coverage/depth/miss-cause metrics — the
failing-test task precedes its implementation task with separate IDs.
Non-critical surfaces (front-end parsing, config validation messages,
quickstart) are tests-with, one task.

**ADRs**: 0032 (driver architecture) authored in Foundational; 0033 (output
contract + statistics conventions) inside US1 with the output work; 0034
(serde_json optional dependency) with the Setup scaffold that adds it.

**Organization**: grouped by user story (template convention). **Commit
mapping** (tasks-input directive, refined): each task phase below ends at a
green checkpoint (`cargo test` **and** `cargo test --features experiments`,
plus fmt/clippy) and is the intended commit boundary; review fixes within a
phase amend rather than append. Mapping to the plan's implementation phasing:
Setup+Foundational = plan phase 1 (+ scaffold), US1 = plan phases 2–4
(single-experiment path end to end), US2 = plan phase 3's sweep/parallel
remainder, US3 = the detail/replay slice, US4 + Polish = plan phase 5.

**Citations**: task descriptions cite anchors as 016-FR-NNN / 016-SC-NNN;
global ids (ADR NNNN) unprefixed.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: parallelizable (disjoint files, no dependency on an incomplete task)
- **[Story]**: US1–US4 on user-story phase tasks only

---

## Phase 1: Setup (gating scaffold)

**Purpose**: everything later lands behind the flag; the default build is
provably untouched.

- [X] T001 Gating scaffold: add `experiments = ["dep:serde_json"]` cargo
      feature and optional `serde_json` dependency in `Cargo.toml`;
      feature-gated `pub mod experiments;` in `src/lib.rs` with an empty
      `src/experiments/mod.rs`; `[[bin]] experiments`
      (`required-features = ["experiments"]`) stub in
      `src/bin/experiments.rs`; verify the default build/test/clippy output
      is unaffected with the feature off (016-FR-001, 016-FR-002,
      016-FR-003; 016-SC-008).
- [X] T002 [P] Author ADR 0034 — serde_json as an optional feature-tied
      dependency (Justified Dependencies standard) in
      `docs/decisions/0034-serde-json-optional-dependency.md` (plan
      Constitution Check III; research R4).

**Checkpoint**: both test configurations green; commit.

---

## Phase 2: Foundational (instrument skeleton — blocks all stories)

**Purpose**: population + driver + fixtures every story runs on.

- [ ] T003 Population layer in `src/experiments/population.rs`: `Participant`
      (class, down, `NodeState`, strategy triad), seeded population build
      (keys via the seeded mock crypto scheme, class assignment), registry
      pre-population and faithful-fold event scripts, build validation
      (016-FR-004, 016-FR-008, 016-FR-011, 016-FR-031; data-model §1).
- [ ] T004 [P] Scripted-topology builders in `src/experiments/scripted.rs`:
      `line(n)`, `star(n)`, `full_mesh(n)` + per-node class overrides via
      the pre-population path — declarative test-construction standard
      (016-FR-032; data-model §8).
- [ ] T005 [P] Experiments-only strategies in
      `src/experiments/strategies.rs`: `SilentRelay` fan-out and
      `UniformSampler` dial (seeded, without replacement,
      min(target_degree, |candidates|) degeneracy), with unit tests
      (016-FR-012, 016-FR-013; research R10).
- [ ] T006 **[TDD test-first]** Driver tests (unit in
      `src/experiments/driver.rs` test module + integration skeleton in
      `tests/experiments_framework.rs`): delivery semantics on scripted
      topologies (dedup, fire-once, exact quiescence), wave-canonicalisation
      determinism (double-run value equality), faithful-mode barrier (all
      registry folds before Synced; all Synced one wave), single-epoch
      behaviour, churn draw semantics (no events; down nodes not stepped),
      and publish repetition per the publishes-per-run knob (fresh messages,
      distinct content hashes, no state reset) — written and failing before
      T007 (016-FR-005…016-FR-010, 016-FR-014; 016-SC-002 partial).
- [ ] T007 Driver implementation in `src/experiments/driver.rs`: wavefront
      scheduler with canonical content-keyed wave sort, `Effect::Send`
      routing, `Misbehaved` consumption/tally, per-phase drains, phase
      orchestration (registration → dial → churn draw → publish, repeated
      per the publishes-per-run knob, default 1), making
      T006 pass (016-FR-004…016-FR-010, 016-FR-014, 016-FR-027; research
      R1/R2; data-model §2/§3).
- [ ] T008 Author ADR 0032 — deterministic experiments driver (wavefront
      scheduler, driver-owned canonicalisation, participant model, phase
      orchestration) in
      `docs/decisions/0032-deterministic-experiments-driver.md` (plan
      Constitution Check III; research R1/R2/R3).

**Checkpoint**: both configurations green; commit.

---

## Phase 3: User Story 1 — one reproducible experiment, metrics readable (P1) 🎯 MVP

**Goal**: configure one experiment, run it from the front end, get the three
artifacts; same seed ⇒ byte-identical outputs.

**Independent test**: run a small experiment (N = 100, silent adversaries,
churn > 0, R = 20) twice with the same master seed; artifacts well-formed
and identical; per-run identity holds (spec US1).

- [ ] T009 [US1] **[TDD test-first]** Graph-analytics tests in
      `src/experiments/graph.rs` test module: M2 extraction from node
      states, iterative Kosaraju + condensation on hand-built digraphs
      (incl. the multi-component worked example), goodness pre/post churn,
      min-publisher-coverage = (smallest sink component − 1)/(up-honest − 1),
      degree/sink statistics — failing before T010 (016-FR-019…016-FR-022).
- [ ] T010 [US1] Graph analytics in `src/experiments/graph.rs`:
      `DisseminationModel` dispatch (M2 variant) owning extraction,
      iterative Kosaraju, condensation, `GoodnessVerdict`, topology shape,
      making T009 pass (016-FR-019…016-FR-022; research R5/R8; data-model §4).
- [ ] T011 [US1] **[TDD test-first]** Metrics tests (unit +
      `tests/experiments_framework.rs`): coverage with excluded-publisher
      denominator, per-node first-receipt waves + depth distribution,
      miss-cause decomposition on a scripted silent-adversary topology with
      hand-computed answer, sends split by recipient class, suppressed
      accounting, the identity
      sends = first receipts + suppressed + sent-to-down, and the
      two-instrument cross-check (drain coverage ≡ graph reachability) —
      failing before T012 (016-FR-015…016-FR-018; 016-SC-002, 016-SC-003).
- [ ] T012 [US1] Metrics implementation in `src/experiments/metrics.rs`:
      drain observation, classification, tallies, identity assertion,
      run-record assembly (pre-churn fields present iff churn > 0), making
      T011 pass (016-FR-015…016-FR-018; data-model §5).
- [ ] T013 [P] [US1] Statistics in `src/experiments/statistics.rs` with
      tests: sparse integer histograms, fixed-width coverage bins,
      means/percentiles, closed-form Wilson 95%, aggregates fold in
      run-index order incl. the full_coverage ≥ good assertion
      (016-FR-023; 016-SC-007; data-model §6).
- [ ] T014 [US1] Single-experiment sweep path in `src/experiments/sweep.rs`:
      manifest construction, SHA-256 seed derivation with domain labels,
      run-as-pure-function orchestration, canonical-order JSONL streaming,
      aggregates emission (016-FR-024…016-FR-026, 016-FR-028, 016-FR-029;
      research R6; contracts/output-artifacts.md).
- [ ] T015 [US1] Config + front end: parsed sweep-description types incl.
      publishes-per-run (default 1) and
      validation in `src/experiments/config.rs` (single topic; eligible
      receivers nonempty; up-honest publisher exists), TOML + clap edge in
      `src/bin/experiments.rs`, wired end to end for a single experiment;
      tests-with (016-FR-031; contracts/sweep-config.md).
- [ ] T016 [US1] Determinism integration tests in
      `tests/experiments_framework.rs`: value-level record equality across
      repeated executions; one file-level byte diff of a tiny sweep written
      twice to temp dirs; replay-by-seed record equality; record
      boundedness (016-SC-005) — two runs differing only in N at fixed
      target_degree: histogram lengths bounded by realised max degree/depth
      + 1 and near-constant across the two N values, no array field scaling
      with N (the structural field inventory is pinned by the golden
      serialization test)
      (016-SC-001 partial, 016-SC-004, 016-SC-005; research R9).
- [ ] T017 [US1] Author ADR 0033 — experiment output contract & statistics
      conventions (three artifacts, derivability invariant, counts + Wilson
      95%, excluded-publisher denominator) in
      `docs/decisions/0033-experiment-output-contract.md` (plan Constitution
      Check III; research R7).

**Checkpoint**: US1 independently deliverable (MVP); both configurations
green; commit.

---

## Phase 4: User Story 2 — sweep a parameter into a curve (P2)

**Goal**: axes expand to experiments; parallel execution never changes
output.

**Independent test**: two-axis small sweep at workers 1 and 8 — grid counts
correct, rows reference experiments by index, artifacts identical (spec US2).

- [ ] T018 [US2] Axes expansion in `src/experiments/config.rs` +
      manifest experiment list in `src/experiments/sweep.rs`; per-experiment
      aggregates entries (016-FR-028, 016-FR-031; contracts/sweep-config.md).
- [ ] T019 [US2] Worker pool in `src/experiments/sweep.rs`:
      `std::thread::scope` over a run-index queue, pre-sized results vector,
      canonical write/fold order, `--workers` knob bounding in-flight runs
      (016-FR-025, 016-FR-026; research R3).
- [ ] T020 [US2] Sweep integration tests in `tests/experiments_framework.rs`:
      workers 1 vs K byte-identical artifacts; grid/row/aggregate counts;
      P(good) reported as counts + Wilson 95% incl. the all-good sample
      (016-SC-001, 016-SC-007).

**Checkpoint**: both configurations green; commit.

---

## Phase 5: User Story 3 — replay and dissect a run (P3)

**Goal**: opt-in per-node detail; replay-by-seed dissection workflow.

**Independent test**: replay a non-full-coverage run's seed with detail on —
record identical, per-node miss causes consistent with the topology (spec US3).

- [ ] T021 [US3] Per-node detail: `--per-node-detail` flag through
      `src/bin/experiments.rs` and `src/experiments/sweep.rs`;
      `PerNodeDetail` emission (first-receipt wave, first-delivery origin,
      degrees, miss cause, class) in `src/experiments/metrics.rs`; test:
      replay equality + detail consistency with recorded topology; detail
      never alters the three artifacts (016-FR-030; 016-SC-004;
      contracts/output-artifacts.md).

**Checkpoint**: both configurations green; commit.

---

## Phase 6: User Story 4 — M2-comparison demonstration shipped (P4)

**Goal**: the two comparison configs + suite smoke variant. The manual
executions themselves are the closing Polish task (T026) — in scope per
016-FR-033's "comparison documented", but never part of this phase's
checkpoint or the test suite.

**Independent test**: smoke variant completes in seconds asserting pipeline
health only (spec US4).

- [ ] T022 [P] [US4] Shipped configurations in `configs/experiments/`:
      `m2-operating-point.toml` (N = 20 000, μ = 0.2, RF = 24;
      uniform-sampler + accept-from-all + forward-to-all),
      `m2-bulk-regime.toml` (named point from m2's full-coverage validation
      grid), `m2-smoke.toml` (suite-sized) (016-FR-033;
      contracts/sweep-config.md).
- [ ] T023 [US4] Smoke test in `tests/experiments_framework.rs`: run
      `m2-smoke.toml` end to end — config parses, sweep executes, artifacts
      well-formed, identities and determinism hold; < 30 s budget; never
      numeric agreement (016-FR-033; 016-SC-006 partial).

**Checkpoint**: both configurations green; commit.

---

## Phase 7: Polish & cross-cutting

- [ ] T024 Docs alignment: rustdoc for the experiments API surface
      (implementation-neutral, no FR citations), quickstart verified against
      the built binary (flags, file names, procedure), fmt/clippy sweep in
      both configurations (Engineering Standards; quickstart.md).
- [ ] T025 Wrap-up: update the `CLAUDE.md` active-feature stanza
      (implemented status), record any deferrals accrued during
      implementation in `specs/IMPLEMENTATION_NOTES.md`, final green sweep
      without and with `--features experiments`.
- [ ] T026 M2-comparison execution & write-up (manual; the feature's closing
      artifact — amended in 2026-07-19: 016-FR-033's "comparison documented"
      is in-scope work, not post-feature usage): run
      `configs/experiments/m2-operating-point.toml` and
      `m2-bulk-regime.toml` per the quickstart procedure; document the
      comparison in `docs/experiments/m2-comparison.md` — measured
      honest-to-honest sends, copies per honest node, depth distribution vs
      comparison.md's published values; P(good) counts + Wilson 95% vs the
      coverage law at the bulk point; deviations recorded and explained
      (informs, does not gate); the mandated uncertainty-methodology note
      (±1σ vs counts+Wilson, to raise with the formal-methods team); cite
      the tool commit and master seeds so the write-up is reproducible. Raw
      artifacts are not committed — the seeds + commit reproduce them
      (016-FR-033; 016-SC-006; quickstart.md).

**Checkpoint**: feature complete; commit.

---

## Dependencies

- Setup (T001–T002) → Foundational (T003–T008) → US1 (T009–T017) →
  US2 (T018–T020) → US3 (T021) → US4 (T022–T023) → Polish (T024–T026).
- T026 requires T023 (instrument + shipped configs) and T024 (verified
  procedure); its ~1 h wall-clock is why it is the closing task, after
  T025's final sweep — the comparison document is the feature's last commit.
- Within Foundational: T003 → T006/T007 (population precedes driver tests);
  T004, T005 parallel to T003 after T001; T006 (failing) strictly before
  T007; T008 with the phase.
- Within US1: T009 → T010; T011 → T012 (and T011 needs T010's reachability
  for the cross-check); T013 parallel to T011/T012; T014 needs T012 + T013;
  T015 needs T014; T016 needs T015; T017 with the output work.
- US2 builds only on US1's sweep path; US3 only on US1's record path;
  US4 only on US2 (axes not required but config kinds and runner are).

## Parallel opportunities

- T002 ∥ T001's tail (disjoint files).
- T004 ∥ T005 ∥ (T003 after its API lands) — disjoint modules.
- T013 (statistics.rs) ∥ T011/T012 (metrics.rs) — disjoint modules.
- T022 ∥ T021 if staged early (configs are data files) — kept in US4 for
  the commit mapping.

## Implementation strategy

MVP = through US1 (Phase 3): a single reproducible experiment with all
metrics and the three artifacts — already scientifically usable. Each phase
is a green-checkpoint commit; TDD pairs (T006→T007, T009→T010, T011→T012)
must show the failing state before the implementation lands. The manual
M2-comparison execution and write-up is T026, the closing task (amendment
2026-07-19, superseding the tasks-input directive that framed it as
post-feature usage — 016-FR-033 mandates the documented comparison): it
runs outside the test suite but inside the feature.

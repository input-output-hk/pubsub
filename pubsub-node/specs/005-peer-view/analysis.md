# Analysis: 005-peer-view (post-implementation consistency pass)

**Date**: 2026-06-30 · **Scope**: spec.md, plan.md, tasks.md, contracts/, research.md, data-model.md vs the constitution **and** the implementation (US1–US3 merged on the branch). Read-only.

## Findings

| ID | Category | Severity | Location | Summary | Recommendation |
|----|----------|----------|----------|---------|----------------|
| I1 | Inconsistency (doc↔decision↔code) | MEDIUM | plan.md Summary (¶2) | Summary still says the feature is "built on a prerequisite determinism/purity refactor … strategies passed as `apply` arguments", contradicting the relaxed-dependency decision (Technical Context + research R6) and the implementation, which retained the current strategy injection. | Reword the Summary to "coordinated with (not built on)" + "current injection retained", matching R6. |
| I2 | Inconsistency (doc↔code) | MEDIUM | plan.md Project Structure (source tree + Structure Decision) | Lists flat `connection.rs`/`acceptance.rs` and says "`Node`/`apply` wiring follows the refactor's strategies-as-arguments shape rather than today's strategies-in-`NodeState`". Code refactored these into `connection/`, `acceptance/`, `fanout/` modules and kept strategies in `NodeState`. | Update the tree to the module layout; correct the wiring note to current injection. |
| I3 | Inconsistency (path drift) | LOW | tasks.md (T005/T006/T009/T015–T019 file paths) | Tasks reference `src/connection.rs` / `src/acceptance.rs`; the per-seam module refactor moved these to `connection/<strategy>.rs` etc. Tasks are complete; only the cited paths drifted. | Optional: note the module layout; non-blocking (tasks done). |
| O1 | Observation (TDD evidence) | LOW | tasks.md / commit history | Tests exist and pass for every correctness claim, but several were committed alongside their implementation rather than as a separate failing-first commit, so the strict "test fails first" ordering isn't independently evidenced from the artifacts. | None required — outcome (tests present + green) satisfies Principle II; note for future strictness. |
| O2 | Task status | LOW | tasks.md T004 | The `ConnectionScript` `rejected` step was added, but T004 also mentioned bounded-node builder helpers; the existing `node_with_strategy` was reused instead and T004 left unchecked. | Mark T004 done with a note (helper reuse) or tick the delivered part. |

No CRITICAL or HIGH findings. No constitution MUST violations.

## Coverage Summary (requirement → task/test)

| Req | Has task? | Task/test | Notes |
|-----|-----------|-----------|-------|
| FR-001/002 bound, all-when-fewer | ✅ | T007, T008 | unit + integration |
| FR-003 determinism | ✅ | T007, T008 | |
| FR-004 default seed 0 | ✅ | T007 (`default_seed_zero`) | |
| FR-005 per-node derivation | ✅ | T007 (`varies_by_self_id`) | |
| FR-006 distinct seeds | ✅ | T020 | |
| FR-007 unbiased | ✅ | T021 (chi-square sweep) | |
| FR-008 tie-break | ✅ | T005, T007 | deterministic ranking |
| FR-009 pure transition | ✅ | T009 | no RNG/clock |
| FR-010 acceptance bound | ✅ | T012, T016 | |
| FR-011 explicit rejection | ✅ | T013, T017 | + no severance |
| FR-012 configurable bounds | ✅ | T010, T016 | CLI edge |
| FR-013 additive defaults | ✅ | T024, main.rs default | |
| FR-014 sticky back-fill | ✅ | T014, T018 | |
| FR-015 under-fill | ✅ | T008, T014 | |
| FR-016 observable outcomes | ✅ | T019 (getter), upstream getter | |
| FR-017 ordered structures | ✅ | T006 (`BTreeSet`) | |
| FR-018 pure strategy objects | ✅ | T009 | seed/bounds at construction |
| SC-001 reproducible | ✅ | T008 | |
| SC-002 bounds respected | ✅ | T008 | |
| SC-003 distinct seeds diverge | ✅ | T020 | |
| SC-004 uniformity | ✅ | T021 | |
| SC-005 unbounded = full mesh | ✅ | T024 (existing suite green w/ defaults) | |
| SC-006 back-fill + under-fill observable | ✅ | T014, T008 | |
| SC-007 rejection count observable | ✅ | T019, T014 | |

**Coverage: 18/18 FR + 7/7 SC = 100%** have ≥1 task/test.

## Constitution Alignment

No violations. ✅ I (traceable), ✅ II (tests present + green for all correctness claims; see O1), ✅ III (ADRs 0024/0025), ✅ IV (acceptance-seam signature change surfaced as ADR 0025), ✅ V (no spec/formal_spec edits). Engineering standards: ✅ reproducible-from-seed, ✅ no wall-clock in transition, ✅ assertions via getters/state (not log strings), ✅ parse-at-edge, ✅ declarative test construction.

## Implementation verification (claims vs code)

Confirmed present: `Admission` enum + `admit` (acceptance/mod.rs); `ConnectionAction::Rejected` (message.rs, tag 0x03); `failed_upstream: BTreeSet` (state.rs); `rejections_received` getter (node.rs); `SeededBoundedSelection`/`BoundedAcceptance`/`ConnectionStrategyKind`/`AcceptanceStrategyKind` re-exported (lib.rs). Full suite green, clippy + fmt clean.

## Metrics
- Requirements: 25 (18 FR + 7 SC) · Tasks: 26 (23 done; T003 = coordination sync, T026 = this pass)
- Coverage: 100% · Ambiguity: 0 · Duplication: 0 (FR↔SC pairing intended) · **Critical: 0**

## Resolutions
- **I1 — resolved.** plan.md Summary reworded to "coordinated with — not built on" the refactor; current injection stated.
- **I2 — resolved.** plan.md Project Structure source tree updated to the `connection/`/`acceptance/`/`fanout/` module layout; Structure Decision corrected to current injection.
- **I3 — resolved.** tasks.md file paths updated to the module layout (`connection/seeded_bounded.rs`, `acceptance/{mod,bounded}.rs`); T021 corrected to the seeded-loop unit test in `seeded_bounded.rs`.
- **O1 — no action.** Tests are complete and green for every correctness claim; the note is about commit ordering (test-first not separately evidenced), not a missing test. Future commits can split test/impl for strict TDD evidence.
- **O2 — resolved.** T004 ticked (the `rejected` `ConnectionScript` step was added; bounded-node construction reused the existing `node_with_strategy` helper).
- **T003** remains open — a coordination sync with the co-developing architect on shared-file edits, not a code task.

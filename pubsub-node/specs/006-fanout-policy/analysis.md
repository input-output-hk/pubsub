# Analysis Ledger — 006 Message Publishing and Fan-out Forwarding

`/speckit-analyze` findings and resolutions, per Constitution Development Workflow (the ledger, not commit messages, closes a finding). Mirrors `specs/004-connections/analysis.md`.

## Session 1 — 2026-06-16 (post-tasks, pre-implementation)

Cross-artifact pass over spec.md, plan.md, tasks.md with research.md, data-model.md, contracts/fanout-protocol.md, ADR 0020 as supporting artifacts. No code yet → cross-artifact consistency + coverage is the whole job.

### Outcome: GO — 0 CRITICAL, 0 HIGH, 0 MEDIUM, 3 LOW (none blocking)

Full requirement→task coverage; no orphan requirement; no task lacking an artifact basis; no constitution conflict. The three LOW items are deliberate or functionally-covered; no spec/plan/tasks edits required.

### Coverage (100%)

| Group | Count | Covered by | Status |
|-------|-------|------------|--------|
| FR-001..005 (publish) | 5 | T003/T004/T005 | ✅ |
| FR-006..009 (fan-out + split-horizon) | 4 | T003/T004 (publish), T007/T008 (receive) | ✅ |
| FR-010 (strategy injected) | 1 | T001 + T005 (structural; CHK031) | ✅ |
| FR-011 (Effect::Send, no new variant) | 1 | T004 (fanout helper) | ✅ |
| FR-012/013/015 (dedup) | 3 | T010/T011 | ✅ |
| FR-014 (Origin) | 1 | T002 + T003 (Local) + T007 (Peer) | ✅ |
| FR-016 (empty downstream) | 1 | T003 | ✅ |
| SC-001 | 1 | T006 | ✅ |
| SC-002 | 1 | T009 (partial) + T012 (full) | ✅ |
| SC-003 / SC-005 | 2 | T010/T012 | ✅ |
| SC-004 (split-horizon) | 1 | T007 + T009 | ✅ |
| SC-006 | 1 | T003 + T006 | ✅ |
| US1 AS1–4 | 4 | T003/T006 | ✅ |
| US2 AS1–5 (incl. AS5 verbatim) | 5 | T007/T008/T009 | ✅ |
| US3 AS1–4 (incl. AS4 no-poisoning) | 4 | T010/T012 | ✅ |

All 17 tasks trace to an artifact (plan rows, R1–R9, data-model, contracts, ADR 0020). No unmapped tasks.

### Focus-area verdicts (per the analyze input)

1. **Task coverage incl. late scenarios** — US2 AS5 (verbatim) → T007; US3 AS4 (publish-path no-poisoning) → T010. Both pinned. ✅
2. **R9 shared-helper coherence** — `validate_dissemination` / `record_and_fanout` consistent across research R9, data-model §2/§4, T004/T008/T011. The incremental layering is coherent and **explicitly flagged**: T004 builds `record_and_fanout` *without* dedup ("NO dedup yet"), T011 adds the dedup gate "inside `record_and_fanout`"; R9/data-model describe the final (with-dedup) form. No contradiction — the end-state artifacts and the incremental tasks agree. ✅
3. **Cyclic-ordering hazard** — present as a binding header note + dependencies in tasks.md; consistent with spec US3 "why this priority" (US1/US2 acyclic before dedup). US2 tasks/scenarios are acyclic (T007/T009); the triangle/full-mesh payload test is in US3 (T012); T009 carries the cycle-verification of pre-existing suites. No contradiction. ✅
4. **Constitution alignment** — TDD test-first explicit (T003→T004, T007→T008, T010→T011); two recorded compile-coupled exceptions (T001 pure helper, T002 reshape); not-parity rework chartered (T015); ADR 0020 covers the structural decisions (seam + dedup + Origin); logs-not-a-test-surface + parse-at-edge + declarative-test-construction honored in the header/conventions. ✅

### Findings

| ID | Category | Severity | Location | Summary | Resolution |
|----|----------|----------|----------|---------|------------|
| L1 | Terminology | LOW | spec/contracts/tasks | `Fanout`/`fanout` (types, module) vs "fan-out" (prose) | **Deliberate** — `Fanout*` follows messaging convention (RabbitMQ "fanout exchange"); prose stays "fan-out". Documented in the spec discussion. No action. |
| L2 | Underspecification | LOW | tasks.md T015 | Which existing suites need fan-out rework is discovery-dependent ("only as required") | **Accepted** — inherently informed by T009's cycle-verification of which suites have downstream+payload interplay; a polish task, not a behavioral gap. No action. |
| L3 | Coverage | LOW | contracts §1.6 vs T010 | Re-publish-identical → `duplicate` is not a *named* test scenario | **Resolved** — added the re-publish-identical scenario to T010 (contracts §1.6 now directly pinned; confirms publish-path `seen` insertion). |

### Metrics

- Total requirements: 16 FR + 6 SC = 22; acceptance scenarios: 13.
- Total tasks: 17. Coverage: 100% (every FR/SC/scenario ≥1 task). Unmapped tasks: 0.
- Ambiguity (blocking): 0. Duplication: 0. Constitution conflicts: 0. CRITICAL: 0.

### Convergence

Single pass reached zero blocking findings. This follows three converged checklist passes (traceability 4→0, then 1→0) that had already tightened the acceptance scenarios, so the cross-artifact surface entered analyze clean. No second analyze session required pre-implementation; the constitution-valued **post-implementation** analyze pass (verify artifact claims against code — lib.rs re-exports vs contracts §5) is chartered as task T016 during `/speckit-implement`.

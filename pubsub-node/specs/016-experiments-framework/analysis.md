# Analysis Ledger — 016-experiments-framework

Findings from `/speckit-analyze` runs and their resolutions (Constitution,
Development Workflow: the ledger, not commit messages, closes findings).

## Session 1 — 2026-07-19 (post-tasks, pre-implementation)

Scope: spec.md (post-clarify, incl. the 2026-07-18 delegation session) ↔
plan.md + research/data-model/contracts/quickstart (post-checklist
convergence) ↔ tasks.md (T001–T026, incl. the T026 amendment).
Constitution v1.2.0 gates evaluated. No CRITICAL or HIGH findings; coverage
100% (every FR/SC ≥ 1 task); duplication 0.

| ID | Category | Severity | Location(s) | Finding | Resolution |
|----|----------|----------|-------------|---------|------------|
| C1 | Coverage | MEDIUM | FR-010 ↔ T006/T007/T015 | The publishes-per-run knob (default 1; repeat publish with fresh messages, no state reset) was covered only by adjacency — no task text owned the repetition behaviour or its test | **Resolved**: named explicitly in T006 (failing-test scope), T007 (orchestration), and T015 (config surface) |
| C2 | Coverage | MEDIUM | SC-005 ↔ T012/T016 | Record boundedness (nothing O(N)) had no explicit test citation — the shape was built (T012) but never asserted against N | **Resolved**: T016 gains the boundedness assertion — two runs differing only in N at fixed target_degree; histogram lengths bounded by realised max degree/depth + 1 and near-constant across N; no array field scaling with N; structural inventory pinned by the golden serialization test. (Noted: in degenerate topologies — full mesh — max degree is N−1 and the bound holds definitionally; the test uses the bounded-degree configuration where N-independence is visible.) |
| I1 | Inconsistency | LOW | T026 ↔ plan.md structure | T026's `docs/experiments/m2-comparison.md` location was absent from the plan's structure tree (the T026 amendment postdates the plan) | **Resolved**: `docs/experiments/` added to plan.md's source-structure tree with the amendment note |
| A1 | Ambiguity | LOW | contracts/sweep-config.md | `--workers` default "number of cores, capped" leaves the cap unspecified | **Resolved** (2026-07-19, follow-up): default = available cores, no invented cap; the flag row now carries the memory guidance (each in-flight run holds a full population — size explicitly at large N); result-neutral by contract guarantee 1 |
| A2 | Terminology | LOW | FR-018 ↔ data-model §5 | "sent-to-down" (identity prose) vs `sends.down` (record field) — one concept, two spellings | **Resolved** (2026-07-19, follow-up): data-model §5 states the realisation — `sends.down` is the record field for the spec's "sent-to-down" term; one concept, one field |

Metrics: 33 FR + 8 SC · 26 tasks · coverage 100% · ambiguity 1 ·
duplication 0 · critical 0.

### Convergence pass (Session 1, re-scan after remediation) — zero findings

Re-checked the remediation edits first (T006/T007/T015 now cite FR-010's
knob consistently with the spec's fresh-messages/no-reset semantics; T016's
new assertion matches SC-005's wording and the R9 test layering; plan tree
now consistent with T026), then re-ran the detection passes over the
artifact triangle: no new findings.

## Session 2 — 2026-07-19 (after A1/A2 follow-up resolutions) — zero findings

On review it was decided to resolve A1 and A2 in the artifacts rather than
carry them as implementation notes. Re-checked those edits first (the
`--workers` default is now specific and consistent with the quickstart's
memory guidance and the plan's risk note; the `sends.down` realisation
clause is consistent with the spec's identity prose and the output
contract's field listing), then re-ran the full detection passes:
**zero findings — converged.** Metrics: ambiguity 0 · duplication 0 ·
critical 0 · coverage 100%.

Ready for `/speckit-implement` (fresh session per project convention). A
post-implementation analyze round remains required by the constitution's
spec-fidelity rule (artifact claims verified against code once code
exists).

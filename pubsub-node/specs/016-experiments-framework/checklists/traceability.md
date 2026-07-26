# Traceability & Cross-Artifact Consistency Checklist: 016-experiments-framework

**Purpose**: Validate that every spec requirement and success criterion traces
into the plan artifacts without contradiction, that clarify-session
resolutions are reflected everywhere they bite, and that plan artifacts
introduce no claims absent from or conflicting with the spec — before
`/speckit-tasks`.
**Created**: 2026-07-19
**Feature**: [spec.md](../spec.md) · plan.md · research.md · data-model.md · contracts/ · quickstart.md

## Requirement → Plan Traceability

- [x] CHK001 - Do the gating/naming requirements trace into the plan's Cargo/feature/binary design without weakening the disabled-build claim? [Consistency, Spec FR-001–FR-003; plan.md Technical Context & Structure]
- [x] CHK002 - Are the driver requirements (uniform routing, wavefront, kind-agnostic routing, canonical order) each carried by a named plan element (driver module, R1, R2) with the canonical send key defined? [Completeness, Spec FR-004–FR-007; research R1/R2; data-model §3]
- [x] CHK003 - Are all four run phases and both setup modes reflected in the phase machine, including the all-folds-before-Synced / one-wave-Synced barrier and the single-epoch constraint? [Completeness, Spec FR-008–FR-010; data-model §2]
- [x] CHK004 - Do the participant/strategy requirements (Level-1 model, Level-2 headroom, silent relay, uniform sampler incl. the min-degeneracy rule) trace into strategies/population design and R10? [Completeness, Spec FR-011–FR-013; research R10; data-model §1]
- [x] CHK005 - Are churn semantics (post-formation mark, no events, denominators, registries/connections untouched, adversary count unchanged) identical across spec, R11, and data-model? [Consistency, Spec FR-014; research R11; data-model §1/§2]
- [x] CHK006 - Do the publish-drain metric requirements (coverage denominator, depth conventions, miss causes, sends split, accounting identity) trace into metrics design and the RunRecord inventory? [Completeness, Spec FR-015–FR-018; data-model §5]
- [x] CHK007 - Do the graph-analytics requirements (extracted-digraph metrics, SCC goodness pre/post churn, min-publisher-coverage formula, opt-in heavy metric, model dispatch owning extraction, v1 M2-only) trace into graph design and R8? [Completeness, Spec FR-019–FR-022; research R5/R8; data-model §4]
- [x] CHK008 - Does the statistics requirement (distributions/percentiles; counts + Wilson 95%) trace into the statistics module design and the artifact contract's probability-field rule? [Completeness, Spec FR-023; research R7; contracts/output-artifacts §7]
- [x] CHK009 - Do the reproducibility/execution requirements (master-seed derivation, pure run, canonical parallelism) trace into R3/R6 and the sweep design? [Completeness, Spec FR-024–FR-026; data-model §3/§6]
- [x] CHK010 - Is rewritten FR-027 (driver-owned determinism; delegation of core ordering) consistently reflected in R2, the plan's Constraints, and the structure's "core unchanged" note? [Consistency, Spec FR-027; Clarifications 2026-07-18]
- [x] CHK011 - Does the output-contract requirement set (three artifacts, streamed canonical order, bounded rows, derivability, opt-in detail) trace one-to-one into contracts/output-artifacts.md? [Completeness, Spec FR-028–FR-030]
- [x] CHK012 - Do configuration/validation requirements (parse-at-the-edge, single topic, rejection rules) trace into config design and the sweep-config contract's validation list? [Completeness, Spec FR-031; contracts/sweep-config]
- [x] CHK013 - Do scripted-topology and M2-demonstration requirements (both shipped configs + smoke, methodology note) trace into scripted design, the shipped-config list, and the quickstart procedure? [Completeness, Spec FR-032–FR-033; quickstart]

## Success Criteria → Plan Traceability

- [x] CHK014 - Is each success criterion carried by a plan element that can produce it (SC-001 determinism tests incl. worker counts; SC-002 scripted exactness; SC-003 cross-check + identity; SC-004 replay; SC-005 bounded rows; SC-006 timings; SC-007 Wilson-with-counts; SC-008 feature-off build)? [Completeness, Spec SC-001–SC-008; plan Technical Context; research R9]
- [x] CHK015 - Are the SC-006 performance numbers stated identically wherever they appear (spec, plan performance goals, quickstart expectations)? [Consistency, Spec SC-006]

## Clarify-Session Resolutions Reflected

- [x] CHK016 - Is the no-resume/interruption resolution reflected in the output contract, the quickstart, and the edge-case list, with identical prefix semantics? [Consistency, Clarifications 2026-07-17 Q1; contracts/output-artifacts §6]
- [x] CHK017 - Is the both-configs resolution (operating point + bulk-regime) reflected in FR-033, the shipped-config list, and the quickstart's two-run procedure? [Consistency, Clarifications 2026-07-17 Q2]
- [x] CHK018 - Is the sampler degeneracy resolution (min(target_degree, available)) reflected wherever the sampler is described? [Consistency, Clarifications 2026-07-17 Q3; research R10]
- [x] CHK019 - Is the counts + Wilson 95% resolution reflected in FR-023, SC-007, the artifact contract, R7, and the quickstart's methodology-note step? [Consistency, Clarifications 2026-07-17 Q4]
- [x] CHK020 - Is the core-ordering delegation resolution reflected in FR-027, plan constraints, R2, and the risks paragraph — with no artifact still implying 016 converts core collections? [Consistency, Clarifications 2026-07-18]

## Plan-Introduced Claims (no conflict with / silent extension of the spec)

- [x] CHK021 - Does the plan's public-surface wording (lib.rs `pub mod experiments` under the feature flag) align with FR-001's disabled-unchanged claim and the spec's "nothing new exported" placement statement, without contradiction? [Conflict?, plan Structure vs Spec FR-001]
- [x] CHK022 - Is the pre-churn field presence rule consistent between the spec's churn-0 edge case ("verdicts coincide, recorded once"), the RunRecord inventory, and the aggregates entries? [Consistency, Spec Edge Cases; data-model §5/§6]
- [x] CHK023 - Is the aggregates-fold invariant `full_coverage.count ≥ good.count` traced to spec grounds (SC-003 drain≡reachability + FR-020) rather than standing as an unsourced plan claim? [Traceability, data-model §6]
- [x] CHK024 - Are plan-level defaults absent from the spec (worker-count default, stderr progress, coverage bin width as a module constant, configs/ file locations) confined to invocation/plan territory the clarify coverage summary deferred — never contradicting a spec statement? [Assumption, contracts/sweep-config; data-model §6]
- [x] CHK025 - Do the three planned ADRs (0032/0033/0034) cover every structural decision the plan introduces (driver architecture, output contract + statistics conventions, new dependency), with none left undocumented? [Completeness, plan Constitution Check III]

## Terminology & Conventions

- [x] CHK026 - Are run/experiment/sweep used with their defined meanings in every artifact (no residual "round of runs"/"config point" phrasing)? [Consistency, Spec Execution structure]
- [x] CHK027 - Are "wave" and "round" used compatibly (spec defines them as the same unit; depth conventions wave 0 = publisher) across spec, data-model, and quickstart? [Consistency, Spec FR-016; data-model §3/§5]
- [x] CHK028 - Is "up-honest" used with its single spec definition (honest ∧ not down) in every artifact that computes denominators or goodness? [Consistency, Spec Driver model; data-model §1]

## Notes

- Findings and their resolutions are recorded below per pass (multi-pass
  convergence: re-run until a recorded zero-finding pass).

### Pass 1 (2026-07-19) — 3 findings, all resolved

- **F1 (CHK021)** — plan.md's structure note claimed "nothing re-exported
  from the library's public surface" while the tree adds the feature-gated
  `pub mod experiments` (needed by the bin target). *Resolution*: plan.md
  structure annotated — the module is the one feature-gated public addition;
  FR-001's normative claim (disabled build unchanged) is unaffected; the
  broader phrasing survives only in the verbatim input records, which are
  historical, not normative.
- **F2 (CHK022)** — data-model §5 listed the `_pre_churn` record fields
  unconditionally, contradicting the spec's churn-0 edge case ("recorded
  once"). *Resolution*: §5 now states the presence rule (present iff
  churn > 0, absent otherwise — absent ≠ zero per the output contract);
  §6 mirrors it for the aggregates' `good_pre_churn` estimate.
- **F3 (CHK023)** — data-model §6's `full_coverage.count ≥ good.count`
  invariant lacked spec anchors. *Resolution*: grounds cited (SC-003
  drain ≡ reachability; FR-020 goodness definition).

### Pass 2 (2026-07-19) — zero findings; converged

Re-checked pass 1's own edits first, then all 28 items: no new findings.
Checklist converged; artifacts ready for `/speckit-tasks`.

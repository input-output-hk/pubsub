# Analysis: 005-peer-view (post-implementation consistency pass)

**Date**: 2026-06-30 (re-mapped 2026-07-02 for the bucketed-pull redesign) · **Scope**: spec.md, plan.md, tasks.md, contracts/, research.md, data-model.md vs the constitution **and** the implementation (US1–US3 merged on the branch). Read-only.

## Findings

| ID | Category | Severity | Location | Summary | Recommendation |
|----|----------|----------|----------|---------|----------------|
| I1 | Inconsistency (doc↔decision↔code) | MEDIUM | plan.md Summary | Summary previously said the feature is "built on a prerequisite determinism/purity refactor … strategies passed as `apply` arguments", contradicting the relaxed-dependency decision (Technical Context + research R6) and the implementation, which retained the current strategy injection. | Reword the Summary to "coordinated with (not built on)" + "current injection retained", matching R6. |
| I2 | Inconsistency (doc↔code) | MEDIUM | plan.md Project Structure (source tree + Structure Decision) | Listed flat `connection.rs`/`acceptance.rs`. Code refactored these under `strategies/` (`connection/`, `acceptance/`, `fanout/`, `config`, `edge`) and kept the current injection. | Update the tree to the module layout; correct the wiring note to current injection. |
| I3 | Inconsistency (mechanism drift) | MEDIUM | all derived docs | Docs described the pre-redesign **seeded-PRNG** mechanism (`SeededBoundedConnection`/`BoundedAcceptance`, `--seed`/degree params, `ChaCha20`). The shipped design is the **verifiable hash-gated (bucketed-pull)** overlay (`HashGatedConnection`/`VerifiableBoundedAcceptance`, `strategies::edge`, `--genesis`/`--rf`/`--cap-buffer`, `Heartbeat { interval }`). | Re-map the derived docs to the bucketed-pull mechanism + the spec.md FR-001..016 / SC-001..007 set (this pass). |
| O1 | Observation (TDD evidence) | LOW | tasks.md / commit history | Tests exist and pass for every correctness claim, but several were committed alongside their implementation rather than as a separate failing-first commit, so the strict "test fails first" ordering isn't independently evidenced from the artifacts. | None required — outcome (tests present + green) satisfies Principle II; note for future strictness. |
| O2 | Task status | LOW | tasks.md T004 | The `ConnectionScript` `rejected` step was added; verifiable-node construction reused the existing `node_with_strategy` helper. | Mark T004 done with a note (helper reuse). |

No CRITICAL or HIGH findings. No constitution MUST violations.

## Coverage Summary (requirement → task/test)

| Req | Has task? | Task/test | Notes |
|-----|-----------|-----------|-------|
| FR-001 predicate selection, degree ≈ RF | ✅ | T007, T008 | unit + integration; `bucket_count` |
| FR-002 pure/verifiable predicate | ✅ | T005 (edge unit), T007, T008 | SHA-256, not `DefaultHasher`; order-independent |
| FR-003 fixed RF; small-topic connect-to-all | ✅ | T007 (`small_topic`/`out_degree_tracks_rf`), T012 | `B=1` floor |
| FR-004 default genesis 0 | ✅ | T007 (`default_genesis_zero`) | |
| FR-005 self/topic folded in | ✅ | T007 (`varies_by_self_id`) | |
| FR-006 interval via Heartbeat; retained | ✅ | T006 (`NodeState.interval`), T009/T015 (interval arg) | driver-fired, no wall-clock |
| FR-007 verifiable bounded acceptance (4-way) | ✅ | T012, T013, T016 | predicate ∧ registered ∧ shared-interest ∧ under `OC` |
| FR-008 silent drop vs explicit Rejected | ✅ | T013, T017 | distinct causes; over-capacity only sends `Rejected` |
| FR-009 Rejected drops pending upstream (no retry) | ✅ | T014, T018 | retry deferred to a future strategy family |
| FR-010 additive defaults | ✅ | T024, main.rs default | unbounded policies selectable |
| FR-011 view = full candidate set; seam for `H_v` | ✅ | T009 (candidates as view), R7 | `H_v` deferred |
| FR-012 single interval (v1) | ✅ | T006 (one heartbeat at readiness) | rotation/teardown deferred |
| FR-013 observable via getters | ✅ | T019, upstream/downstream getters | no rejection-count getter |
| FR-014 ordered structures + pure strategies | ✅ | T005/T006 (`BTreeSet`/`BTreeMap`), T009 | genesis/RF/c fields, interval input |
| FR-015 two-phase construction (ADR 0028) | ✅ | T027 | per-seam params + fallible build |
| FR-016 no incentive/chain layer | ✅ | T023 (deferred), scope | overlay mechanics only |
| SC-001 reproducible (incl. cross-machine) | ✅ | T007, T008 | |
| SC-002 acceptor predicate == dialer's | ✅ | T005 (edge), T008 | verifiability |
| SC-003 uniform over sweep (chi-square) | ✅ | T021 (`edge_density_…`) | |
| SC-004 degree tracks RF; no node exceeds `OC` | ✅ | T007, T008, T012 | |
| SC-005 no amplification (1/B density) | ✅ | T020 | adversary bounded to hash share |
| SC-006 small-topic / unbounded = connect-to-all | ✅ | T024, T007/T012 (`B=1`) | |
| SC-007 Rejected drops pending; under-fill observable | ✅ | T014, T008 | no retry/back-fill |

**Coverage: 16/16 FR + 7/7 SC = 100%** have ≥1 task/test.

## Constitution Alignment

No violations. ✅ I (traceable), ✅ II (tests present + green for all correctness claims; see O1), ✅ III (ADRs 0024/0025/0028/0029/0030), ✅ IV (acceptance-seam signature change + interval threading surfaced as ADR 0025/0030), ✅ V (no spec/formal_spec edits). Engineering standards: ✅ reproducible-from-genesis, ✅ no wall-clock in transition, ✅ assertions via getters/state (not log strings), ✅ parse-at-edge, ✅ declarative test construction.

## Implementation verification (claims vs code)

Confirmed present: `strategies::edge` (`is_valid_edge`/`bucket_count`/`accept_cap`, SHA-256 over a length-prefixed canonical encoding); `HashGatedConnection` + `expected_upstream(…, interval)` (connection/hash_gated.rs); `Admission { Accept, RejectMembership, RejectIllegitimate, RejectOverCapacity }` + `admit(…, interval)` (acceptance/mod.rs); `VerifiableBoundedAcceptance` recomputing the predicate to verify + capping at `OC` (acceptance/verifiable_bounded.rs); `Event::Heartbeat { interval }` + `NodeState.interval` folded in `handle_heartbeat` (event.rs/state.rs); `ConnectionAction::Rejected` (message.rs, tag 0x03) + distinct log causes `membership_validation_failed` / `illegitimate_request` / `downstream_capacity_reached`; `handle_connection_rejected` removes the matching pending `AwaitingAccept` only (state.rs); two-phase builder with `ConnectionParams`/`AcceptanceParams` (strategies/config.rs); CLI kinds `connect-to-all`/`hash-gated`, `accept-from-all`/`verifiable-bounded`, re-exported from lib.rs. Full suite green, clippy + fmt clean. (No failed-set field and no rejection-count getter — retry/back-fill deferred to a future strategy family.)

## Metrics
- Requirements: 23 (16 FR + 7 SC) · Tasks: 28 (T001–T028, incl. T002b; 26 done; T003 = coordination sync, T026 = this pass)
- Coverage: 100% · Ambiguity: 0 · Duplication: 0 (FR↔SC pairing intended) · **Critical: 0**

## Resolutions
- **I1 — resolved.** plan.md Summary reworded to "coordinated with — not built on" the refactor; current injection stated.
- **I2 — resolved.** plan.md Project Structure source tree updated to the `strategies/` module layout (`connection/`/`acceptance/`/`fanout/`/`config`/`edge`); Structure Decision corrected to current injection + `connection_state` for lifecycle state.
- **I3 — resolved.** All derived docs re-mapped from the superseded seeded-PRNG mechanism to the verifiable hash-gated (bucketed-pull) design and to spec.md's FR-001..016 / SC-001..007 set (this pass): `strategies::edge`, `HashGatedConnection`, `VerifiableBoundedAcceptance`, `Heartbeat { interval }`, `--genesis`/`--rf`/`--cap-buffer` throughout.
- **O1 — no action.** Tests are complete and green for every correctness claim; the note is about commit ordering (test-first not separately evidenced), not a missing test. Future commits can split test/impl for strict TDD evidence.
- **O2 — resolved.** T004 ticked (the `rejected` `ConnectionScript` step was added; verifiable-node construction reused the existing `node_with_strategy` helper).
- **T003** remains open — a coordination sync with the co-developing architect on shared-file edits, not a code task.

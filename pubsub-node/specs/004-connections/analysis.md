# Analysis ledger: 004-connections

Findings and resolutions from `/speckit-analyze` passes, per the Development Workflow's
analysis-ledger convention (mirrors `specs/001-minimal-node-scaffold/analysis.md`).
Commit messages are not the ledger; a finding without an entry here is not closed.

## Session 1 — 2026-06-12 (pre-implementation; spec + plan + tasks)

Scope per the agreed analyze input: cross-artifact consistency (coverage, TDD/checkpoint
structure, terminology, task executability, constitution alignment), with
existing-code claims verified against source. Adjudicated items excluded (spec
Clarifications ×9; consistency checklist F1/F2).

### Findings

- [x] **C1** — Coverage — **MEDIUM** — `tasks.md` T009 / spec US1-AS3, FR-008
  T009's scenario list covers setup-dials-all, empty view, self-exclusion, and
  repeated-setup diff behavior, but omits the explicit US1-AS3 assertion that
  **membership updates arriving after setup trigger no new requests** (selection runs
  only on setup events). The behavior is implemented by construction
  (`handle_membership_update` returns no effects), which is precisely why an explicit
  regression pin belongs in the state suite.
  _Recommendation: add the scenario to T009's list ("membership update after setup →
  no entries added, no Send effects")._
  _Fixed 2026-06-12: T009 gains the two-sided scenario — the membership update **folds
  into candidates** (state change asserted) **but creates no connection entries and
  returns no effects**; a subsequent setup event then dials the new member. Wording
  sharpened per maintainer clarification: "trigger" means effects/entries from `apply`,
  not the state folding, which continues unchanged._

- [x] **T1** — Task accuracy / green checkpoint — **MEDIUM** — `tasks.md` T012
  T012 lists `src/main.rs` and `tests/common/mod.rs` as the `Node::new` call sites to
  update in the signature-change increment, but `tests/candidate_set.rs:30` calls
  `Node::new` directly (verified against source). As written, the T012 increment would
  not compile — a green-checkpoint break.
  _Recommendation: add `tests/candidate_set.rs` to T012's call-site list._
  _Fixed 2026-06-12: T012 now reads "every `Node::new` call site … `src/main.rs`,
  `tests/common/mod.rs`, and the direct call in `tests/candidate_set.rs`"._

- [x] **D1** — Duplication / executability — **LOW** — `tasks.md` T004, T005, T014
  `src/lib.rs` re-export responsibility is split ambiguously: T004 "module wired in
  src/lib.rs with re-exports per contracts §4", T014 "complete the `src/lib.rs`
  re-export delta per contracts §4", while T005's new message types have no named
  export owner (implicitly T014). Two tasks editing the same lib.rs region against the
  same contract list invites either a double edit or a silent gap.
  _Recommendation: make T004 export only the connection-module types it creates; make
  T014 the single owner of the remaining delta (message types + any stragglers),
  naming T005's types explicitly._
  _Fixed 2026-06-12: T004 scoped to its module's three types; T014 named single owner
  of the rest, listing T005's message types — with the correction that
  `keypair_from_alias` needs no lib.rs work (a method on the already-exported
  `MockCryptoScheme`)._

### Coverage summary

- FR-001..028: every FR maps to ≥1 task (FR-004/022/023/026..028 are
  negative/preservation requirements covered by absence-by-design plus the T032
  verify-against-code pass and T029/T030 deferral notes). FR-008 is task-covered but
  scenario-incomplete → C1.
- SC-001..007: all covered (T016, T017/T020, T021/T023, T026, T019, T027, T027).
- Unmapped tasks: none — every task traces to an FR, plan decision row, research
  entry, or post-plan obligation.

### Verified-against-source this pass

`Node::new` call sites (grep over tests/ + main.rs → surfaced T1);
`Signer::public_key` (crypto/mod.rs trait); `EventQueue::push` silent-on-closed
(event.rs); `NetworkSender: Clone` + handle internals (network.rs);
`deny_unknown_fields` placement — on `RawNodeConfig`, so T013's "shadow updated"
wording is accurate (config.rs).

### Clean passes

TDD pairing (T009/T010→T011, T017→T018, T021→T022, T024→T025) and the four sequencing
constraints survive into phase structure; Phase-4 single-commit pairing internally
consistent with green checkpoints; terminology uniform across the six artifacts
(roles, carried emitter vs frame sender, setup event vs timer, drop causes);
constitution gates of plan.md hold over tasks.md (declarative construction T008,
logs-not-a-test-surface, no FR citations in operator strings, ADRs already authored).

### Metrics

| Metric | Value |
|---|---|
| Requirements (FR + SC) | 28 + 7 |
| Tasks | 33 |
| Coverage (≥1 task) | 100% (1 scenario-level gap → C1) |
| Findings | 3 (0 critical, 0 high, 2 medium, 1 low) |
| Ambiguities / placeholders | 0 |
| Duplications | 1 (D1) |

## Session 2 — 2026-06-12 (convergence check after Session-1 remediation)

Targeted verification of the three Session-1 fixes — all clean: T009's two-sided
membership scenario matches US1-AS3/FR-008 and the repeated-setup EC (the no-effects
assertion correctly scoped as a current-behavior regression pin); T012's call-site
list matches the source grep (4 calls / 3 files); the T004/T014 export split covers
the full contracts §4 type delta with the `keypair_from_alias` method correction, and
the Phase-2→3 export window is safe. Broad re-scan findings:

- [x] **P1** — Inconsistency — **LOW** — `tasks.md` parallel-examples line vs T013/T014
  The example claims `T013 ∥ T014 ∥ T015`, but T013 and T014 both edit `src/node.rs`
  (timer producer vs getters + lib.rs); correctly, neither task carries a `[P]` marker,
  so the example contradicts both the markers and the different-files rule.
  _Recommendation: correct the example to `(T013 → T014) ∥ T015`._
  _Fixed 2026-06-12: example now reads "(T013 → T014, serial — both edit src/node.rs)
  ∥ T015"._

Cumulative trajectory: 3 → 1 (low). Everything else clean (coverage 100% both
directions; TDD/checkpoint structure intact; terminology uniform; constitution gates
hold).

## Session 3 — 2026-06-12 (zero findings; converged)

Targeted pass over Session 2's remediation: the corrected parallel example agrees with
the `[P]` markers, the Dependencies section, and the file-overlap rule (T015 is
tests/common-only, genuinely parallel with the serial node.rs pair). Same-class sweep
over every other parallel example (Phases 1, 2, 8): all consistent with their markers
and file sets. Broad re-scan: no new findings. **Cumulative trajectory 3 → 1 → 0 —
pre-implementation analysis converged.** Next analyze obligation: the
post-implementation pass verifying artifact claims against the code (Development
Workflow spec-fidelity rule; carried by tasks T032/T033 plus a closing analyze
session).

## Session 4 — 2026-06-12 (post-013 reconciliation re-convergence)

External change: feature 013 (topic registry) merged to `main` after Session 3; the
branch was rebased and a reconciliation commit updated the artifacts (ADRs renumbered
0017/0018/0019; merged receive chain enumerated in FR-016/017/018; `Node::new`
baseline + quickstart + T012/T015/T019 absorb the topic-registry parameter and suites;
new edge case + staleness row S7 + fifth T029 deferral for the revisit-flagged
membership-only acceptance decision — maintainer-decided, rationale in spec
Clarifications 2026-06-12 referencing the cross-registry ordering invariant raised on
the 013 PR).

Re-convergence walk over the reconciliation's edit surface plus a broad re-scan:

- [x] **R1** — Coverage — **LOW** — the S7 decision lacked a regression pin (the C1
  precedent: deliberate-by-construction behavior needs an explicit test).
  _Fixed 2026-06-12: T010 gains "acceptance succeeds for a membership-valid topic
  absent from the topic registry"._
- Code claims introduced by the reconciliation verified against source:
  `TopicRegistryControl::set_topic`/`remove_topic`, `InMemoryTopicRegistry::new`,
  lib.rs re-exports (quickstart's new calls compile-accurate in shape).
- Coverage both directions re-verified (the new EC maps to T010's pin + T029's
  deferral; no orphan edits); TDD pairs and the Phase-4 single-commit structure
  unaffected; terminology uniform (merged-chain wording identical across six
  artifacts); constitution gates hold; plan-input.md/spec Input verbatim records
  untouched; historical clarify entries retain pre-013 wording by design.

**Cumulative trajectory 3 → 1 → 0 → 1 (reconciliation-induced, fixed in-pass) → 0.**
Checklist counterpart: consistency.md Pass 3.

### Session 4 addendum — maintainer-prompted systematic sweep (same day)

The maintainer correctly challenged that the Session-4 walk verified only the claims
the reconciliation *introduced*, not the prior verifications against the **new** 013
code. Systematic re-sweep (PeerId usage in `src/topic_registry/` + fixtures; all
`Node::new` call sites including doctests; tests/common helper names; main.rs shape):

- [x] **R2** — Task accuracy / green checkpoint — **MEDIUM** — `tasks.md` T012
  The Session-1 `Node::new` call-site grep predates 013: `src/network.rs:142` carries
  a `no_run` **doctest** calling `Node::new` (doctests compile — a real call site),
  and tests/common now holds four calls (013 added `node_sharing`).
  _Fixed 2026-06-12: T012 lists the doctest and notes the four common calls; the plan
  tree's "(only change)" note on network.rs corrected._
- Verified clean: `src/topic_registry/` has zero `PeerId` usage (T003 list stands —
  now verified, previously inferred); the topic-registry fixture is topic-id +
  publisher keys (no alias-rule interaction; `PublisherId`/`PublicKey` untouched by
  the reshape); `await_candidates` exists as T015 claims and 013's helpers already
  register topics; main.rs's new `--topic-registry` loading is covered by T012 (call
  site) and T003 (self_id parse).

**Re-converged after the addendum: trajectory … → 1 → 0.** Lesson recorded: after an
external merge, *re-run* the source-facing verifications — artifact-internal delta
walks do not refresh stale code claims.

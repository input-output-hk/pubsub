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

## Session 5 — 2026-06-12 (final pre-implementation deep pass; full surface)

Scope per the agreed input: all feature artifacts + ADRs 0017–0019 + the current
post-013 code + IMPLEMENTATION_NOTES + the event-loop contract; every code claim
re-verified (none grandfathered); semantic-interaction hunt across the 013×004
surface; edge-case/staleness completeness. Findings (all mechanical; none required a
maintainer decision):

- [x] **S5-1** — Stale claim — **MEDIUM** — the setup timer was described as the
  **third** node-owned producer (R6, ADR 0018 §3, plan row 6); 013 added the
  topic-registry reader, making it the **fourth** (verified: `Node::new` spawns
  mailbox + subscription reader + topic reader). _Fixed 2026-06-12 in all three._
- [x] **S5-2** — Stale claim — **LOW** — T031/plan claimed `Node`'s rustdoc "still
  references the removed subscribe/unsubscribe mutators"; 013's polish already fixed
  that (the doc now describes the two-registry fold). _Fixed: T031 reworded to the
  real obligation (add the connection surface)._
- [x] **S5-3** — Task accuracy — **LOW** — `peer.rs`'s rustdoc example uses
  `as_str()` (a compiling doctest); T002 removes `as_str` but didn't name the
  example. _Fixed: T002 names it._
- [x] **S5-4** — Underspecification — **MEDIUM** — with 013's collapse of
  `Node::subscriptions()` to the effective filter, the strategy's `subscriptions`
  input was ambiguous (field vs snapshot). Pinned everywhere as the
  **membership-derived `NodeState` field** — the dial side deliberately mirrors the
  S7 acceptance rule (same revisit flag). _Fixed: R5, data-model §1.4, spec strategy
  entity._
- [x] **S5-5** — Numbering — **LOW** — IMPLEMENTATION_NOTES now ends at N-009 (013
  added N-008/N-009); T029's five entries take N-010..N-014. _Fixed: T029 notes it._

Semantic interactions verified clean (recorded so they are not re-derived):

- `handle_topic_registry_update` is a pure fold; `Removed` only deletes
  `registered_topics[topic]` — **no** connection effect, matching S7's deliberate
  non-effect and the validation table.
- `handle_membership_update` still drops `candidates[topic]` on own-topic loss —
  consistent with S4 (connections persist; no re-dial after loss, since both strategy
  inputs lose the topic).
- The two 013 drop causes sit before signature verification in
  `handle_signed_message` — the misbehavior boundary (severance iff ①–④ passed) maps
  exactly onto current code order.
- `await_subscriptions` waits on the effective filter — T015/T016's preamble order
  (register topic first) is therefore load-bearing and correctly stated.
- Producer/teardown story (ADR 0019): drop-abort covers all four producers; the
  Shutdown terminal-marker mechanics are unaffected by the added reader.
- `main.rs` (`--topic-registry` + `from_file`) and tests/common's four `Node::new`
  calls covered by T012; T013's config flows through `load_node_config` as planned.
- N-002 and N-006 exist in IMPLEMENTATION_NOTES as T029 describes; contract §1.3
  ("Connections (forward-looking, ~004)") still present — T030's supersession note
  remains accurate and owed.
- plan-input.md / spec Input verbatim records untouched; adjudicated decisions
  unchallenged (factual claims inside them re-verified).

**Go/no-go: GO.** Coverage 100% both directions; zero open findings at any severity;
all code claims current as of the post-013 merge (`2678696`). The remaining analyze
obligation is unchanged: the post-implementation verify-against-code pass
(T032/T033 + closing session).

## Session 6 — 2026-06-13 (T032: post-implementation verify-against-code, contracts §4)

Implementation of 004-connections complete (Phases 1–8: T001–T033). This is the
T032 verify-against-code pass — contracts §4's public-surface claims checked
against the merged code on `004-connections` (Phase-1..8 commits), applying the
003 lesson (compare artifacts to **code**, not just to each other).

### Verified-against-source this pass

- **`PeerId` reshape** — `src/peer.rs` wraps `PublicKey`; `as_str` is **gone**
  (grep: none in `peer.rs`); `FromStr`/`Display` implement the alias rule; serde
  is string-shaped (Serialize via `collect_str`, Deserialize via `FromStr`).
  Matches contracts §4 "Changed".
- **`Node::new` signature** — `src/node.rs` is exactly `(self_id, config,
  network, signer: Arc<dyn Signer>, verifier, subscription_registry,
  topic_registry, strategy: Arc<dyn ConnectionStrategy>)`, in §4's order;
  identity/signer coherence returns the new `NodeError::IdentityMismatch`
  (`src/error.rs`) **before** registration.
- **`NodeConfig`** — gains `connection_setup_delay: Option<Duration>` (TOML
  `connection_setup_delay_ms`), unset by default.
- **Added surface** — `pub async fn Node::shutdown(self)`;
  `upstream_connections()` / `downstream_connections()` getters with §4's return
  types.
- **Re-exports** (`src/lib.rs`) — `UpstreamState`, `ConnectionStrategy`,
  `ConnectToAllCandidates`, `ConnectionMessage`, `PlainConnection`,
  `ConnectionAction` all present alongside the existing message types;
  `MockCryptoScheme::keypair_from_alias` present on the already-exported scheme.
- **Crate-internal invariants (§5 note)** — `NodeState` and `Effect` are
  `pub(crate)`, not re-exported through `lib.rs`.
- **Explicitly absent (§4)** — no `connect`/`disconnect` verb; `ConnectionAction`
  has only `Request`/`Accepted`/`Terminated` (no `Rejected`); no transport change.

**Finding: zero divergences.** Every contracts §4 claim matches the code as
built; no artifact reconciliation was required (a clean verify pass is still a
ledgered pass — the 003 lesson).

### Documentation reconciliation this phase (T029–T031)

- `IMPLEMENTATION_NOTES.md`: N-002 and N-006 marked resolved; N-010 (added
  mid-Phase-6) plus the deferred-dynamics package N-011..N-015, each mapped to a
  data-model staleness row S1–S7 (except N-013 identity-binding — an
  ADR-0017/FR-028 deferral with no staleness row).
- `event-loop-and-registry-contract.md` §1.3: supersession note added (logical
  connections in `NodeState` over the single mailbox; the keyed-producer sketch
  is deferred to a real transport, 009+).
- `Node` rustdoc: now documents the connection surface, the connection-gated
  receive path, severance, and the two teardown paths.

**Go/no-go: GO.** Contracts/quickstart accurate against code; T033 sweep green.
The only remaining obligation is the closing post-implementation
`/speckit-analyze` session.

## Session 7 — 2026-06-13 (closing post-implementation pass; independent, full-surface)

Maintainer-requested independent verify-against-code pass, broader than Session 6's
T032 (which scoped to contracts §4): every adjudicated decision checked against the
built code **and its tests**, plus public surface, task-ledger truth, edge-case /
staleness realization, and constitution spec-fidelity. Read source directly
(`state.rs`, `node.rs`, `lib.rs`, `error.rs`, `peer.rs`) — nothing grandfathered.

### Verified clean against code (with locations)

- **Public surface (§4)**: `lib.rs:53–75` re-exports match the contract exactly
  (`UpstreamState`/`ConnectionStrategy`/`ConnectToAllCandidates` from `connection`;
  `ConnectionMessage`/`PlainConnection`/`ConnectionAction` from `message`);
  `NodeState` and `Effect` both `pub(crate)` (`state.rs:46,161`); `PeerId::as_str`
  absent crate-wide; getters `subscriptions`/`upstream_connections`/
  `downstream_connections` present with the documented signatures (`node.rs:330–377`);
  8-param `Node::new` generic over `<N, R, T>` (`node.rs:125`); `NodeError::IdentityMismatch`
  present (`error.rs:66`).
- **Receive chain (FR-016/017/018)**: `handle_signed_message` orders connection
  (Active upstream) → subscription → topic-registered → publisher-authorized →
  signature, each an early-return drop with the contracts-§3 cause; severance fires
  only at the signature step (reachable only past all four prior checks), removes the
  upstream entry, returns `Effect::Misbehaved`, sends **no** `Terminated`.
- **Coherence (FR-024)**: `*self_id.as_public_key() != signer.public_key()` →
  `IdentityMismatch`, **before** `network.register` (no leak).
- **Diff rule + S5-4 (the load-bearing one)**: `handle_connection_setup` reads the
  **membership-derived `state.subscriptions` field**, not the effective filter;
  Active → `continue`, AwaitingAccept → kept + re-dialed, missing → inserted + Request;
  never removes. The getter `subscriptions_snapshot` computes the
  `∩ registered_topics` intersection separately (`state.rs`), so dial-side membership
  and delivery-side effective-filter are correctly distinct — the S7 decision is
  realized exactly as specified.
- **Membership-only acceptance + S7 pin**: `handle_connection_request` validates own
  topic ∈ membership `subscriptions` AND requester ∈ `candidates` only — registration
  not consulted. Pinned by `request_accepted_for_membership_valid_but_unregistered_topic`
  (`state.rs:1548`).
- **Shutdown (FR-020, ADR 0019)**: `handle_shutdown` emits one `Terminated` per
  `upstream.keys()` (all states, incl. AwaitingAccept) chained with `downstream`, then
  clears both; the event loop executes a `Shutdown` event's effects, **then** breaks
  (`node.rs:181–192`) — notices on the wire before termination, which `shutdown()`
  awaits.
- **Four producers + drop-abort**: mailbox + subscription reader + topic reader +
  (conditional) `setup_timer_producer` (`node.rs:217–229`); `Drop` aborts the loop and
  every producer.
- **Constitution**: zero log/event assertions in `tests/` (state/effect surfaces
  only); zero FR citations in non-comment source; `ConnectionScript` used for
  multi-step state scenarios (`state.rs`, `connection.rs`); all phases landed as green
  commits `dd5679d..b045ef2`.

### Findings

- [x] **V1** — Task-ledger accuracy — **LOW (resolved by implementer)** — Session 5
  predicted the T029 deferral entries would take N-010..N-014; the implementer surfaced
  N-010 (restart inexpressibility) mid-Phase-6, so the package became N-011..N-015 and
  **T029's own text was updated to say so**. Sound deviation, self-recorded — no action.
- [x] **V2** — Spec measurability — **LOW (no code defect; optional)** — SC-004 reads
  "an abruptly restarted node returns to Active with every counterpart it re-requests",
  which scans as an end-to-end claim; per N-010 the literal same-alias restart is
  **inexpressible** on `InMemoryNetwork` (no deregistration), so the healing mechanic is
  verified at the **state level** (`duplicate_request_idempotent_then_stale_on_failed_revalidation`)
  with the limitation documented in `tests/connections.rs:320` and N-010. The behavior
  is correct and deliberately bounded; only SC-004's wording slightly over-promises the
  *test altitude*. Optional one-line spec footnote on SC-004 pointing at N-010;
  **not a merge blocker** (maintainer decision — left unchecked pending your call).
  _Resolved 2026-06-13: maintainer chose the footnote; SC-004 gains a restart-recovery
  note pointing at N-010 (literal same-alias restart inexpressible on the mock; healing
  mechanic state-tested, graduates to end-to-end at 009)._

### N-010 reconciliation

N-010 is **not** an uncatalogued stale flow: it records that the *literal* restart
in US4-AS4/SC-004 cannot be exercised end-to-end on the mock, and routes the healing
mechanic to a state-level test. It neither overlaps nor contradicts S1–S7 (those are
deliberate non-reconciliations of live connection state; N-010 is a mock-transport
expressibility limit). Its deferral trigger (009 real transport / 011–012 persistent
identity, cf. N-008) is correct. Properly placed.

**Go/no-go: GO for opening the PR / merge.** Every adjudicated decision is realized in
code and covered by tests; public surface matches the contract; zero divergences at
CRITICAL/HIGH/MEDIUM. The two LOW findings are a self-resolved numbering note (V1) and
an optional spec-wording footnote (V2) — neither blocks merge. Coverage 100% both
directions; build + tests green at `b045ef2`.

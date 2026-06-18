# Analysis Ledger — 006 Message Publishing and Fan-out Forwarding

`/speckit-analyze` findings and resolutions, per Constitution Development Workflow (the ledger, not commit messages, closes a finding). Mirrors `specs/004-connections/analysis.md`.

## Session 1 — 2026-06-16 (post-tasks, pre-implementation)

Cross-artifact pass over spec.md, plan.md, tasks.md with research.md, data-model.md, contracts/fanout-protocol.md, ADR 0021 as supporting artifacts. No code yet → cross-artifact consistency + coverage is the whole job.

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

All 17 tasks trace to an artifact (plan rows, R1–R9, data-model, contracts, ADR 0021). No unmapped tasks.

### Focus-area verdicts (per the analyze input)

1. **Task coverage incl. late scenarios** — US2 AS5 (verbatim) → T007; US3 AS4 (publish-path no-poisoning) → T010. Both pinned. ✅
2. **R9 shared-helper coherence** — `validate_dissemination` / `record_and_fanout` consistent across research R9, data-model §2/§4, T004/T008/T011. The incremental layering is coherent and **explicitly flagged**: T004 builds `record_and_fanout` *without* dedup ("NO dedup yet"), T011 adds the dedup gate "inside `record_and_fanout`"; R9/data-model describe the final (with-dedup) form. No contradiction — the end-state artifacts and the incremental tasks agree. ✅
3. **Cyclic-ordering hazard** — present as a binding header note + dependencies in tasks.md; consistent with spec US3 "why this priority" (US1/US2 acyclic before dedup). US2 tasks/scenarios are acyclic (T007/T009); the triangle/full-mesh payload test is in US3 (T012); T009 carries the cycle-verification of pre-existing suites. No contradiction. ✅
4. **Constitution alignment** — TDD test-first explicit (T003→T004, T007→T008, T010→T011); two recorded compile-coupled exceptions (T001 pure helper, T002 reshape); not-parity rework chartered (T015); ADR 0021 covers the structural decisions (seam + dedup + Origin); logs-not-a-test-surface + parse-at-edge + declarative-test-construction honored in the header/conventions. ✅

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

## Session 2 — 2026-06-18 (post-implementation of Phases 1–2, post-014 rebase)

Deep pass after Phases 1–2 (T001–T006) were implemented and the branch rebased onto merged 014. Verified the **reconciled artifacts against the actual code** (`src/{state,fanout,received,event,node,lib}.rs`, `tests/dissemination.rs`) and confirmed the 014-interaction deferrals (N-011/N-017/N-018/N-019) are uncontradicted.

### Outcome: GO / **CONTINUE** — 0 CRITICAL, 0 HIGH, 2 MEDIUM, 1 LOW

> **Addendum (2026-06-18, from the implementing session's closing notes):** A2 below was surfaced by the session that implemented Phases 1–2 and missed by this analyze pass's first read — recorded and resolved here.

The feature is self-consistent across spec/plan/tasks/code on the implemented surface; two documentation tidy-ups (below) and no behavior change. The continue-vs-revert evidence (part E) strongly favors **continuing** from the reconciled code.

### (A) Artifact ↔ code fidelity — PASS

- `fanout.rs` (`FanoutStrategy` trait + `ForwardToAll`) matches data-model §1.2 / contracts §2.2. The scaffolded `cfg(test)` no-op (`ForwardToNobody`) was found unused + unusable by integration crates and **removed** (see A2); `FanoutStrategy`/`ForwardToAll` are the only fan-out exports. ✅
- `received.rs` (`Origin { Local, Peer }`, `ReceivedDelivery.origin`) matches §1.1 / §5; `event.rs` `Event::Publish(SignedMessage)`; `node.rs` `publish` (fire-and-forget) + `Node::new` `fanout_strategy` param + `received_messages` getter; `lib.rs` re-exports `FanoutStrategy`/`ForwardToAll`/`Origin`/`ReceivedDelivery`. All match contracts §5. ✅
- Rebase-reconciliation items: `validate_dissemination` authorization now uses `TopicEntry::is_publisher_authorized` (014) — consistent and noted in data-model; `node_state` auto-registers subscription topics — noted; deleted publish-drop "subscribed-but-unregistered" case — see A1.

### (B) DONE stories under 014 — HOLD

US1 (publish + first-hop fan-out) and the Phase-1 scaffold are sound under 014. `Origin`/`FanoutStrategy`/`Event::Publish`/dedup-seam are orthogonal to the registry rework. No US1 acceptance scenario assumed subscribe-without-register — US1 AS4 lists exactly three drop causes (not-subscribed / unauthorized / signature), already consistent with 014 and with the code. The defensive `topic_not_registered` guard is unreachable on the subscribed path under the invariant but harmless (matches 014's own receive path).

### (C) PENDING stories under 014 — HOLD

US2 (T007–T009) and US3 (T010–T012) are unaffected: 014 changed the registry/connection substrate, not fan-out or dedup. The cyclic-ordering hazard still applies (receive fan-out without dedup loops; US2 stays acyclic, cyclic test waits for US3). Test establishment is viable post-014 — the green US1 integration test builds downstream through the real path via `node_with` (registries populated before `Node::new` → indexer folds → `Event::Synced` → dial) + `establish_upstreams`; `Event::ConnectionSetup` is retained as the dial action, so the scripted partial/line topology for T009 remains constructible. No blocker.

### (D) 014-interaction coverage — uncontradicted (1 new LOW)

- **N-017** (topic-`Removed` cascade clears `upstream`/`downstream`): `fanout()` reads `state.downstream` at the record point, so a cascade before fan-out simply removes targets — consistent, no contradiction. ✅
- **N-011/N-019** (membership-loss retains connections): delivery on an unsubscribed topic is gated (`topic_not_subscribed`) and publish requires subscription, so a retained-stale downstream is never fanned to — consistent. ✅
- **N-018** (`synced` gates dialing, not acceptance/receive): see D1 — N-018 enumerates acceptance + receive but not the **publish** path; `publish` while `!synced` is benign (empty `subscriptions` → `topic_not_subscribed` drop) but unnamed by the note.

### Findings

| ID | Severity | Location | Summary | Recommendation |
|----|----------|----------|---------|----------------|
| A1 | MEDIUM | tasks.md T003 vs spec US1-AS4 + code | T003 enumerates **four** publish-drop scenarios incl. "not-registered", but the implemented test has **three** (the not-registered case was deleted as unreachable under 014's invariant) and spec US1-AS4 also lists only three (not-subscribed / unauthorized / signature). Tasks wording is stale. | Update T003 to three scenarios; note the `topic_not_registered` guard is defensive-under-014 (unreachable on the publish path, not unit-tested there). |
| A2 | MEDIUM | spec/research/data-model/quickstart/plan/tasks T001+T015 vs code | Six artifacts said connection-lifecycle **integration** suites inject the `fanout::test_support` no-op (`ForwardToNobody`), but it is `#[cfg(test)] pub(crate)` — compiled out when the crate is a dependency, so **invisible to `tests/` crates**. The intended injection is impossible (and unnecessary: `connections.rs` is green with the public `ForwardToAll`); `ForwardToNobody` was also **unused**. | **Resolved**: the unused `ForwardToNobody` + its `test_support` module were **removed** from `src/fanout.rs` (build + full suite green); all six artifacts corrected — integration suites use public `ForwardToAll`, no-fan-out unit assertions use empty-downstream, no test-only no-op exists. Verbatim spec Input flagged as superseded. |
| D1 | LOW | IMPLEMENTATION_NOTES N-018 | N-018 names acceptance + receive as the `synced`-ungated paths but not the **publish** path; `publish` while `!synced` is the same class (drops `topic_not_subscribed` against cold `subscriptions`). | **Resolved**: N-018 scope extended to name the publish path. |

### (E) Continue-vs-revert evidence

**Extent of 014's impact on the implemented Phase-1/2 surface — small and localized:**

- `fanout.rs`, `received.rs` (`Origin`), `event.rs` (`Event::Publish`): **zero** 014 impact (orthogonal new surface).
- `node.rs`: `Node::new` gained the `fanout_strategy` param alongside 014's params; `publish` untouched. Clean.
- `state.rs`: the **only** real touch point — `validate_dissemination`'s authorization swapped from a raw `BTreeSet` check to `TopicEntry::is_publisher_authorized` (one call), plus removing the now-duplicated inline receive-path checks (014's version subsumed by the helper). `handle_publish` / `record_and_fanout` / `fanout` are unchanged by 014.
- tests: one publish-drop case deleted (unreachable under the invariant); `node_state` seeds registered topics (one loop). Full suite green.

**Architectural soundness:** the reconciled code is **sound, not merely patched**. 014 changed the *substrate* `validate_dissemination` reads; 006's fan-out/publish/dedup design sits cleanly on top and needed only the one authorization-call swap. The R9 shared-helper factoring made the reconciliation a one-line change — evidence the seam was well-placed. **No 006 design decision was invalidated.**

**Evidence verdict (decision deferred to maintainer):** the cost of reverting Phases 1–2 and re-implementing would discard correct, green, minimally-adapted code for no architectural gain; the impact surface is one authorization call + a dedup of redundant checks + test-setup seeding. The evidence favors **continue**.

### Artifacts to update before Phase 3

- tasks.md T003 (A1) — three publish-drop scenarios.
- IMPLEMENTATION_NOTES N-018 (D1) — name the publish path.

Both are documentation-only; neither blocks Phase 3 on correctness. No CRITICAL/HIGH; Phase 3 (US2 relay) may proceed once A1/D1 are tidied.

## Session 3 — 2026-06-18 (Phase 3 implementation: T007–T009, US2 relay)

Implemented Phase 3 (T007 failing-first state tests → T008 wires `record_and_fanout(Origin::Peer(from), Some(&from))` into `handle_signed_message` → T009 acyclic-line integration). Full sweep green (fmt + clippy + 104 lib + integration suites). The one substantive finding is the T009 pre-existing-suite cycle verification, which **corrected a plan/tasks assumption**.

### (F) Finding — the "star/2-node cycle-free" assumption was only half right

| ID | Severity | Location | Summary | Resolution |
|----|----------|----------|---------|------------|
| F1 | MEDIUM | tasks.md header + plan / spec Assumptions vs code | The ordering-hazard note asserts "the 004 star and 2-node suites are cycle-free by split-horizon." **2-node is** (split-horizon empties the single back-edge), but the multi-node suites are **not stars** at the connection layer: `four_node_star_fixture` (`n_node_graph`) and the 3-node `topic_registry_network` subscribe every node to **one shared topic**, and `establish_upstreams`/`ConnectToAllCandidates` dials **every** co-member — so each builds a **full bidirectional mesh**, not a hub-and-spoke. Once T008 wires receive-path fan-out (no dedup until US3/T011), a payload circulates the mesh unbounded; the suites' per-node "exactly one" counts blow up (observed 25 / 915 / 25729 records before the assertions trip). | **Empirically verified** (each suite run under a watchdog). The genuinely cycle-free suites (`two_node_ping`, `topic_filter`, `topic_validity`, `connections`, `candidate_set`, `topic_runtime`) stay green. The two mesh suites are **`#[ignore]`-deferred** with a reason + tracking ref to **T015** (rework onto a controlled topology) and **T012** (the cyclic "exactly once" guarantee dedup provides) — exactly T009's "deferred to T012's note" branch. Reworking them now is T015 (Phase 5), out of Phase-3 scope. |

### (G) US1 `dissemination` star reworked (in-scope, 006's own suite)

T006's US1 test established its two downstream via `establish_upstreams` over a shared topic, which (same cause as F1) created a `d1↔d2` spoke edge; T008's receive fan-out then had d1/d2 relay P's message to each other (d1 recorded 2 copies). Reworked to a **controlled star**: each spoke is told only about the hub P, so it dials only P and no spoke-to-spoke edge forms. This is `dissemination.rs` (006's own suite, maintained across T006/T009/T012), not a T015 suite.

### (H) Controlled-topology test machinery (T009 helpers)

The all-candidates policy over one shared registry can only build a full mesh, so an **acyclic** line/star on a single topic is scripted by **pinning each node's dialed edges** rather than withholding candidates. New `tests/common` machinery: a test-harness `ConnectToExplicit(Vec<(PeerId, TopicId)>)` connection strategy (dials a fixed edge set, ignoring candidates; acceptance still uses the real candidate set) plus a fluent `node(registry, network, id).topic(t).dials(&[(&hub, &t)] | .dials_nobody()).build()` builder over `node_with_strategy`. Each node dials only its declared edges, so no unwanted mesh edge forms; readiness (`Synced`) is irrelevant — the node only ever dials its explicit set, and acceptance never consults `synced` (N-018), so the auto-readiness dial is harmless and no `is_synced` gymnastics are needed. Establishment reuses the existing `establish_upstreams` (re-dial after candidate convergence). **No production code is test-shaped**: `ConnectToExplicit` lives in `tests/common` (a strategy in `src` would risk shipping the node mis-wired, and a `#[cfg(test)]` one would be invisible to integration crates — the A2 lesson); `ConnectToAllCandidates`/`ForwardToAll` remain the only strategies on the node's public surface. (An earlier draft scripted this via empty-registry `bare_node` + event-queue injection; replaced by the declarative strategy on review.)

### Outcome: GO — Phase 3 complete, green at the ⛳

US2 relay (publish + first-hop + onward relay with split-horizon) is implemented and observable on an acyclic line. The cyclic-mesh "exactly once" case and the two deferred suites are correctly held for US3 (T010–T012). No CRITICAL/HIGH. Phase 4 (US3 dedup) not started, per scope.

## Session 4 — 2026-06-18 (Phase 4 implementation: T010–T012, US3 dedup)

Implemented Phase 4 (T010 failing-first state tests → T011 `seen: HashSet<MessageHash>` + dedup gate inside `record_and_fanout` → T012 cyclic triangle integration). Full sweep green (fmt + clippy + 108 lib + integration suites; the cyclic test terminates in ~0.1s).

### Dedup design as built
- The gate is the single line `if !state.seen.insert(MessageHash::of(&signed.plain)) { …drop "duplicate"… }` at the top of `record_and_fanout` — `insert` returning false-if-present is the combined check-and-insert. Because both `handle_publish` and `handle_signed_message` route through `record_and_fanout` **after** their verification step, both paths dedup identically and a verification failure (publish plain-drop or receive severance) never reaches the gate, so it cannot poison `seen` (FR-013). The `duplicate` drop log lives at this shared point and is path-agnostic (self_id/topic/publisher_id; no `from`) — consistent with logs-not-a-test-surface.
- `seen` is unbounded (in-memory); bounding is deferred (D1, a Phase-5/T013 IMPLEMENTATION_NOTES entry).

### Sanity-check of the deferred suites under dedup (per Phase-4 input)
Ran the two `#[ignore]`-deferred suites via `cargo test --test … -- --ignored` (no source change, so nothing to restore) under a watchdog, to confirm dedup eliminated the unbounded circulation:
- **`n_node_graph`** — **terminates** (exit 101, ~0.24s; pre-dedup it ran away to 25729 records). Now bounded: every node records each distinct message exactly once. The count assertions still fail because they assumed a hub-and-spoke star, but the real topology is a full mesh where every node receives every message — that reframing is the **T015** rework, not a dedup gap.
- **`topic_registry_network`** — **terminates and now passes** (exit 0): each node records the authorized message once and drops the unauthorized one. Could simply be **un-ignored at T015**.

Both left `#[ignore]`d (T015 owns the rework); no commit touched them. The loop hazard the F1 finding flagged (Session 3) is confirmed closed by dedup.

### Outcome: GO — Phase 4 complete, green at the ⛳

US3 dedup spans both paths, suppresses cyclic circulation (triangle records-once + terminates), and does not poison on failed verification. No CRITICAL/HIGH. Phase 5 (polish — T013–T017, incl. the deferred-suite rework T015 and the D1–D5 IMPLEMENTATION_NOTES entries) not started, per scope.

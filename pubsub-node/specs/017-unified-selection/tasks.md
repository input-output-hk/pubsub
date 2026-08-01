# Tasks: Unified selection plane

**Input**: Design documents from `/specs/017-unified-selection/`

**Prerequisites**: plan.md, spec.md (post-clarify, post-checklist),
research.md (R1–R12), data-model.md, contracts/{node-cli,sweep-config}.md,
quickstart.md

**Tests**: MANDATORY-first for the TDD-critical set (research R12): the
`Selection` draw semantics, the `UnifiedAcceptance` admission matrix, the
commit-A equivalence pin, the M4 topology properties, and the experiments
byte-identity/determinism gates. Test tasks precede and must fail before
their implementation tasks. Non-critical (tests-with): CLI validation
matrix, config parsing, docs examples.

**Commit mapping (017 FR-026 — the pinned A→B ordering)**: the **commit A
milestone** closes Phase 3 (T017: baselines byte-diff identical under the
reproduced derivation); the **commit B milestone** closes Phase 6 (T028:
re-baseline after the honest-preimage swap). Phases 4–5 land as ordinary
green-checkpoint commits between the two gates; every task leaves the repo
compiling with the full suite green in both configurations.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: parallelizable (different files, no dependency on an incomplete task)
- **[Story]**: user story label (US1–US5); Setup/Foundational/Polish carry none

## Phase 1: Setup

**Purpose**: pin the starting state the byte-identity gate measures against.

- [x] T001 Verify the branch-point state: full suite green in both
      configurations (`cargo test`, `cargo test --features experiments`),
      `cargo fmt --check`, `clippy --all-targets -- -D warnings`; re-run the
      recorded baseline sweeps and byte-diff `runs.jsonl`/`aggregates.json`
      per `notes/experiments-baselines/README.md` to confirm pre-change
      identity at the branch point

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the two new types land additively (old machinery untouched, so
every commit stays green) with their tests written first.

**⚠️ CRITICAL**: no user story work begins until this phase is complete.

- [x] T002 [P] Write failing unit tests for `Selection` draw semantics in
      `src/strategies/connection/selection.rs` (test module): exactly
      min(pick count, survivors) without replacement; gate-then-pick
      composition (dialed ⊆ predicate survivors); order-independence in the
      candidate set; boundary values (pick count absent = all survivors,
      0 = empty; bucket count absent ≡ 1); repeated-call stability (the
      heartbeat re-dial primitive); per-kind hash domains; symmetric
      predicate composition — via `strategies/test_support` fixtures
      (017 FR-001, FR-002, FR-004)
- [x] T003 [P] Write failing unit tests for `UnifiedAcceptance` in
      `src/strategies/acceptance/unified.rs` (test module): the 2×2
      admission matrix (gate × cap); membership prelude reuse (idempotent
      re-accept, membership silent drop); gate `None` admits without
      predicate; gate `Some(B)` → `RejectIllegitimate` on predicate failure;
      cap reached → `RejectOverCapacity`; cap 0 refuses every new link;
      per-kind disjoint capacity scan; symmetric predicate verification
      (017 FR-010, FR-011, FR-013)
- [x] T004 Write the failing commit-A equivalence pin: `Selection` at
      (bucket count absent, pick count = K) reproduces
      `UniformSampler::expected_links` value-for-value over identical views
      and seeds (asserted directly against the still-present
      `UniformSampler`; re-pinned as fixture values in T016 when the sampler
      is deleted) in `src/strategies/connection/selection.rs` (017 FR-025,
      FR-026; research R2)
- [x] T005 Implement `Selection` in `src/strategies/connection/selection.rs`
      (+ export in `src/strategies/connection/mod.rs`): gate via
      `is_valid_edge`/`is_valid_edge_publisher`/`is_valid_edge_sym` at the
      stored bucket count, then the seeded draw with the **commit-A
      derivation** (`experiments/uniform-sampler/v1`, concatenated preimage,
      no nonce/self-id, `ChaCha20Rng` + `rand::seq::index::sample` over the
      sorted self-excluded survivor order) — T002 + T004 pass (research R1,
      R2)
- [x] T006 Implement `UnifiedAcceptance` in
      `src/strategies/acceptance/unified.rs` (+ export in
      `src/strategies/acceptance/mod.rs`): `admit_prelude` first, then gate
      (`gate: Option<usize>`), then cap (`accept_cap: Option<usize>`) —
      T003 passes (research R1; data-model decision order)
- [x] T007 Reshape construction in `src/strategies/config.rs`:
      `SelectionParams { self_id, kind, symmetric, bucket_count, pick_count,
      seed }` and `AcceptanceParams { self_id, kind, symmetric,
      bucket_count, accept_cap }`; one fallible `NodeStrategies` constructor
      taking the relay pair plus `Option<(SelectionParams,
      AcceptanceParams)>` for the publisher seam (one
      `StrategyConfigError` map site — absorbs §1.2 item 6); core-domain
      validation (bucket count 0 rejected; 1 legal ≡ ungated); added
      alongside the old builder (deleted in T016) with unit tests
      (017 FR-003 core rule; research R4, R5)

**Checkpoint**: both types + construction exist and are fully tested; the
old machinery still runs everything.

---

## Phase 3: User Story 1 — Configure selection as plane coordinates (Priority: P1) 🎯 MVP

**Goal**: the knob-only CLI and the experiments framework construct
`Selection`/`UnifiedAcceptance` everywhere; the old kinds, types, flags, and
helpers are deleted; the commit-A byte-identity gate closes the phase.

**Independent Test**: build nodes at each plane point and boundary value and
assert the selected upstream sets (spec US1 acceptance scenarios); the
experiments M2 operating point byte-diffs identical against the recorded
baselines.

- [ ] T008 [US1] Rewrite `Args` in `src/main.rs` to the knob surface per
      `contracts/node-cli.md`: per-seam
      `--relay-{bucket-count,pick-count,accept-cap}` +
      `--relay-symmetric` (renames `--symmetric-edges`) +
      `--relay-accept-unverified`, the publisher mirrors,
      `--selection-seed <u64>`, `--fanout-strategy` default
      `forward-to-all`; delete the kind/degree/shared flags (017 FR-006,
      FR-009)
- [ ] T009 [US1] Rewrite flag validation as a testable
      pure function (startup maps its `Result` to exit 2) in `src/main.rs`,
      implementing the full matrix: bucket counts ≥ 2; seed required iff any
      pick count ≥ 1 and rejected when unused; publisher seam activated
      solely by acceptance knobs rejected (error names
      `--publisher-pick-count 0`); `--*-accept-unverified` without that
      seam's bucket count rejected; the old symmetric-requires-hash-gated
      rule deleted — with value-level unit tests (no stderr assertions)
      (017 FR-007, FR-008, FR-014; spec Clarifications 2026-07-31)
- [ ] T010 [US1] Switch `src/main.rs` construction to the new params +
      `NodeStrategies` constructor (loader expands the seed u64 → 32 bytes —
      provisional expansion; the final domain lands with T025), publisher
      pair through the same call (017 FR-016 seam; research R8)
- [ ] T011 [P] [US1] Rewrite the experiments `StrategyTable` in
      `src/experiments/config.rs` to coordinates per
      `contracts/sweep-config.md`: `pick_count`, `bucket_count` (≥ 1 legal),
      `accept_cap`, `accept_unverified`, `symmetric`; kind strings
      (`connection`/`acceptance`, `uniform-sampler`) removed;
      `forward-to-all` still rejected — with config-parsing tests including
      the boundary values; NO publisher-pair fields (the 017 FR-019
      boundary: population construction stays relay-only — that surface is
      the next feature's) (017 FR-017, FR-018 domains, FR-019; research R6)
- [ ] T012 [US1] Update `src/experiments/population.rs`: build
      `Selection`/`UnifiedAcceptance` from the spec'd coordinates, thread
      `symmetric` into the relay params and `NodeStrategies.symmetric_edges`
      (today hardcoded false), keep per-participant sampler-seed threading
      into `Selection.seed` (research R6)
- [ ] T013 [P] [US1] Rewrite the shipped sweep configs under
      `configs/experiments/` to the coordinate vocabulary (same operating
      points, same seeds — manifests may differ, values must not)
- [ ] T014 [P] [US1] Rework `src/strategies/test_support.rs` fixtures to
      knob-built instances (declarative builders preserved), and migrate
      `tests/model_family.rs` + `tests/publisher_links.rs` recipes to knob
      construction — absorbing §1.2 items 12–13 (stale pre-A9/A13 comments;
      `no_links()` fixture adoption) (017 FR-023 dissolved items)
- [ ] T015 [US1] Add the US1 plane-point integration coverage to
      `tests/model_family.rs`: the four points + pick count 0 (M1 shape,
      acceptance still serving) + publisher-seam presence-activation
      on/off — the spec US1 acceptance scenarios
- [ ] T016 [US1] Deletion sweep (SC-008): remove
      `src/strategies/connection/{connect_to_all,hash_gated,none,kind}.rs`,
      `src/strategies/acceptance/{accept_from_all,bounded,hash_gated,hash_gated_bounded,none,kind}.rs`,
      `UniformSampler` from `src/experiments/strategies.rs` (re-pin T004 to
      fixture values), the old `NodeStrategies::builder` phase-1 +
      `require_target_degree`/`validate_bucket_count` in
      `src/strategies/config.rs`, and `resolve_buckets`/`bucket_count`/
      `accept_cap` in `src/strategies/edge.rs`; export
      `is_valid_edge_publisher` beside its siblings; update `src/lib.rs`
      re-exports and the `NodeView::candidates_len` doc (017 FR-005,
      FR-010, FR-012; research R10)
- [ ] T017 [US1] **Commit A gate**: full suite green in both
      configurations; re-run the recorded baseline sweeps and byte-diff
      `runs.jsonl`/`aggregates.json` **identical** (manifests differ in tool
      commit + config text only); determinism battery green (replay-by-seed,
      workers 1 vs K) — quickstart validation step 1 (017 FR-026, FR-028)
- [ ] T018 [US1] Author ADR 0039 — the unified selection plane — in
      `docs/decisions/0039-unified-selection-plane.md` (scope per research
      R11: fed knobs, type/enum collapse, acceptance merge +
      verification-follows-B, fed caps, knob-only CLI, fan-out default flip,
      the verifiable region; amendment notes against ADRs
      0018/0023/0024/0025/0028/0031/0032/0034; alternatives-rejected records
      the balanced-B registry computation — rejected as mechanism, formula
      retained as operator guidance, registry-as-carrier of a
      governance-set B open as a separate future feature — 017 FR-022)

**Checkpoint**: the plane is the only selection machinery; byte-identity
proven — US1 is a shippable MVP.

---

## Phase 4: User Story 2 — The real M4 (Priority: P2)

**Goal**: uniform picks + symmetric handshake carry the M4 label with
fleet-level evidence; the deferral text is retired.

**Independent Test**: the M4 fleet test suite passes (reciprocity, degree
floor, mean); no "approximation" disclaimer remains greppable in the named
docs.

- [ ] T019 [P] [US2] Write the failing M4 fleet tests in
      `tests/model_family.rs`: symmetric + pick count (no bucket count) ⇒
      full reciprocity (both maps, both ends), minimum degree ≥ pick count,
      mean degree within 5% of 2× pick count; plus the symmetric × gated
      composition case (unordered-pair predicate gates before the draw) —
      then make them pass (017 SC-003; spec US2 scenarios)
- [ ] T020 [US2] Upgrade the M4 label where it is disclaimed: the M4 section
      of `specs/015-publisher-links/quickstart.md` (recipe now claims the
      label via the 017 knobs), the modelling caveat consequence in
      `docs/decisions/0032-publisher-links-and-model-family.md` and the
      deferred-label consequence in
      `docs/decisions/0034-connection-vocabulary-and-constructed-symmetric-reciprocity.md`
      (dated amendment notes, not rewrites) (017 FR-021)

**Checkpoint**: M4 claimed and evidenced; independent of Phase 5.

---

## Phase 5: User Story 3 — Acceptance as two dimensions (Priority: P2)

**Goal**: the merged acceptance behaviour is integration-proven on the state
machine, including the deliberate cap-0 semantics change.

**Independent Test**: state-level handshake scenarios drive each acceptance
configuration and assert admissions, replies, and dialer-side cleanup.

- [ ] T021 [P] [US3] Write state-level integration tests (failing first)
      in `tests/publisher_links.rs` + the connection scenarios of
      `tests/model_family.rs`: cap 0 refuses with explicit `Rejected` and
      the dialer removes its pending entry (the old silent-drop contrast —
      deliberate change, 017 FR-013); publisher-seam over-capacity emits
      `Rejected` end-to-end (§1.2 item 2's missing coverage); trusting
      acceptors (`gate: None` under gated dialers) admit predicate-failing
      requests; gated acceptance drops them — then make them pass
      (017 FR-011, FR-013, FR-023)
- [ ] T022 [US3] Update the acceptance-module rustdoc
      (`src/strategies/acceptance/mod.rs`, `unified.rs`): the merged policy
      description (gate follows the seam's bucket count; caps fed
      absolutely; the four baselines as knob combinations),
      implementation-neutral wording

**Checkpoint**: acceptance semantics fully evidenced; independent of Phase 4.

---

## Phase 6: User Story 4 — Seeded selection on a real node (Priority: P3)

**Goal**: commit B — the honest derivation (new domain, self-id + epoch
nonce, length-prefixed preimage) and the final CLI seed chain; one
re-baseline.

**Independent Test**: the seed-property test battery (identity/nonce/seed
variation) passes; regenerated baselines recorded.

- [ ] T023 [P] [US4] Write the failing seed-property tests in
      `src/strategies/connection/selection.rs`: same (seed, self-id, nonce,
      view) ⇒ identical picks; differing seed ⇒ differing picks (whp);
      **two nodes sharing one seed draw independently** (self-id in the
      preimage); **one node's relay and publisher instances draw
      independently** (per-seam domains — the sampling analogue of edge.rs's
      `publisher_domain_is_an_independent_draw`); nonce change ⇒ re-draw;
      repeated heartbeats ⇒ stable; plus a pinned preimage fixture
      (length-prefixed layout) (017 FR-015; spec US4 scenarios)
- [ ] T024 [US4] Swap the derivation to commit B in
      `src/strategies/connection/selection.rs`: per-seam domains selected by
      the instance's `LinkKind` — `pubsub/uniform-selection/relay/v1` /
      `pubsub/uniform-selection/publisher/v1` — preimage `lp(domain) ‖
      lp(seed) ‖ lp(self-id key bytes) ‖ nonce_le8 ‖ lp(topic)` via
      `push_len_prefixed`; update the T004 equivalence fixtures to the new
      pinned values (the commit-A pin is superseded by design — note it in
      the test) (research R2)
- [ ] T025 [US4] Finalise the CLI seed expansion in `src/main.rs`:
      `SHA-256(lp("pubsub/selection-seed/v1") ‖ seed_le8)` → constructor
      bytes; seed-required validation already live from T009 (research R8)
- [ ] T026 [P] [US4] Author ADR 0040 — selection randomness derivation — in
      `docs/decisions/0040-selection-randomness-derivation.md` (seed chain,
      preimage layout, the per-seam draw domains and the correlation defect
      they close, the privacy stand-in posture and its trigger, the
      two-commit derivation swap) (research R11)
- [ ] T027 [US4] Record the two new implementation notes in
      `specs/IMPLEMENTATION_NOTES.md` at the next free numbers: gate-failing
      dials as provable-but-unrecorded evidence (trigger: incentive/chain
      layer) and selection-seed privacy (trigger: first adaptive-adversary
      experiment or real-crypto identity work); re-point N-032's trigger
      (first experiment needing symmetric × capped — may never arrive)
      (017 FR-022)
- [ ] T028 [US4] **Commit B gate**: full suite + determinism battery green;
      regenerate and record fresh baseline generations per
      `notes/experiments-baselines/README.md` (new tool commit, new values —
      deliberate) (017 FR-026)

**Checkpoint**: final derivation in place; baselines current.

---

## Phase 7: User Story 5 — Plane sweeps and re-validated baselines (Priority: P3)

**Goal**: the plane is sweepable (axes + boundary points), the m2-comparison
is re-validated at the new implementation, and the first M4 baseline exists.

**Independent Test**: an axis sweep crossing bucket count (incl. 1) and pick
count (incl. 0) runs with boundary cells reproducing the off/ungated
behaviours; the m2-comparison doc carries the re-validated values.

- [ ] T029 [P] [US5] Rename the `target_degree` axis parameter to
      `pick_count` and add the `bucket_count` axis in
      `src/experiments/config.rs` (+ `src/experiments/sweep.rs`
      follow-through), with tests covering boundary axis values
      (`bucket_count = 1` cell ≡ ungated; `pick_count = 0` cell ≡ seam off)
      (017 FR-018; spec US5 scenario 1)
- [ ] T030 [US5] Ship the M4-completing sweep configuration under
      `configs/experiments/` (`symmetric = true`, `pick_count` set, no
      `bucket_count`; existing M2 extraction per research R7) and record its
      baseline per the baselines README — the first M4 baseline
      (017 FR-027)
- [ ] T031 [US5] Re-execute the m2-comparison operating points at the final
      implementation; confirm statistical agreement per the document's
      recorded methodology (raw counts + Wilson 95%; exact-agreement checks
      where defined); update `docs/experiments/m2-comparison.md` with the
      re-validated values and the 017 tool commit (017 FR-025, FR-027)

**Checkpoint**: E7/E10 runnable from shipped configs; validation contract
fully discharged.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T032 [P] Uniform `Active` check in
      `src/strategies/fanout/forward_to_relays.rs` (relay arm currently
      admits any `LinkState`) with a unit test (§1.2 item 1; 017 FR-023)
- [ ] T033 [P] Fan-out docs: fix the `src/strategies/fanout/mod.rs`
      intra-doc link + pre-015 text (§1.2 item 9); flip the default
      documentation in `src/strategies/fanout/kind.rs`; add the M5-footgun
      line to the CLI help in `src/main.rs` (017 FR-009)
- [ ] T034 [P] Correct the E12 status line in `docs/experiments-program.md`
      (ready; the flooding point = bucket count pinned, no pick count, with
      silent-relay fan-out — the FR-022 disposition's one edit)
      (017 FR-022, FR-024)
- [ ] T035 Refresh the `CLAUDE.md` active-work stanza to the completed-branch
      form (017 delivered shape, ADR/N-note pointers, baselines note)
      (017 FR-024)
- [x] T036 Declined (maintainer decision, 2026-07-31): the optional
      configuration-placement ADR (017 FR-024) is not authored — the node
      has no config file to regulate (ADR 0033 deleted its last field), so
      there is no flags-vs-TOML convention left to record; the remaining
      TOML surfaces (registry data files, sweep configs) are separate
      programs' inputs with their own contracts
- [ ] T037 Final sweep: `cargo fmt`, `clippy --all-targets -- -D warnings`,
      full suite in both configurations; SC-008 symbol sweep (deleted
      types/functions/flags absent from `src/` and `--help`); SC-001 recipe
      verification (every quickstart command parses and boots against the
      final flag surface)

---

## Dependencies & Execution Order

- **Phase 1 → Phase 2 → Phase 3** strictly sequential (foundational types
  are additive; Phase 3 performs the switchover + deletion and closes with
  the commit-A gate T017).
- **Phases 4 and 5 (US2, US3)** depend only on Phase 3 and are independent
  of each other — parallelizable.
- **Phase 6 (US4)** depends on Phase 3 (and lands after 4/5 in practice so
  the commit-B re-baseline happens once, late).
- **Phase 7 (US5)** depends on Phase 6 (baselines and the m2 re-run must
  measure the final derivation).
- **Phase 8** anytime after Phase 3 for T032–T034; T035–T037 last.
- Commit milestones: **A = T017**, **B = T028**; all other commits are
  ordinary green checkpoints.

## Parallel Examples

- Phase 2: T002 ∥ T003 (different files); T004 after T002 exists (same
  file), before T005.
- Phase 3: T011 ∥ T013 ∥ T014 while T008–T010 proceed (different files);
  T016 strictly after T008–T015; T017 after T016.
- Phases 4 ∥ 5 wholesale; T032–T034 ∥ anything after Phase 3.

## Implementation Strategy

**MVP = Phase 1–3 (US1)**: the plane replaces all selection machinery with
byte-identity proven — independently shippable and the riskiest surface
retired first. Then US2/US3 add the evidence suites (parallel), US4 lands
the one deliberate derivation change late so re-baselining happens once,
US5 discharges the measurement contract, and Polish closes the obligations
ledger. Every task keeps the repo at a green checkpoint; the two named
gates (T017, T028) are the byte-identity and re-baseline milestones the
spec's validation contract pins.

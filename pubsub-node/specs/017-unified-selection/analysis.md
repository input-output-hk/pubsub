# Analysis ledger — 017-unified-selection

`/speckit-analyze` findings and resolutions (Development Workflow: the
ledger is this file, not commit messages). Artifacts analyzed: spec.md
(post-clarify, post-checklist), plan.md + plan-input.md, research.md,
data-model.md, contracts/{node-cli,sweep-config}.md, quickstart.md,
tasks.md, against constitution v1.2.0.

## Pass 1 — 2026-07-31 (pre-implementation)

3 findings (0 CRITICAL, 0 HIGH, 2 MEDIUM, 1 LOW). Coverage 100% at FR
granularity (28 FRs, 8 SCs, 37 tasks); no duplications, no ambiguities, no
terminology drift, no constitution issues; unmapped tasks limited to
setup/optional/polish infrastructure (T001, T036, T037).

### Inconsistency

- **I1 (MEDIUM)** — tasks.md T010/T025 vs spec FR-026: FR-026's letter said
  commit B "lands the CLI seed derivation", while T010 introduces a
  provisional loader expansion in the commit-A window (required: T009's
  seed-required validation means sampling nodes must boot before commit B).
  **Resolution**: FR-026 amended to "lands the **final** CLI seed
  derivation" with the parenthetical noting a provisional expansion may
  precede it — the node CLI has no recorded baselines; only the
  experiments-facing derivation is byte-identity-constrained. Normative
  content (byte-identity gate, honest-preimage timing) unchanged.

### Coverage

- **G1 (MEDIUM)** — spec FR-022's balanced-B-rejection disposition had no
  explicit owning task (T027 owns the N-notes + N-032 trigger, T034 owns
  E12; the balanced-B record had no named home). **Resolution**: T018 (ADR
  0039) extended — the alternatives-rejected section records the balanced-B
  registry computation: rejected as mechanism, formula retained as operator
  guidance, registry-as-carrier of a governance-set B open as a separate
  future feature.
- **G2 (LOW)** — spec FR-019 (the sweep config MUST NOT gain publisher-pair
  fields) was covered only implicitly (T011's `deny_unknown_fields` +
  absence). **Resolution**: T011 extended to name the FR-019 boundary
  explicitly, so the implement phase treats the absence as a requirement,
  not an omission.

## Pass 2 — 2026-07-31 (post-remediation re-run)

Re-evaluated the pass-1 edits first (FR-026 parenthetical consistent with
research R8 and the data-model seed chain; T018's addition consistent with
FR-022's wording; T011's note consistent with FR-019), then the full
detection set (duplication, ambiguity, underspecification, constitution
alignment, coverage, inconsistency). **Zero findings** — convergence
recorded. Metrics unchanged: 28 FRs + 8 SCs, 37 tasks, coverage 100%,
critical 0.

Gate clear for `/speckit-implement`. A post-implementation analyze round
remains required (Development Workflow: spec fidelity is verified against
code when code exists); its passes will be ledgered below.

## Implementation-round findings

### 2026-08-01 — Phase 2 checkpoint observation (correlated seam draws)

- **I2 (HIGH, latent)** — the commit-B draw preimage as designed (research
  R2 / data-model) carried no seam component, and the CLI expansion derives
  one seed for both seams: an M3/M5 node's relay and publisher `Selection`
  instances (same seed, self-id, nonce, topics) would derive identical
  per-topic RNG streams — with both seams ungated and equal pick counts,
  publisher targets identical to relay upstreams. Contradicts the models'
  independent-draws assumption and the independence the edge predicate's
  per-seam hash domains already provide for gated selection. Undetectable
  by the commit-A gate (experiments are relay-only) — would have shipped
  latent until the publisher-pair experiments feature. **Resolution**:
  per-seam draw domains selected by the instance's `LinkKind`
  (`pubsub/uniform-selection/relay/v1` / `…/publisher/v1`), the edge.rs
  pattern; one shared `--selection-seed` stays (the `--genesis` analogy —
  one value, decorrelated per seam by domains); no symmetric draw domain
  (the switch changes the handshake, not the draw). Amended: spec FR-015
  (purity tuple + per-seam independence property), research R2 (derivation
  + rejected alternatives), data-model (derivation line + invariant),
  tasks T023 (independence test) / T024 (both domains) / T026 (ADR 0040
  scope), plan summary. Commit A unaffected by construction.

### 2026-08-01 — Phase 4 checkpoint observation (sampled selection under view growth)

- **I3 (MEDIUM, deferred-with-note)** — the T019 M4 fleet test's first
  (failing) run measured a real interaction: a sampled pick set is a
  function of the whole candidate view, so dials over partial views draw
  subset picks (below the pick count if the view is smaller, no retry) and
  re-dials after view growth draw *different* samples whose add-only union
  inflates degree past 2× the pick count until rotation. ADR 0031's
  heartbeat re-dial idempotence is thereby conditional for the sampling arm
  (stable view), while hash-gating remains unconditionally
  monotone-consistent. Not reachable today: the v1 node fires one readiness
  heartbeat over a fully-folded snapshot, and the driver's faithful mode
  has the all-synced barrier (the fast path pre-populates). **Resolution**:
  third implementation note added to T027's scope (cross-referencing
  N-011; trigger: periodic heartbeats / epoch rotation, or the first
  staggered-boot fleet or experiment) and a caution added to the 017
  quickstart; the test harness seeds full membership before construction,
  matching the driver's documented barrier.

### 2026-08-01 — Phase 7 checkpoint rulings

- **I4 (LOW)** — SC-005's letter ("E7 and E10 are runnable from shipped
  sweep configurations") overstated the E10 arm: the plane axes are shipped
  and boundary-verified down to run values, but no config crosses the plane,
  and shipping one would pre-empt the E10 grid design that the experiment
  program (stage 4) owns — every value in a feature-shipped placeholder
  would be an undesigned choice. The M4 config differs in kind: the
  feature's validation contract required its baseline. **Resolution**
  (maintainer ruling): SC-005 amended to capability-satisfied wording (E7
  runnable from the shipped configuration; E10 axes shipped and verified;
  the grid design stays program work); T037's SC sweep checks the amended
  criterion.
- Also ruled at this checkpoint: `docs/experiments-program.md`'s
  strategy-inventory vocabulary refresh rides T034 (same file as its E12
  status edit) — task text amended.

## Pass 3 — 2026-08-01 (post-implementation: spec fidelity verified against code)

Independent verification round (implementation-session reports treated as
unverified input; every claim re-derived from the code and committed
artifacts on the branch). Verified green: both suites (`cargo test`,
`cargo test --features experiments`), `fmt --check`, `clippy --all-targets
-- -D warnings`. Re-derived against code: the `Selection` draw semantics and
commit-B preimage (per-seam domains, `lp` layout, nonce_le8 — pinned layout
test reconstructs it end to end), the `UnifiedAcceptance` decision order and
2×2 matrix, the construction layer (core bucket-0-rejected/1-legal split,
seed expansion `SHA-256(lp("pubsub/selection-seed/v1") ‖ seed_le8)`), the
full CLI knob surface + validation matrix (every SC-007 misconfiguration
exercised live against the binary — all fail with actionable messages; the
family-1 recipes and gated/capped variants all parse and boot to file
loading), the SC-008 deletion sweep (symbol grep over `src/` + `--help`
clean; residual mentions are negative tests and historical doc comments),
the experiments coordinate surface (StrategyTable fields, `pick_count`/
`bucket_count` axes with boundary points, old vocabulary rejected by test,
`forward-to-all` still rejected, no publisher-pair fields per FR-019), the
M4 fleet evidence (reciprocity both-ends/both-collections, min degree ≥
pick count, mean within 5% of 2×, symmetric × gated composition), the US3
state-level scenarios (cap 0 → explicit `Rejected` + dialer cleanup;
publisher over-capacity end-to-end; trusting vs verifying acceptors),
ADRs 0039/0040 (scope per T018/T026 incl. the G1 balanced-B record; 0040's
u32-BE `lp` claim checked against `push_len_prefixed`), N-036/N-037/N-038 +
the N-032 re-point, the four M4-label sites (all carry dated 2026-08-01
amendments — including the 015 contracts table, which FR-021 names though
T020's list omitted it), E7/E10/E12 read ready in the program doc with the
coordinate vocabulary refresh, and the plan's core-untouched claim (zero
diffs outside `src/state/tests/` under the core paths; no files outside
`pubsub-node/` — Principle V clean). Verification boundary: the
byte-identity and baseline claims (SC-002, FR-026/027) were verified as
recorded-evidence consistency only — baseline sweeps are off-limits this
round; the chain (baselines README generations `7e50e3a`/`d7e7132`,
m2-comparison values, commit graph) is internally consistent (71/8000 ↔
7929/8000; the commit-A parent hash `1c860e3` matches the log).

**2 findings (0 CRITICAL, 0 HIGH, 0 MEDIUM, 2 LOW). Resolutions pending
maintainer ruling.**

### Inconsistency

- **I5 (LOW)** — SC-003's letter ("no remaining 'approximation' disclaimer
  in quickstart, contracts, or ADR caveats") and tasks.md Phase 4's
  independent test ("no 'approximation' disclaimer remains greppable in the
  named docs") conflict with T020's own convention ("dated amendment notes,
  not rewrites"), which the implementation followed: all four sites (015
  quickstart ×2, ADR 0032 caveat, ADR 0034 consequence, plus the 015
  contracts recipe row) retain the original disclaimer text, each superseded
  in place by a dated amendment claiming the label. Intent satisfied — every
  disclaimed site now claims the label and the canonical 017 docs carry no
  disclaimer — but the two greppability-style sentences fail their letter.
  Pre-existing wording (passes 1–2 did not flag it); surfaced now because
  this round greps. **Resolution** (maintainer ruling, 2026-08-01: reword):
  SC-003's parenthetical amended to "no un-amended 'approximation'
  disclaimer remains … every disclaiming site carries a dated amendment
  claiming the label, per the amendment-not-rewrite convention"; tasks.md
  Phase 4's independent test amended to match. The delivered docs already
  satisfy the reworded criterion; no doc edits.
- **I6 (LOW)** — implementation-note count drift: spec FR-022 enumerates
  "Two implementation notes" and plan.md Scale/Scope says "2 implementation
  notes (N-036, N-037)", while three landed — N-038 was added by the I3
  ruling, which amended T027 (now "the three new implementation notes") and
  the 017 quickstart but not FR-022 or the plan summary. FR-022's MUST is a
  floor and is satisfied; the artifact set drifted around the I3 resolution.
  **Resolution** (maintainer sign-off, 2026-08-01): FR-022 amended to "Three
  implementation notes" with (c) sampled selection under view growth (the
  I3 addition, its trigger carried); plan Scale/Scope updated to "3
  implementation notes (N-036, N-037, N-038)"; research R11's count amended
  identically (the same drift's third site, found by the pass-4 sweep and
  fixed under this ruling). Normative content unchanged.

## Pass 4 — 2026-08-01 (post-remediation re-run)

Re-evaluated the pass-3 resolutions first: SC-003's reworded parenthetical
and tasks.md Phase 4's independent test now state the amendment-not-rewrite
criterion the delivered docs already satisfy (all four disclaiming sites
verified carrying dated amendments in pass 3 — no doc edits were needed);
FR-022's "(c)" matches N-038's recorded scope and trigger verbatim; the
note-count sweep is clean across spec/plan/research/tasks/contracts/
quickstart and both ADRs (the one residual — research R11 — was fixed under
the I6 ruling; the ledger's own historical quotes are the only remaining
occurrences of the old wording, correctly). Then the full detection set
(duplication, ambiguity, underspecification, constitution alignment,
coverage, inconsistency) over the amended artifacts. **Zero findings** —
convergence recorded. Metrics: 28 FRs + 8 SCs, 37 tasks, coverage 100%,
critical 0.

The post-implementation analyze round is closed: spec fidelity verified
against code (pass 3's evidence), both resolutions applied with maintainer
sign-off, zero-finding pass recorded. The branch awaits maintainer review;
nothing pushed.

## Addendum — 2026-08-02 (T034 authorization chain)

T034 scope note: E12's ready flip is the FR-022/FR-024 mandated edit (one
edit, not two — FR-024's clause merges the FR-022 disposition and the E12
status line into a single edit); the E7/E10 flips ride T034's Phase-7 rider
— the retired `[needs: uniform exactly-RF selection kind]` tags are pre-017
kind vocabulary naming machinery 017 delivered as the pick count — with
"ready" in the program document's no-machinery-dependency sense (the
E9/E11/E13 sense) and I4's grid-design ownership boundary restated in the
E10 entry.

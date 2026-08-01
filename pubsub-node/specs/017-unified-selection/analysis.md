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

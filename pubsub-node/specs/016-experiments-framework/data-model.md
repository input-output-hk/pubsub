# Data Model — 016-experiments-framework (Phase 1)

Field names below are normative in *content*, illustrative in *spelling*
(final identifiers fixed in code review); every structure that reaches an
output artifact contains only order-stable containers (`Vec`, `BTreeMap`)
per research R4/R9.

## 1. Population & participants

- **ParticipantClass** — `Honest | Adversarial`. Assigned at population
  build by the seeded class draw. Adversarial = Level-1 only in v1 (honest
  `apply`, hostile strategy bundle). Storage is an enum shaped to admit a
  future protocol-violating variant without rework (spec FR-011).
- **Participant** — `{ peer_id, class, down: bool, node_state: NodeState,
  strategies: {connection, acceptance, fanout} }`. `down` is set only by the
  churn draw, only on honest participants, only between the dial drain and
  the publish drain. **Up-honest** ≙ `class == Honest && !down`.
- **Population** — `BTreeMap<PeerId, Participant>` plus the build inputs
  (topic, per-class strategy configuration). Invariants enforced at build:
  single topic; every member subscribed to it; topic registered open;
  |up-honest| ≥ 2 after the churn draw (publisher + at least one eligible
  receiver — spec FR-031 validation).
- **Setup modes** — `Faithful` (membership/topic-registry/Synced events fed
  through `apply`; all registry folds before any Synced; all Synced as one
  wave) vs `Prepopulated` (state fields written directly; readiness set).
  Both produce the same observable population (asserted in tests on small
  configurations).

## 2. Run phase machine

```
Build population ──> Registration/sync ──> Dial drain ──> Churn draw
      (seeded)         (faithful | fast)     (Heartbeat,      (seeded mark,
                                              quiescence)      no events)
 ──> SCC pass (pre-churn, iff churn > 0) ──> SCC pass (post-churn)
 ──> Publish drain × publishes_per_run ──> Measure ──> RunRecord
      (seeded up-honest publisher,
       fresh message per publish)
```

Transitions are strictly ordered; no phase overlaps another. The epoch nonce
stays at genesis for the whole run (single-epoch, spec FR-009).

## 3. Driver structures

- **Wave** — `Vec<Delivery>` where `Delivery = { from: PeerId, to: PeerId,
  message }`. Before routing, each wave is stably sorted by the canonical
  content key `(from, to, message identity)` — research R2; this sort is
  permanent and load-bearing.
- **DrainOutcome** — per-phase observation: waves processed, per-node
  first-receipt wave (`BTreeMap<PeerId, u32>`), sends tally split by
  recipient class, suppressed count, severance tally. The publish drain's
  outcome feeds metrics; the dial drain contributes tallies only.
- **Seed derivation** — `run_seed = truncate(SHA-256(master_seed ||
  run_index))`; sub-seeds by domain label: `keys`, `classes`, `churn`,
  `publisher`, `sampler` (research R6). Recorded in the manifest.

## 4. Graph analytics

- **PropagationDigraph** — extracted from node states by the
  **DisseminationModel** dispatch; vertices = up-honest participants, edge
  `u → v` iff v is in u's fan-out target set for the model. v1: M2 variant
  only (edges = `downstream` records between up-honest peers; seeds =
  {publisher}). Adjacency in sorted form (deterministic iteration).
- **DisseminationModel** — enum, v1 `{ M2 }`; owns extraction, per-publisher
  seed-set derivation, and the goodness rule (spec FR-022). Named in config
  and manifest.
- **Condensation** — output of the iterative Kosaraju pass: component id per
  vertex, component sizes, component DAG edges, source/sink component sets.
- **GoodnessVerdict** — `{ good: bool, min_publisher_coverage: f64,
  sccs: usize, largest_scc: usize }`; computed post-churn (primary) and
  pre-churn (diagnostic, iff churn > 0). `good` ⟺ one SCC;
  `min_publisher_coverage = (smallest sink component − 1)/(up-honest − 1)`
  (excluded-publisher convention). The full-delivery publisher fraction is
  opt-in and absent when not computed (spec FR-021).
- **TopologyShape** — in/out-degree histograms over the extracted digraph +
  honest-sink count (out-degree 0).

## 5. Metrics & records

- **MissCause** — `AllUpstreamsAdversarialOrDown | NoUpstream |
  NoUpHonestPath` (spec FR-017); classified from driver-owned state.
- **RunRecord** (one JSONL row; content per spec FR-028; toy shapes agreed
  in discussion):
  - identity: `run`, `experiment`, `seed`;
  - population as drawn: `honest`, `adversarial`, `down`, `up_honest`,
    `publisher`;
  - dial phase: `dial_waves`, `dial_sends`, `rejected_over_capacity`;
  - graph analytics: `good`, `min_publisher_coverage`, `sinks`, `sccs`,
    `largest_scc`, `in_degree_hist`, `out_degree_hist`; the `_pre_churn`
    counterparts (`good_pre_churn`, `min_publisher_coverage_pre_churn`,
    `sinks_pre_churn`) are **present iff churn > 0** and absent otherwise
    (spec edge case: churn = 0 runs one pass and records once; absent ≠
    zero per the output contract);
  - publish drain (per publish; default one): `coverage`, `received`,
    `missed`, `max_depth`, `depth_hist` (index = wave; wave 0 = publisher),
    `miss_causes` (counts per `MissCause`), `sends` `{honest, adversarial,
    down}`, `suppressed`, `severed`.
  - **Invariant asserted before emission**: `sends_total = first_receipts +
    suppressed + sends.down` (spec FR-018).
  - **Size invariant**: scalars + degree/depth-bounded vectors only —
    nothing O(N) (spec FR-028; SC-005).
- **PerNodeDetail** (opt-in, off by default): per node — first-receipt wave,
  first-delivery origin, in/out degree, miss cause (if missed), class/down.
  Regenerable exactly from `seed` (spec FR-030).

## 6. Statistics & aggregates

- **CountEstimate** — `{ count, runs, p, wilson95: [lo, hi] }` (research
  R7; used for `good`, `full_coverage`, and — for experiments with
  churn > 0 — `good_pre_churn`, mirroring the per-run presence rule).
  Structural invariant: `good ⇒ full coverage` per run under v1 relays —
  drain coverage ≡ graph reachability (spec SC-003) and good means every
  publisher reaches everyone (spec FR-020) — so
  `full_coverage.count ≥ good.count`, asserted in the aggregates fold.
- **Histograms** — sparse integer maps (`BTreeMap<u64, u64>`) for
  integer-valued metrics (missed count, max depth, sink count); fixed-width
  bins for coverage fractions (bin width a statistics-module constant, not
  config); pooled depth histogram = element-wise sum of in-run `depth_hist`.
- **ExperimentAggregates** — per experiment: the three `CountEstimate`s,
  the histograms above, message-cost means/percentiles (incl. per-class
  send means, duplication ratio), min-publisher-coverage histogram.
  **Derivability invariant**: a pure fold of the run records in run-index
  order (spec FR-029); float folds in canonical order (research R3).
- **SweepManifest** — tool commit, master seed + derivation rule
  description, fixed parameters, axes, expanded experiment list (index →
  parameter set). Result-affecting inputs only; invocation surface (output
  dir, worker count, detail flags) deliberately excluded (contracts/
  sweep-config.md).

## 7. Configuration

- **SweepDescription** (parsed from TOML at the edge; spec FR-031) —
  dissemination model (v1: `m2` only accepted), population size, class
  counts, churn (count | proportion), topic, strategy parameters (per-class
  triads incl. `target_degree`), `runs_per_experiment`, `publishes_per_run`
  (default 1), axes (parameter name → value list), master seed.
- **Validation** (rejection at parse/build): unknown model; zero eligible
  receivers; churn exceeding the honest population or leaving no up-honest
  publisher; multi-topic requests.
- **Invocation flags** (clap, outside the manifest): config path, output
  directory, `--workers`, `--per-node-detail`.

## 8. Scripted topologies (test support)

Declarative builders (Engineering Standard: declarative test construction)
producing prepopulated populations with hand-computable metrics: `line(n)`,
`star(n)`, `full_mesh(n)`, each with optional per-node class overrides (e.g.
`silent(peer)`) — used by the known-topology, silent-adversary, and
cross-check validation tests (spec FR-032; SC-002/SC-003).

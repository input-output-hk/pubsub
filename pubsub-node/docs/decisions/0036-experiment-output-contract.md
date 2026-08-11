# ADR 0036: Experiment output contract & statistics conventions

**Status**: Accepted. **Amended by ADR 0041**: the run records' send
accounting gains a per-link-kind split (relay/publisher, attributed at
emission with relay winning the both-kinds case) and a second per-run
identity — relay + publisher = total sends — beside the existing one.
**Date**: 2026-07-19
**Feature**: 016-experiments-framework
**Source**: `specs/016-experiments-framework/contracts/output-artifacts.md`; `research.md` R3/R4/R6/R7/R9

## Context

The experiments framework's externally consumed surface is its output
directory: analysis scripts and the documented M2 comparison read the files,
not the Rust API. Two properties carry the instrument's credibility — the
artifacts must be byte-reproducible from (sweep description, master seed,
tool commit) at any worker count and across process restarts (016-SC-001),
and every reported probability must stay meaningful at the all-good samples
that dominate healthy configurations.

## Decision

**Three artifacts per sweep**, written only by the sweep layer (runs are
pure functions performing no I/O):

- `manifest.json` — tool commit (the schema/version pin), master seed, the
  seed-derivation rule verbatim, runs-per-experiment, and the expanded
  experiment list; run records reference experiments by index. Only
  result-affecting inputs appear: invocation surface (output directory,
  worker count, detail flags) never reaches the manifest.
- `runs.jsonl` — one JSON object per run, streamed in canonical run-index
  order. Records carry scalars and degree/depth-bounded vectors only —
  nothing sized by the population (016-SC-005); opt-in per-node detail is a
  separate file, never these rows. Pre-churn fields (`good_pre_churn`, …)
  are present iff the run drew churn — **absent ≠ zero**.
- `aggregates.json` — one entry per experiment, in experiment order, each a
  **pure fold of its run records in run-index order** (float summation is
  not reorder-stable, so the fold order is load-bearing). External tooling
  can recompute the file from `runs.jsonl` and diff it.

**Seed derivation** (016-FR-024, recorded in the manifest):
`run_seed = SHA-256('experiments/run-seed/v1' ‖ master_seed ‖ run_index)`;
per-run sub-seeds derive from the run seed under domain labels
(`keys`/`classes`/`sampler`/`churn`/`publisher`). Pre-derived seeds are
independent of execution order — the prerequisite for run-granularity
parallelism — and the recorded hex seed alone replays any run.

**Statistics conventions**:

- Probability estimates are always `{count, runs, p, wilson95}` — raw
  counts plus the closed-form Wilson score interval at a fixed 95% level.
  Wilson has well-defined nonzero width at p̂ ∈ {0, 1}; the formal folder's
  ±1σ standard errors degenerate to zero width exactly at the all-good
  sample, and any other convention is derivable from the counts. No bare
  probabilities, no configurable level.
- Coverage uses the excluded-publisher denominator: eligible receivers are
  the up-honest subscribed nodes minus the publisher.
- Integer-valued metrics aggregate as sparse histograms (`BTreeMap`);
  fraction-valued metrics (coverage, min publisher-coverage) bin at a fixed
  width owned by the statistics module, not configuration.
- Structural invariant asserted in the fold: `full_coverage.count ≥
  good.count` — under v1's all-or-nothing relays a good topology delivers
  everything, so a good run without full coverage means the two instruments
  disagree and the sweep must fail loudly rather than emit.

**Encoding**: `serde_json` (optional dependency, ADR 0037) over types whose
containers are order-stable (`Vec`, `BTreeMap` — never a `HashMap` field);
byte-identity = value-determinism ∘ deterministic encoding. Determinism
testing is layered per research R9: value-level equality is the workhorse,
one golden serialization test pins the record's field inventory and
encoding, and a single file-level byte-diff integration test anchors the
artifact-level claim.

**Interruption**: no resume in v1. An interrupted sweep leaves `runs.jsonl`
as a valid canonical-order prefix with no completion claim; re-running
reproduces everything from the same seed.

## Consequences

- Analysis tooling may rely on schema stability within a tool commit; the
  manifest's commit field is the cross-commit pin. Schema changes must
  consciously update the golden serialization test.
- Every record row is self-sufficient for replay (`seed` + the manifest's
  experiment parameters), which is what makes the per-node-detail dissection
  workflow (016-FR-030) an opt-in re-run rather than a default cost.
- Holding the statistics conventions here (not per-config knobs) keeps every
  sweep's outputs comparable and keeps the M2-comparison methodology note a
  documentation concern, not a configuration matrix.

## Alternatives considered

- **±1σ standard errors as the reported field** (the formal folder's
  convention): zero width at p̂ ∈ {0, 1} — the common all-good case reads as
  false certainty; carried instead as a methodology note in the documented
  comparison.
- **Clopper–Pearson intervals**: conservative and needs Beta quantiles; the
  Wilson closed form is two lines and standard.
- **Resume support for interrupted sweeps**: bookkeeping (validity markers,
  partial-fold state) for a tool whose runs are cheap to reproduce by
  construction; re-run is simpler and provably equivalent.
- **A binary/columnar format** (CSV, Parquet): JSONL keeps records
  self-describing and diffable in review; the volumes (10³–10⁴ bounded rows)
  do not justify a format dependency.

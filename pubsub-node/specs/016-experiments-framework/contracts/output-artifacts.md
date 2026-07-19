# Contract — Sweep output artifacts

The externally consumed surface of the experiments framework is its output
directory: exactly three data artifacts per sweep (spec FR-028…FR-030).
Downstream consumers are analysis scripts and the documented M2 comparison;
this contract is what they may rely on.

## Files

| File | Format | One per | Ordering |
|---|---|---|---|
| `manifest.json` | JSON object | sweep | n/a |
| `runs.jsonl` | JSON Lines — one object per run | run | canonical run-index order, streamed |
| `aggregates.json` | JSON object | sweep (entries per experiment) | experiment-index order |

Optional, per run, only when `--per-node-detail` is on: a per-node table
(placement/naming fixed at implementation; content per data-model §5
PerNodeDetail). Never produced by default.

## Guarantees

1. **Byte reproducibility**: same sweep description + master seed + tool
   commit ⇒ byte-identical artifacts, at any worker count, across process
   restarts (SC-001). Invocation flags (output dir, workers, detail) never
   affect artifact bytes (detail adds files; it does not change these
   three).
2. **Derivability**: `aggregates.json` is a pure function of `runs.jsonl`
   — external tooling can recompute and diff it (FR-029).
3. **Bounded rows**: a run record contains scalars and degree/depth-bounded
   vectors only; size independent of population (FR-028, SC-005).
4. **Self-description**: `manifest.json` carries tool commit, master seed,
   the seed-derivation rule, fixed parameters, axes, and the expanded
   experiment list; run records reference experiments by index into it.
5. **No plotting, no prose**: artifacts are data; interpretation lives in
   analysis tooling and write-ups.
6. **Interruption**: an interrupted sweep leaves `runs.jsonl` as a valid
   prefix in canonical order with **no completion claim**; only a sweep that
   ran to completion has all three artifacts consistent (Clarifications
   2026-07-17 — no resume in v1).
7. **Probability fields**: always raw counts + Wilson 95% interval
   (`{count, runs, p, wilson95}`); no bare probabilities (FR-023).
8. **Absent ≠ zero**: opt-in metrics not computed (full-delivery publisher
   fraction) are absent fields, never zero/null placeholders.
9. **Schema stability**: within one tool commit the schema is fixed;
   cross-commit schema changes are permitted (the manifest's commit field is
   the version pin — reproducibility is a property of code + seed).

## Field inventories

Normative content lists: data-model.md §5 (RunRecord), §6
(ExperimentAggregates, SweepManifest). Exact field spellings are fixed in
implementation and pinned by the focused serialization test (research R9).

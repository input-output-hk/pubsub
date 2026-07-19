# Quickstart — 016-experiments-framework

## Build & test

```sh
cargo build --features experiments            # library + experiments module
cargo build --features experiments --bins    # + the `experiments` binary
cargo test                                    # default build — must be unaffected
cargo test --features experiments             # + framework suite (incl. smoke)
```

## Run a single experiment

Write a sweep description (see `contracts/sweep-config.md` for the shape;
omit `[axes]` for a single experiment), then:

```sh
experiments --config my-experiment.toml --out results/my-experiment/
```

Outputs in `results/my-experiment/`: `manifest.json` (what ran),
`runs.jsonl` (one record per run), `aggregates.json` (distributions,
percentiles, P(good) as counts + Wilson 95%). Re-running with the same
config and master seed reproduces the files byte-for-byte.

## Sweep a parameter

Add axes to the same file:

```toml
[axes]
churn = [0.0, 0.05, 0.10]
```

Each grid point becomes one experiment in the manifest; `aggregates.json`
gains one entry per experiment — one point of the metric-vs-parameter curve
each. `--workers N` parallelises without changing any output byte.

## Replay and dissect a run

Take `run` and `seed` from the interesting row of `runs.jsonl`, narrow the
config to that experiment's parameters, and re-run with
`--per-node-detail` — the run record reproduces exactly, plus the per-node
table (first-receipt wave, first-delivery origin, degrees, miss cause) for
tracing which cluster missed and why.

## The M2-comparison demonstration (manual procedure)

1. `experiments --config configs/experiments/m2-operating-point.toml --out results/m2-op/`
   (N = 20 000, μ = 0.2, RF = 24; expect ≲ 1 h — pick `--workers` for your
   memory budget; ~40+ runs).
2. `experiments --config configs/experiments/m2-bulk-regime.toml --out results/m2-bulk/`
   (the named bulk-regime point; R ~ 10⁴ cheap small-N runs).
3. Fill the comparison table against the formal simulators' published
   values (`../formal_spec/hybrid_dissemination/models/comparison.md` and
   m2's `full_coverage.md`):
   - from `results/m2-op/aggregates.json`: honest→honest sends
     (`sends.honest` mean), copies per honest node, depth distribution;
   - from `results/m2-bulk/aggregates.json`: P(good) counts + Wilson 95%
     vs the coverage law's prediction at those parameters.
4. Record agreement or explained deviation — the demonstration informs, it
   does not gate. Include the **uncertainty-methodology note** (required by
   the spec): the formal folder reports ±1σ standard errors, which
   degenerate to zero width at all-good samples; this framework reports raw
   counts + Wilson 95%; the conventions map via the counts. Flag the topic
   to the formal-methods team.

The suite-sized `m2-smoke.toml` variant of the same shape runs inside
`cargo test --features experiments` and asserts pipeline health only —
config parses, sweep executes, artifacts well-formed, identities and
determinism hold — never numeric agreement.

## Interruption

There is no resume: an interrupted sweep leaves a readable canonical-order
prefix of `runs.jsonl` with no completion claim — re-run it (same seed ⇒
same results).

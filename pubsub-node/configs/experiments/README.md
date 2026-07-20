# The experiments framework

A deterministic, in-process measurement instrument for dissemination
experiments. It drives populations of the crate's real node cores — the
same state-transition function, strategy seams, and message vocabulary the
node runs — under a synchronous round-based scheduler, with no networking
or async runtime in the measurement path. Everything is feature-gated:
with the `experiments` cargo feature off, the crate's build, public API,
and tests are unaffected.

The central guarantee is **byte reproducibility**: the same sweep
description, master seed, and tool commit produce byte-identical output
files, at any worker count and across process restarts. Every run is
replayable in isolation from the seed recorded in its output row.

## Build

```sh
cargo build --features experiments --bins             # development
cargo build --release --features experiments --bins   # real experiment runs
cargo test  --features experiments                     # framework test suite
```

Use the release binary for real experiments — large populations are
10× slower unoptimized.

## Describe an experiment

An experiment is described by a TOML file (the three `m2-*.toml` files in
this directory are complete working examples):

```toml
model = "m2"            # dissemination model for the graph analytics
master_seed = 42        # all randomness in the sweep derives from this

[population]
size = 1000             # participants (all subscribed to the one topic)
adversarial_fraction = 0.2   # or `adversarial = N` (count)
churn = 0.05            # honest nodes marked down after topology formation,
                        # as a fraction of the honest population
                        # (or `churn_count = N`; omit for no churn)
topic = "t0"

[strategies.honest]     # the strategy triad each class runs
connection = "uniform-sampler"   # dial: protocol kinds or uniform-sampler
target_degree = 8
acceptance = "accept-from-all"   # protocol acceptance kinds
fanout = "forward-to-all"

[strategies.adversarial]
connection = "uniform-sampler"
target_degree = 8
acceptance = "accept-from-all"
fanout = "silent-relay"          # the non-forwarding worst-case adversary

[execution]
runs_per_experiment = 100
publishes_per_run = 1   # optional; fresh message each, no state reset
```

Strategy tables also take the protocol's per-seam parameters where the
kind needs them: `bucket_count` (optional pinned bucket count, hash-gated
kinds) and `cap_buffer` (bounded acceptance kinds; default 3).

To sweep a parameter into a curve, add axes — each entry is one swept
parameter, and the cross-product expands into the experiment grid in
declaration order (first-declared axis varying slowest):

```toml
[[axes]]
parameter = "churn"     # size, adversarial, adversarial_fraction, churn,
values = [0.0, 0.05, 0.1]   # churn_count, target_degree, publishes_per_run

[[axes]]
parameter = "target_degree"
values = [4, 8, 16]
```

Everything in this file is result-affecting and is embedded in the output
manifest. Invalid descriptions — unknown kinds, conflicting count/fraction
spellings, grid points that leave no up-honest publisher/receiver pair —
are rejected before anything runs.

## Run

```sh
experiments --config sweep.toml --out results/my-sweep/ [--workers N] [--per-node-detail]
```

| flag | meaning | default |
|---|---|---|
| `--config` | sweep-description TOML | required |
| `--out` | output directory | required |
| `--workers` | maximum in-flight runs | available cores |
| `--per-node-detail` | also write per-run per-node tables | off |

Each in-flight run holds one full population in memory, so `--workers` is
also the memory knob: populations cost O(N²) for the full candidate views
(roughly 1.3 GB at N = 4 000 and 30 GB at N = 20 000 — use `--workers 1`
for the largest populations). Neither flag affects output bytes.

## Outputs

Three files per sweep:

- `manifest.json` — what ran: tool commit, master seed, the seed-derivation
  rule, and the expanded experiment list that run records reference by
  index.
- `runs.jsonl` — one row per run, streamed in canonical run order. Rows
  hold scalars and degree/depth-bounded histograms only (nothing scales
  with the population), including the per-run seed that replays the run.
- `aggregates.json` — one entry per experiment, a pure fold of its rows:
  distributions, percentiles, and probabilities as raw counts plus a
  Wilson 95 % interval (meaningful even at all-good samples).

With `--per-node-detail`, each run additionally writes
`run-NNNNNN-detail.jsonl` — one row per node with its first-receipt wave,
first-delivery origin, propagation-graph degrees, and (for eligible
receivers that missed) the classified miss cause. Detail never changes the
three main files.

There is no interruption resume: a stopped sweep leaves `runs.jsonl` as a
valid prefix in canonical order with no completion claim — re-run it (same
seed, same results).

To dissect an interesting run: take its `seed` and parameters from
`runs.jsonl` and the manifest, narrow the config to that experiment, and
re-run with `--per-node-detail` — the row reproduces exactly, plus the
per-node table for tracing which cluster missed and why.

## The shipped configurations

- [`m2-operating-point.toml`](m2-operating-point.toml) — N = 20 000,
  μ = 0.2, RF = 24: the formal M2 model's sized operating point, for the
  cost/latency comparison (manual run, ~15 min release at `--workers 1`).
- [`m2-bulk-regime.toml`](m2-bulk-regime.toml) — N = 4 000, μ = 0.2,
  RF = 16: the named bulk-regime validation point, for the
  P(good)-vs-coverage-law comparison (manual run, 8 000 runs).
- [`m2-smoke.toml`](m2-smoke.toml) — the same shape suite-sized; runs
  inside `cargo test --features experiments` asserting pipeline health
  only.

The executed comparison against the formal model's published values is
documented in [`docs/experiments/m2-comparison.md`](../../docs/experiments/m2-comparison.md).

## Further reading

- API documentation: `cargo doc --features experiments --open`, module
  `pubsub_node::experiments`.
- Design records: ADR 0032 (the deterministic driver), ADR 0033 (the
  output contract and statistics conventions), ADR 0034 (the optional
  `serde_json` dependency) under [`docs/decisions/`](../../docs/decisions/).

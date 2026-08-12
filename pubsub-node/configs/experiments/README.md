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

An experiment is described by a TOML file (the `*.toml` files in this
directory are complete working examples):

```toml
model = "m2"            # m1..m5: the dissemination model the analytics
master_seed = 42        # measure; all randomness derives from this seed

[population]
size = 1000             # participants (all subscribed to the one topic)
adversarial_fraction = 0.2   # or `adversarial = N` (count)
churn = 0.05            # honest nodes marked down after topology formation,
                        # as a fraction of the honest population
                        # (or `churn_count = N`; omit for no churn)
topic = "t0"

[strategies.honest]     # the selection-plane coordinates each class runs
pick_count = 8          # exact seeded uniform picks per topic (absent = every gate survivor; 0 = dial none)
fanout = "forward-to-relays"     # the forwarding policy each class runs

[strategies.adversarial]
pick_count = 8
fanout = "silent-relay"          # the non-forwarding worst-case adversary

[execution]
runs_per_experiment = 100
publishes_per_run = 1   # optional; fresh message each, no state reset
```

Strategy tables take the full coordinate set: `bucket_count` (the
hash-gate width; absent = ungated, and `1` is legal here as the ungated
point on an axis), `accept_cap` (absolute per-topic serving cap; absent =
unbounded, `0` = serve none), `accept_unverified` (default `false`:
acceptors verify the gate iff `bucket_count` is present), and `symmetric`
(default `false`: the bidirectional relay handshake).

A class additionally turns the **publisher pair** on — standing initiation
links, the M3/M5 wiring — by declaring a `publisher` sub-table with the
same knobs minus `symmetric`:

```toml
[strategies.honest]
pick_count = 8                   # the relay seam (k_in)
publisher = { pick_count = 3 }   # the publisher seam (k_out / s − 1);
fanout = "forward-to-relays"     # pick_count, bucket_count, accept_cap,
                                 # accept_unverified — presence = seam on
```

Fan-out kinds: `forward-to-relays` (held messages over relay links, own
publications seeded over publisher links — M2/M3), `forward-to-all` (every
held message over both kinds — M5), `silent-relay` (the non-forwarding
adversary).

The `model` name is validated against the **honest** class's wiring before
anything runs, so one config name always yields consistent wiring and
measurement: `m2`/`m4` require relay-only tables and `forward-to-relays`
(`m2` directional, `m4` symmetric); `m3` requires a publisher table,
`forward-to-relays`, and directional links; `m5`/`m1` require a publisher
table, `forward-to-all`, and directional links; `m1` additionally requires
`pick_count = 0` (no relay mesh — the k_in = 0 boundary). The selection
knobs (pick/bucket counts, caps) are otherwise free, and the adversarial
class is never constrained — its deviations are the experiment.

To sweep a parameter into a curve, add axes — each entry is one swept
parameter, and the cross-product expands into the experiment grid in
declaration order (first-declared axis varying slowest):

```toml
[[axes]]
parameter = "churn"     # size, adversarial, adversarial_fraction, churn,
values = [0.0, 0.05, 0.1]   # churn_count, pick_count, bucket_count,
                            # publisher_pick_count, publishes_per_run

[[axes]]
parameter = "pick_count"    # sets both classes' tables; boundary values are
values = [4, 8, 16]         # legal axis points (pick_count 0; bucket_count 1)
```

`publisher_pick_count` (k_out) overrides both classes' publisher
sub-tables and requires them declared in the base config — the axis sweeps
a seam, it never turns one on.

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

Each in-flight run holds one full population in memory; candidate views
are shared per topic (ADR 0038), so a single N = 20 000 run peaks well
under 1 GB and `--workers` is a wall-clock choice, not a memory budget.
Neither flag affects output bytes.

## Outputs

Three files per sweep:

- `manifest.json` — what ran: tool commit, master seed, the seed-derivation
  rule, and the expanded experiment list that run records reference by
  index.
- `runs.jsonl` — one row per run, streamed in canonical run order. Rows
  hold scalars and degree/depth-bounded histograms only (nothing scales
  with the population), including the per-run seed that replays the run.
  Each publish slice splits its sends by recipient class **and** by
  carrying link kind (`sends_by_kind`: relay/publisher, with degenerate
  columns at zero — under M3 the split reads relaying vs seeding, under
  M5 pull-serving vs push-forwarding).
- `aggregates.json` — one entry per experiment, a pure fold of its rows:
  distributions, percentiles, and probabilities as raw counts plus a
  Wilson 95 % interval (meaningful even at all-good samples).

With `--per-node-detail`, each run additionally writes
`run-NNNNNN-detail.jsonl` — one row per node with its first-receipt wave,
first-delivery origin, propagation-graph degrees, connection accounting
(serving slots split by the linked peer's class; the node's own dials
refused over capacity; refusals it issued, split by the refused dialer's
class), and (for eligible receivers that missed) the classified miss
cause. Detail never changes the three main files.

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
- [`m4-uniform-symmetric.toml`](m4-uniform-symmetric.toml) — N = 4 000,
  μ = 0.2, pick count 16 over the symmetric handshake: the M4-completing
  recipe, the E7 comparison's starting point (manual run, ~30 s release).
- [`m2-smoke.toml`](m2-smoke.toml) — the M2 shape suite-sized; runs inside
  `cargo test --features experiments` asserting pipeline health only.
- [`m3-smoke.toml`](m3-smoke.toml) / [`m5-smoke.toml`](m5-smoke.toml) —
  the publisher-pair shapes suite-sized (M3's initiation links; M5's
  k-in/k-out with `forward-to-all`); pipeline health only, including the
  sends-by-kind split populating both columns.
- [`comparisons/`](comparisons/) — the model-family comparison cells
  (M1/M3/M4/M5 coverage-law rows and the P(bad) ≤ 10⁻⁴ operating points,
  one config per cell with its master seed and run count); the suite
  validates every one of them.

The executed comparisons against the formal models' published values are
documented in [`docs/experiments/`](../../docs/experiments/):
[`m2-comparison.md`](../../docs/experiments/m2-comparison.md),
[`m3-comparison.md`](../../docs/experiments/m3-comparison.md),
[`m4-comparison.md`](../../docs/experiments/m4-comparison.md), and
[`m5-comparison.md`](../../docs/experiments/m5-comparison.md) (M1
included).

## Further reading

- API documentation: `cargo doc --features experiments --open`, module
  `pubsub_node::experiments`.
- Design records: ADR 0035 (the deterministic driver), ADR 0036 (the
  output contract and statistics conventions), ADR 0037 (the optional
  `serde_json` dependency), ADR 0038 (shared candidate views), ADR 0041
  (the publisher-pair configuration, per-model extraction, and the
  sends-by-kind split) under [`docs/decisions/`](../../docs/decisions/).

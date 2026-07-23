# Contract — Sweep configuration & invocation

The `experiments` binary's input surface: a TOML sweep-description file
(result-affecting; embedded in the manifest) plus clap invocation flags
(result-neutral; never in the manifest). Parse-at-the-edge: the experiments
API takes the parsed `SweepDescription`; the binary owns argument parsing
and reading the config file (FR-031). Output artifacts are written by the
sweep layer (contracts/output-artifacts.md).

## TOML sweep description (illustrative shape)

```toml
# Result-affecting. Embedded verbatim (as parsed values) in manifest.json.
model = "m2"                 # dissemination model; v1 accepts only "m2"
master_seed = 42

[population]
size = 10000                 # N
adversarial = 500            # count (or `adversarial_fraction = 0.05`)
churn = 0.05                 # proportion (or `churn_count = …`); may be 0
topic = "t0"

[strategies.honest]
connection = "uniform-sampler"     # experiments-only kind, or "hash-gated", …
target_degree = 12
acceptance = "accept-from-all"
fanout = "forward-to-relays"       # the protocol's default forwarding policy

[strategies.adversarial]
connection = "uniform-sampler"
target_degree = 12
acceptance = "accept-from-all"
fanout = "silent-relay"            # experiments-only kind

[execution]
runs_per_experiment = 200
publishes_per_run = 1              # default 1

[[axes]]                           # optional; none ⇒ single experiment
parameter = "churn"
values = [0.0, 0.05, 0.10]
```

Rules:

- Strategy `connection`/`acceptance` kinds accept the protocol kinds (005's)
  plus the experiments-only kinds (`uniform-sampler`, `silent-relay` on the
  fan-out seam); experiments-only kinds never appear in the node's own CLI.
- Strategy tables also accept the 005 per-seam parameters where the kind
  needs them: `bucket_count` (optional pinned B, hash-gated kinds) and
  `cap_buffer` (bounded acceptance kinds; default 3).
- Axes are an array of tables (`[[axes]]`, one swept `parameter` + its
  `values` each) — the shape that preserves declaration order, which is
  load-bearing: the cross-product expands into the manifest's experiment
  list in declaration order, first-declared axis varying slowest. (A plain
  TOML table would not preserve key order.)
- Validation errors (rejected before any run executes): unknown model or
  strategy kind; conflicting count/fraction spellings (`adversarial` vs
  `adversarial_fraction`, `churn` vs `churn_count`); zero eligible
  receivers; churn exceeding the honest population or leaving no up-honest
  publisher; `runs_per_experiment` or population size of zero. (The schema
  admits exactly one topic — a multi-topic request is a parse error, not a
  semantic validation.)
- Error messages are operator-facing: implementation-neutral, no FR/spec
  citations (Engineering Standards).

## Invocation flags (never in the manifest, never result-affecting)

```
experiments --config <sweep.toml> --out <dir> [--workers N] [--per-node-detail]
```

| Flag | Meaning | Default |
|---|---|---|
| `--config` | sweep-description TOML path | required |
| `--out` | output directory (three artifacts) | required |
| `--workers` | worker-pool size = max in-flight runs (each in-flight run holds a full population — size explicitly for memory at large N) | available cores |
| `--per-node-detail` | emit the opt-in per-node tables | off |

Progress reporting goes to stderr (operator UX; never a measurement or test
surface). Determinism tests enforce the result-neutrality of every flag in
this table (contracts/output-artifacts.md guarantee 1).

## Shipped configurations

`configs/experiments/m2-operating-point.toml` (N = 20 000, μ = 0.2,
RF = 24 — manual, cost/latency means), `m2-bulk-regime.toml` (named point
from m2's full-coverage validation grid — manual, P(good) vs law),
`m2-smoke.toml` (suite-sized; pipeline-health assertions only). Spec FR-033.

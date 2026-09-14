# Checking the CIP's evidence

Run these checks from the repository root with Python 3.11 or later; they
use only the standard library:

```sh
python3 -B -m unittest discover -s pubsub-node/docs/experiments -p 'test_cip_checks.py'
python3 -B pubsub-node/docs/experiments/check_cells_against_docs.py
python3 -B pubsub-node/docs/experiments/check_cip_bucket_table.py
python3 -B pubsub-node/docs/experiments/make_cip_figures.py --check
```

The `CIP evidence` workflow runs them on changes to the CIP, its source
experiment documents, the checkers and the figure generator.

## What is checked

- **Source transcription.** `check_cells_against_docs.py` matches operating
  and alternative costs, standing-link means/maxima and downtime budgets
  to a specific experiment document, section, configuration, metric row
  and value column. Coverage and churn counts are matched to their own
  configuration rows; formal Monte Carlo counts cannot substitute for
  the instrument's measurements. Missing documents, unknown models,
  missing metrics, missing or duplicated datasets and ambiguous source rows fail the check.
- **Predictions.** Ungated coverage and churn laws are recomputed with
  `analyse_churn.py`. Coverage/churn entries allow only the rounding error
  of eight decimal places; operating-point predictions allow 0.1% relative
  error for their 3–4 significant-digit reporting. Measured quantities are
  checked at the source cell's printed precision, using decimal half-up
  rounding. Counts and maxima must agree exactly. These tolerances check
  transcription, not statistical agreement with a model.
- **Gated evidence.** The gated reference counts and baseline/flooded
  predictions are checked against E20. The E10 gate ladder's counts,
  headroom and confidence intervals are checked against its source table.
  The concentration identity budget is an illustrative positive-integer
  chart input, not a measurement. M3's gated downtime is [recomputed from
  the model](m3-gated-downtime.md) and checked against both CIP tables.
- **Bucket tables.** `check_cip_bucket_table.py` reads Table 2 and checks
  every integer population from 2 through 20,000 under the declared
  baseline assumptions. It also compares Table 14's full row list,
  ceilings, bucket counts and rounded losses against its calculations.
  Its output for larger populations is arithmetic, not simulation.
- **Figures.** `make_cip_figures.py --check` compares the generated SVGs
  byte-for-byte with fresh output and verifies that every image referenced
  by the CIP and companion belongs to the figure inventory. `joining.svg`
  is maintained directly: its presence is checked, while its
  content requires human review. Regenerate the other figures by running
  the command without `--check`.

## What is outside these checks

The scripts do not rerun simulations or inspect their uncommitted raw
artifacts. Matching a source does not validate its experimental method.
The checker covers the six `cells.json` groups consumed by the figures:
`operating_points`, `alternatives`, `coverage_cells`, `churn_cells`,
`gated_point` and `gate_tradeoff`. Historical groups that the current
figures do not read, such as `severity`, `cap_tradeoff`, `tail_correction`
and `bucket_bounds`, and unused provenance/annotation fields are not
fully cross-checked. Most prose and CIP tables still need review; only
the specific table checks described above are automated.

Changing a source table's structure or adding an experiment family may
require updating its explicit source mapping. Keep that mapping separate
from the value being checked. A value's appearance elsewhere in a document
is never evidence that it belongs to this configuration or metric.

Regression tests deliberately corrupt plausible small values, swap in
values from neighbouring configurations and columns, remove source data,
and alter Table 14. They test that errors are rejected as well as that
the committed evidence passes.

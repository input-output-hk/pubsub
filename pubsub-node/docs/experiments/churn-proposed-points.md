# Churn at the proposed operating points — M3 (13, 7) and M4 RF = 9

The churn reduction has been tested twice: once on proxy cells chosen so
that failures could be counted, and once at the operating points under
heavy downtime. Both rounds are recorded in
[churn-tolerance.md](churn-tolerance.md).

The second round ran M3 (12, 8), M4 RF = 8 and M5 (9, 8). Two of those
three are the *superseded* configurations. The comparison now proposes
M3 (13, 7) and M4 RF = 9, so for the two designs still in contention the
churn evidence sits at neighbouring parameters rather than at the ones
being proposed. This pass closes that gap.

Nothing here changes what can be measured about a churn budget itself: a
budget is defined where P(bad) meets 10⁻⁴, and resolving a rate that low
would take on the order of 10⁵–10⁶ draws per churn level. What is under
test is the same reduction as before — that honest downtime with
per-epoch probability *p* enters as a shift of the adversarial fraction
to μ_eff = μ + *p*(1−μ) — now read at the parameters actually proposed.

Like the model comparisons, this document **informs, it does not gate**.
Statistical conventions (raw counts + Wilson 95 %, and why not ±1σ) are
documented in [m2-comparison.md](m2-comparison.md) §4 and apply
unchanged.

## Provenance

| | |
|---|---|
| Tool commit | `3887ea7` |
| Configurations | [`configs/experiments/churn/m3-n20k-rf13-s7-churn.toml`](../../configs/experiments/churn/m3-n20k-rf13-s7-churn.toml) (master seed 861), [`configs/experiments/churn/m4-n20k-rf9-churn.toml`](../../configs/experiments/churn/m4-n20k-rf9-churn.toml) (master seed 862) |
| Scale | N = 20 000, μ = 0.2 throughout |
| Runs | M3: 1 200 per churn level. M4: 2 000 per churn level |
| Predictions | each design's own coverage law read at μ_eff, computed before the cells ran and recorded in the config comments |

Artifacts are reproduced byte-for-byte from the tool commit and master
seeds; worker count is a wall-clock choice and not part of the contract.

### Why the two designs use different churn levels

M3 (13, 7) keeps the levels of the earlier operating-point round — 20,
25 and 30 % of honest nodes offline — so the two rounds are directly
comparable across the re-split.

M4 RF = 9 is shifted one level up, to 25, 30 and 35 %, and run deeper.
RF = 9 sits an order of magnitude below RF = 8 in P(bad), so at 20 %
downtime the law predicts about five failures in 1 200 draws, which
carries almost no power. At the levels used the law predicts roughly 28,
87 and 244 failures in 2 000.

## 1. Results

<!-- RESULTS: filled from the completed sweeps -->

## 2. What this establishes

<!-- CONCLUSIONS: written once the numbers are in -->

## 3. Scope

Single topic, oblivious adversaries, independent per-node downtime. The
correlated case — upgrade waves, region outages — is not represented by
a single independent *p* and is untouched here, as in the earlier
rounds. The budgets themselves remain read off the laws rather than
observed; what these cells test is that the laws apply under churn at
the proposed parameters.

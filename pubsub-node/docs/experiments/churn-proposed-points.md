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

**M3 (RF = 13, s = 7)** — 1 200 runs per level, master seed 861:

| churn | μ_eff | bad / runs | measured | Wilson 95 % | law at μ_eff | in CI | z |
|---|---:|---:|---:|---|---:|:--:|---:|
| 20 % | 0.360 | 39 / 1 200 | 0.0325 | [0.0239, 0.0441] | 0.0281 | yes | +0.93 |
| 25 % | 0.400 | 131 / 1 200 | 0.1092 | [0.0928, 0.1281] | 0.0953 | yes | +1.63 |
| 30 % | 0.440 | 321 / 1 200 | 0.2675 | [0.2432, 0.2933] | 0.2696 | yes | −0.17 |

Mean z = +0.80, Stouffer +1.38.

**M4 (RF = 9)** — 2 000 runs per level, master seed 862:

| churn | μ_eff | bad / runs | measured | Wilson 95 % | law at μ_eff | in CI | z |
|---|---:|---:|---:|---|---:|:--:|---:|
| 25 % | 0.400 | 28 / 2 000 | 0.0140 | [0.0097, 0.0202] | 0.0141 | yes | −0.02 |
| 30 % | 0.440 | 82 / 2 000 | 0.0410 | [0.0332, 0.0506] | 0.0437 | yes | −0.59 |
| 35 % | 0.480 | 254 / 2 000 | 0.1270 | [0.1131, 0.1423] | 0.1221 | yes | +0.67 |

Mean z = +0.02, Stouffer +0.03.

Every cell in both designs placed the prediction inside the
measurement's interval. Pre-churn coverage was complete in all 9 600
runs, so every failure counted is attributable to the downtime rather
than to a topology that was already bad before nodes went offline.

## 2. What this establishes

**The reduction holds at the proposed parameters.** Six of six cells,
across an effective adversarial fraction from 0.36 to 0.48 — beyond the
0.44 the earlier rounds reached. The configurations the comparison names
are now tested under churn directly, which previously was true only of
M5.

**M4's law is exact under churn; M3's is slightly optimistic.** This is
the more interesting result, and it resolves something the earlier
rounds left open. The first round noted an excess — measurements
sitting above their predictions — that did not grow with downtime and
so did not behave like a mistaken reduction, and it was recorded as
unexplained. Pooling per design across all three rounds separates it:

| design | churned cells | parameterisations | mean z | Stouffer |
|---|---:|---|---:|---:|
| M3 | 10 | (10, 4), (12, 8), (13, 7) | **+0.761** | **+2.41** |
| M4 | 10 | RF = 5, RF = 8, RF = 9 | +0.303 | +0.96 |

The excess is M3's, and it is present at every split tested rather than
at one. M4 shows none.

That matches an effect measured independently and without churn:
[finite-n.md](finite-n.md) §6 concludes that **M3's law is optimistic at
low fanout, at any population tested**, and its §7 lists M4 among the
designs never checked for a deviation of its own. These cells supply
that check, and the two findings agree — so the churn excess is most
likely not a property of the churn reduction at all, but the same
optimism in M3's law, showing up along a second axis.

Two qualifications. Stouffer +2.41 is suggestive rather than decisive,
and the association is between two deviations of the same sign in the
same design, which is not a demonstration of a common cause;
finite-n.md §7 is explicit that the mechanism behind M3's deviation is
unidentified. And the direction is conservative in both cases: it makes
M3's budget smaller than stated, never larger.

**The budgets themselves are still not measured.** A budget sits where
P(bad) meets 10⁻⁴ and cannot be sampled at any feasible run count. The
2.17 % and 7.43 % figures remain read off the laws. What has changed is
that the laws they are read from are now validated under churn at those
exact parameters rather than at neighbouring ones.

## 3. Scope

Single topic, oblivious adversaries, independent per-node downtime. The
correlated case — upgrade waves, region outages — is not represented by
a single independent *p* and is untouched here, as in the earlier
rounds. The budgets themselves remain read off the laws rather than
observed; what these cells test is that the laws apply under churn at
the proposed parameters.

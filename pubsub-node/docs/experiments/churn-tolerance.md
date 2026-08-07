# E13 — churn tolerance: the shifted-adversarial-fraction reduction, measured

The churn experiment from the [experiment program](../experiments-program.md),
executed against all five dissemination models. Like the model-family
comparisons it **informs, it does not gate**; the statistical conventions are
[`m2-comparison.md`](m2-comparison.md) §4.

## What is actually under test

`formal_spec/hybrid_dissemination/models/churn_tolerance.md` reduces churn to
the existing analysis rather than modelling it separately: an honest node that
is offline for an epoch holds its links and forwards nothing, which is
indistinguishable from the silent adversary already specified. Honest downtime
with per-epoch probability *p* should therefore shift the effective adversarial
fraction to

> μ_eff = μ + p(1 − μ)

with each model's own coverage law applying unchanged at the shifted value.
The **churn budget** p_max — the largest downtime a deployed configuration
absorbs while still meeting the 10⁻⁴ target — then follows from that law.

**p_max itself is not measurable.** It sits at P(bad) = 10⁻⁴ by definition, and
resolving a rate that low takes on the order of 10⁵–10⁶ draws *per churn point*.
At the operating points a few hundred runs observe zero bad graphs at every
churn level worth deploying, and learn nothing.

So this sweep tests the reduction instead, at parameters where P(bad) is
already large enough to count. If the reduction holds, every p_max follows from
a law that the [model-family comparisons](m2-comparison.md) have already
validated. If it fails, the churn analysis needs rebuilding — a far larger
finding than any single p_max number.

## Provenance

| | |
|---|---|
| Tool commit | `e609cee` |
| Configurations | [`configs/experiments/churn/`](../../configs/experiments/churn/) — `m3-n4k-rf10-s4-churn.toml` (seed 801), `m1-n4k-f16-churn.toml` (802), `m5-n4k-6-6-churn.toml` (803), `m2-n4k-rf16-churn.toml` (804), `m4-n4k-rf5-churn.toml` (805) |
| Grid | churn ∈ {0, 2, 5, 8, 12} % of the honest population, 4 000 runs per point — 25 cells, 100 000 runs |
| Timings | 91 min total on all cores; 11–23 min per model |
| Analysis | [`analyse_churn.py`](analyse_churn.py) — scores each point against its model's law read at μ_eff |
| Reference | `formal_spec/hybrid_dissemination/models/churn_tolerance.md` |

Each cell reuses a configuration whose zero-churn behaviour the comparison
documents already validated, so the churn axis is the only thing changing. M4
is the exception: its published coverage cells are at N = 20 000, where a churn
sweep costs hours, so a new N = 4 000 cell at RF = 5 was added with P(bad)
starting at a measurable 0.018.

Artifacts reproduce byte-for-byte from the tool commit and master seeds.

## 1. Results

| model | params | churn | μ_eff | bad / runs | measured | Wilson 95% | law at μ_eff | in CI | z |
|---|---|---:|---:|---:|---:|---|---:|:--:|---:|
| M1 | F=16 | 0% | 0.200 | 31 / 4000 | 0.0077 | [0.0055, 0.0110] | 0.0086 | yes | -0.57 |
| M1 | F=16 | 2% | 0.216 | 46 / 4000 | 0.0115 | [0.0086, 0.0153] | 0.0109 | yes | +0.39 |
| M1 | F=16 | 5% | 0.240 | 60 / 4000 | 0.0150 | [0.0117, 0.0193] | 0.0154 | yes | -0.22 |
| M1 | F=16 | 8% | 0.264 | 105 / 4000 | 0.0262 | [0.0217, 0.0317] | 0.0219 | yes | +1.89 |
| M1 | F=16 | 12% | 0.296 | 139 / 4000 | 0.0348 | [0.0295, 0.0409] | 0.0347 | yes | +0.00 |
| M2 | RF=16 | 0% | 0.200 | 27 / 4000 | 0.0067 | [0.0046, 0.0098] | 0.0088 | yes | -1.39 |
| M2 | RF=16 | 2% | 0.216 | 41 / 4000 | 0.0103 | [0.0076, 0.0139] | 0.0111 | yes | -0.53 |
| M2 | RF=16 | 5% | 0.240 | 71 / 4000 | 0.0177 | [0.0141, 0.0223] | 0.0158 | yes | +0.99 |
| M2 | RF=16 | 8% | 0.264 | 115 / 4000 | 0.0288 | [0.0240, 0.0344] | 0.0224 | **no** | +2.72 |
| M2 | RF=16 | 12% | 0.296 | 136 / 4000 | 0.0340 | [0.0288, 0.0401] | 0.0355 | yes | -0.51 |
| M3 | RF=10, s=4 | 0% | 0.200 | 31 / 4000 | 0.0077 | [0.0055, 0.0110] | 0.0088 | yes | -0.69 |
| M3 | RF=10, s=4 | 2% | 0.216 | 67 / 4000 | 0.0168 | [0.0132, 0.0212] | 0.0129 | **no** | +2.17 |
| M3 | RF=10, s=4 | 5% | 0.240 | 105 / 4000 | 0.0262 | [0.0217, 0.0317] | 0.0224 | yes | +1.64 |
| M3 | RF=10, s=4 | 8% | 0.264 | 154 / 4000 | 0.0385 | [0.0330, 0.0449] | 0.0381 | yes | +0.14 |
| M3 | RF=10, s=4 | 12% | 0.296 | 308 / 4000 | 0.0770 | [0.0691, 0.0857] | 0.0747 | yes | +0.56 |
| M4 | RF=5 | 0% | 0.200 | 72 / 4000 | 0.0180 | [0.0143, 0.0226] | 0.0184 | yes | -0.18 |
| M4 | RF=5 | 2% | 0.216 | 118 / 4000 | 0.0295 | [0.0247, 0.0352] | 0.0285 | yes | +0.36 |
| M4 | RF=5 | 5% | 0.240 | 233 / 4000 | 0.0583 | [0.0514, 0.0659] | 0.0523 | yes | +1.70 |
| M4 | RF=5 | 8% | 0.264 | 373 / 4000 | 0.0932 | [0.0846, 0.1027] | 0.0902 | yes | +0.68 |
| M4 | RF=5 | 12% | 0.296 | 679 / 4000 | 0.1698 | [0.1584, 0.1817] | 0.1715 | yes | -0.30 |
| M5 | (6, 6) | 0% | 0.200 | 15 / 4000 | 0.0037 | [0.0023, 0.0062] | 0.0033 | yes | +0.49 |
| M5 | (6, 6) | 2% | 0.216 | 20 / 4000 | 0.0050 | [0.0032, 0.0077] | 0.0057 | yes | -0.56 |
| M5 | (6, 6) | 5% | 0.240 | 47 / 4000 | 0.0118 | [0.0088, 0.0156] | 0.0119 | yes | -0.10 |
| M5 | (6, 6) | 8% | 0.264 | 100 / 4000 | 0.0250 | [0.0206, 0.0303] | 0.0235 | yes | +0.62 |
| M5 | (6, 6) | 12% | 0.296 | 214 / 4000 | 0.0535 | [0.0469, 0.0609] | 0.0534 | yes | +0.02 |

## 2. The reduction holds

Twenty-three of the twenty-five cells place the law inside the measurement's
Wilson 95 % interval, across five models, five churn levels, and a shift of the
adversarial fraction from 0.200 to 0.296. A second round at the operating
points (§2b) extends that to 0.44.

The strongest single piece of evidence is the **12 % row**, the largest shift
tested: z = +0.00, −0.51, +0.56, −0.30, +0.02 across the five models, mean
−0.04. At the point where a mistaken reduction would have diverged furthest,
all five models land essentially exactly on their laws.

## 2b. The operating points, run directly

The cells above were chosen so that P(bad) could be counted, which meant weaker
configurations than any deployment would use. The operating points were
therefore run as well, under heavier downtime — 20, 25 and 30 % of honest nodes
offline — where their failure rates become countable.

| model | params | churn | μ_eff | bad / runs | measured | Wilson 95% | law at μ_eff | in CI | z |
|---|---|---:|---:|---:|---:|---|---:|:--:|---:|
| M3 | RF=12, s=8 | 20% | 0.360 | 70 / 1200 | 0.0583 | [0.0464, 0.0731] | 0.0629 | yes | -0.65 |
| M3 | RF=12, s=8 | 25% | 0.400 | 240 / 1200 | 0.2000 | [0.1783, 0.2236] | 0.1935 | yes | +0.57 |
| M3 | RF=12, s=8 | 30% | 0.440 | 575 / 1200 | 0.4792 | [0.4510, 0.5075] | 0.4677 | yes | +0.79 |
| M4 | RF=8 | 20% | 0.360 | 28 / 1200 | 0.0233 | [0.0162, 0.0335] | 0.0213 | yes | +0.49 |
| M4 | RF=8 | 25% | 0.400 | 77 / 1200 | 0.0642 | [0.0516, 0.0795] | 0.0625 | yes | +0.24 |
| M4 | RF=8 | 30% | 0.440 | 193 / 1200 | 0.1608 | [0.1411, 0.1827] | 0.1630 | yes | -0.20 |
| M5 | (9, 8) | 20% | 0.360 | 24 / 1200 | 0.0200 | [0.0135, 0.0296] | 0.0189 | yes | +0.28 |
| M5 | (9, 8) | 25% | 0.400 | 74 / 1200 | 0.0617 | [0.0494, 0.0767] | 0.0594 | yes | +0.33 |
| M5 | (9, 8) | 30% | 0.440 | 208 / 1200 | 0.1733 | [0.1530, 0.1958] | 0.1646 | yes | +0.81 |

**All nine place the law inside the interval**, mean z = +0.30, Stouffer +0.89,
sd 0.47. Two things follow.

The configurations this analysis actually recommends are now tested under churn
directly, rather than by inference from proxies. And the two rounds together
carry the reduction from an adversarial fraction of 0.20 out to 0.44, more than
doubling it, with the laws tracking throughout.

It also bears on §3. The first round's excess does not reappear here: the
operating-point cells sit at +0.30 against the proxy cells' +0.58, with a
spread of 0.47 against 1.00. Pooled over all 29 churned cells the mean is
+0.494, so the effect has not vanished from the record, but it is absent from
the cells that carry the conclusions.

Provenance: `configs/experiments/churn/m{3,4,5}-n20k-op-churn.toml`, seeds
811-813, 1 200 runs per point, 115 min. M1 and M2 are omitted: their operating
points are over-provisioned enough that a countable failure rate needs hours
per cell, which is the same fact their large churn budgets record.

## 3. A residual the sweep cannot explain

The churned cells sit above their laws more often than chance alone predicts:

| | cells | mean z | Stouffer z |
|---|---:|---:|---:|
| zero churn | 5 | −0.47 | −1.04 |
| churned | 20 | **+0.58** | **+2.61** |
| difference | | +1.05 ± 0.38 | t = +2.79 |

Taken alone that suggests churn costs slightly more than the reduction
predicts. Two observations argue against reading it that way.

**It does not grow with churn.** By churn level the mean z runs −0.47, +0.37,
+0.80, +1.21, −0.04: it rises to 8 % and then vanishes at 12 %. The correlation
between z and churn is r = +0.18 (t = +0.88, n = 25), not significant. A
reduction that were simply wrong — down nodes costing more than adversaries —
would scale with the number of down nodes and show a strong positive
correlation. This does not.

**The excess is concentrated in one row.** The 8 % row carries Stouffer +2.70,
driven by M1 (+1.89) and M2 (+2.72); across five churn rows, one at that level
is unremarkable. The zero-churn baseline also happens to sit low at −0.47,
which inflates the difference test above.

Per model over churned cells: M3 +1.13 (Stouffer +2.25), M2 +0.67, M4 +0.61,
M1 +0.52, M5 −0.01.

**Direction matters more than the size.** Whatever the cause, the residual is
positive — measurement above law — so the effect on p_max is to make it
*smaller* than the law states, not larger. It therefore does not soften the
finding that M3 at (12, 8) is the most churn-brittle point in the family; if
anything it sharpens it, and M3 carries the largest per-model residual.

## 4. What this licenses

The reduction may be used. The churn budgets below are read off each model's
validated law at μ_eff, and the sweep confirms that law applies under churn
across the range it could be tested in:

| model | operating point | p_max |
|---|---|---:|
| M5 | (9, 8) | 2.18 % |
| M1 | F = 24 | 1.76 % |
| M2 | RF = 24 | 1.70 % |
| M4 | RF = 8 | 1.07 % |
| M3 | RF = 12, s = 8 | **0.54 %** |
| M3 | RF = 13, s = 7 | 2.17 % |

These remain law-derived rather than directly measured, and should be described
that way. What has changed is that the law's applicability under churn is now
evidence rather than assumption.

## 5. Limits

- **p_max is not measured and cannot be**, for the reason in the opening
  section. This sweep validates the mechanism that produces it.
- **The residual in §3 is unresolved.** Settling it needs more cells per churn
  level rather than more runs per cell — the per-cell intervals are already
  tight; what is thin is the number of independent cells at each level.
- **Correlated downtime is out of scope.** Every node here fails independently.
  Region outages and upgrade waves violate that, in the direction that makes
  the guarantee weaker.
- **One network size.** All cells are N = 4 000. The reduction is a statement
  about the laws' μ argument and is not expected to be size-dependent, but that
  was not tested here.

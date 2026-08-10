# Deep tail — the small-component correction, measured

The coverage laws count a single cut-off node exactly and a small cut-off
*group* only approximately, so they are expected to run mildly optimistic where
failures are rare. Both studies have carried that as a factor of about 1.11.
This measures it. Like the model-family comparisons it **informs, it does not
gate**; the statistical conventions are [`m2-comparison.md`](m2-comparison.md) §4.

## Why it needed 10^5 draws

The published tail cells are 30 000 draws. At a failure rate near 5×10⁻³ that
resolves the rate to about ±8 % — wider than the effect being looked for, so
neither team's sample could size it. Worse, the two samples disagreed in
opposite directions: the formal study's landed at 1.11× the law and ours at
0.94×, which is exactly what independent samples do when each is too small.

Separating a ten-percent effect at 3σ needs of the order of 10⁵ draws. These
runs are sized for that.

## Provenance

| | |
|---|---|
| Configurations | [`configs/experiments/tail/`](../../configs/experiments/tail/) — `m3-n4k-rf9-s5-deeptail.toml` (seed 821, 170 000 runs), `m4-n20k-rf6-deeptail.toml` (seed 822, 110 000 runs) |
| Seeds | fresh, independent of the 30 000-draw cells, so the samples pool rather than replace |
| Timings | M3 ~7.5 h; M4 substantially longer at N = 20 000 |

## 1. M3 (RF = 9, s = 5), N = 4 000

| source | bad / draws | P(bad) | ratio to law |
|---|---|---|---|
| this run | 912 / 170 000 | 0.005365 | **1.004** |
| ours, earlier | 150 / 30 000 | 0.005000 | 0.936 |
| formal study | 178 / 30 000 | 0.005933 | 1.110 |
| **pooled** | **1 240 / 230 000** | **0.005391** | **1.009** |

Law 0.005344. This run's Wilson 95 % is [0.005028, 0.005723], z = +0.12 against
the law. Pooled, the correction factor is **1.009 ± 0.029**, and the 1.11
figure is rejected at z = −3.37.

**There is no correction to apply at this cell.** The earlier disagreement was
sampling noise in both directions, and the truth sits on the law. The
95 % interval on the factor is [0.941, 1.071], which excludes 1.11 outright.

## 2. M4 (RF = 6), N = 20 000

Running. This is the cell that matters most, because the 1.11 figure was
originally measured on M4, at RF = 7 over 200 000 graphs. A prefix of the
sample is tracking near the law rather than above it, but a prefix is not a
result and the number is not reported here until the run completes.

## 3. What it changes

Operating points carry more margin than the corrected figures implied. Any
sizing decision that used ×1.11 to push a configuration over the target should
be re-read: at this cell the correction does not exist, and a decision that
turned on it may reverse.

## 4. Limits

- **One cell per design, not a curve.** The factor is measured where failures
  are frequent enough to count, near 5×10⁻³. Operating points sit near 10⁻⁴.
  Whether the factor is constant across that range is not established here,
  and the extrapolation is the same one the coverage laws already carry.
- **M4 is pending**, so the result stands on one design.

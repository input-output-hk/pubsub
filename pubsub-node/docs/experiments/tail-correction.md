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
| Timings | M3 ~7.5 h; M4 ~66 h at N = 20 000 |

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

| source | bad / draws | P(bad) | ratio to law |
|---|---|---|---|
| this run | 886 / 110 000 | 0.008055 | **0.963** |
| published cell | 260 / 30 000 | 0.008667 | 1.036 |
| **pooled** | **1 146 / 140 000** | **0.008186** | **0.979 ± 0.029** |

Law 0.008363. This run sits 1.1 standard errors *below* the law, the earlier
cell sat above it, and pooled they land on it. The 95 % interval on the factor
is [0.922, 1.035], which excludes 1.11 at more than four standard errors.

This is the cell that mattered most, because the 1.11 figure was originally
measured on M4, at RF = 7 over 200 000 graphs. It does not reproduce.

## 2b. Both designs together

| design | pooled draws | factor |
|---|---|---|
| M3 (RF = 9, s = 5), N = 4 000 | 230 000 | 1.009 ± 0.029 |
| M4 (RF = 6), N = 20 000 | 140 000 | 0.979 ± 0.029 |
| **combined** | **370 000** | **0.994 ± 0.021** |

Inverse-variance weighted, the two designs give 0.994 ± 0.021, so 1.11 is
rejected at z = −5.7. The correction is not present on either design, at two
network sizes, on four independent master seeds.

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
- The two cells sit at P(bad) ≈ 5 × 10⁻³ and 8 × 10⁻³. Whether the factor stays
  absent two decades below that, where the operating points live, is not
  established here; it is the same extrapolation the coverage laws already carry.

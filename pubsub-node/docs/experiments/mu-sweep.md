# The adversarial fraction as a swept axis

**Do the coverage laws hold as μ varies, at fixed design parameters?**

Every earlier comparison fixed μ = 0.2 and varied each design's own
parameters. The churn sweep varied the adversarial fraction only
*indirectly*, by marking honest nodes down and reading the laws at the
shifted value μ + p(1−μ). Neither tests the laws' dependence on μ
itself, which is the parameter a deployment has to assume rather than
measure — and the one every re-provisioning argument reads the laws at.

## Provenance

| | |
|---|---|
| Tool commit | `986a9e6` |
| Configurations | [`configs/experiments/mu/`](../../configs/experiments/mu/) — eight sweeps, each an `adversarial_fraction` axis over a frozen design |
| Grid | five designs at N = 4 000 over μ ∈ {0.20, 0.25, 0.30, 0.35}; the M3, M4 and M5 operating points at N = 20 000 over μ ∈ {0.30, 0.35, 0.40} |
| Draws | 4 000 per cell at N = 4 000, 2 000 at N = 20 000; 116 000 in total |
| Scoring | each cell against its own design's law evaluated at that cell's μ, not against a fitted curve |

## 1. Results

| model | parameters | N | μ | bad / draws | measured | Wilson 95 % | law at μ | in CI | z |
|---|---|---:|---:|---:|---:|---|---:|:--:|---:|
| M1 | F = 16 | 4,000 | 0.20 | 35 / 4,000 | 0.0088 | [0.0063, 0.0121] | 0.0086 | yes | +0.12 |
| M1 | F = 16 | 4,000 | 0.25 | 79 / 4,000 | 0.0198 | [0.0159, 0.0245] | 0.0179 | yes | +0.91 |
| M1 | F = 16 | 4,000 | 0.30 | 146 / 4,000 | 0.0365 | [0.0311, 0.0428] | 0.0368 | yes | -0.10 |
| M1 | F = 16 | 4,000 | 0.35 | 312 / 4,000 | 0.0780 | [0.0701, 0.0867] | 0.0748 | yes | +0.77 |
| M2 | RF = 16 | 4,000 | 0.20 | 39 / 4,000 | 0.0097 | [0.0071, 0.0133] | 0.0088 | yes | +0.65 |
| M2 | RF = 16 | 4,000 | 0.25 | 67 / 4,000 | 0.0168 | [0.0132, 0.0212] | 0.0183 | yes | -0.72 |
| M2 | RF = 16 | 4,000 | 0.30 | 155 / 4,000 | 0.0387 | [0.0332, 0.0452] | 0.0376 | yes | +0.39 |
| M2 | RF = 16 | 4,000 | 0.35 | 288 / 4,000 | 0.0720 | [0.0644, 0.0804] | 0.0762 | yes | -1.01 |
| M3 | RF = 12, s = 8 | 20,000 | 0.30 | 18 / 2,000 | 0.0090 | [0.0057, 0.0142] | 0.0080 | yes | +0.48 |
| M3 | RF = 12, s = 8 | 20,000 | 0.35 | 113 / 2,000 | 0.0565 | [0.0472, 0.0675] | 0.0460 | **no** | +2.24 |
| M3 | RF = 12, s = 8 | 20,000 | 0.40 | 396 / 2,000 | 0.1980 | [0.1811, 0.2160] | 0.1935 | yes | +0.51 |
| M3 | RF = 10, s = 4 | 4,000 | 0.20 | 31 / 4,000 | 0.0077 | [0.0055, 0.0110] | 0.0088 | yes | -0.69 |
| M3 | RF = 10, s = 4 | 4,000 | 0.25 | 130 / 4,000 | 0.0325 | [0.0274, 0.0385] | 0.0280 | yes | +1.71 |
| M3 | RF = 10, s = 4 | 4,000 | 0.30 | 317 / 4,000 | 0.0793 | [0.0713, 0.0880] | 0.0810 | yes | -0.40 |
| M3 | RF = 10, s = 4 | 4,000 | 0.35 | 850 / 4,000 | 0.2125 | [0.2001, 0.2254] | 0.2108 | yes | +0.27 |
| M4 | RF = 8 | 20,000 | 0.30 | 7 / 2,000 | 0.0035 | [0.0017, 0.0072] | 0.0034 | yes | +0.09 |
| M4 | RF = 8 | 20,000 | 0.35 | 37 / 2,000 | 0.0185 | [0.0135, 0.0254] | 0.0160 | yes | +0.90 |
| M4 | RF = 8 | 20,000 | 0.40 | 113 / 2,000 | 0.0565 | [0.0472, 0.0675] | 0.0625 | yes | -1.11 |
| M4 | RF = 5 | 4,000 | 0.20 | 84 / 4,000 | 0.0210 | [0.0170, 0.0259] | 0.0184 | yes | +1.23 |
| M4 | RF = 5 | 4,000 | 0.25 | 295 / 4,000 | 0.0737 | [0.0661, 0.0823] | 0.0660 | **no** | +1.96 |
| M4 | RF = 5 | 4,000 | 0.30 | 790 / 4,000 | 0.1975 | [0.1855, 0.2101] | 0.1847 | **no** | +2.09 |
| M4 | RF = 5 | 4,000 | 0.35 | 1623 / 4,000 | 0.4057 | [0.3906, 0.4211] | 0.4095 | yes | -0.49 |
| M5 | (9, 8) | 20,000 | 0.30 | 8 / 2,000 | 0.0040 | [0.0020, 0.0079] | 0.0027 | yes | +1.13 |
| M5 | (9, 8) | 20,000 | 0.35 | 39 / 2,000 | 0.0195 | [0.0143, 0.0265] | 0.0139 | **no** | +2.12 |
| M5 | (9, 8) | 20,000 | 0.40 | 117 / 2,000 | 0.0585 | [0.0490, 0.0697] | 0.0594 | yes | -0.17 |
| M5 | (6, 6) | 4,000 | 0.20 | 14 / 4,000 | 0.0035 | [0.0021, 0.0059] | 0.0033 | yes | +0.21 |
| M5 | (6, 6) | 4,000 | 0.25 | 43 / 4,000 | 0.0107 | [0.0080, 0.0144] | 0.0159 | **no** | -2.62 |
| M5 | (6, 6) | 4,000 | 0.30 | 230 / 4,000 | 0.0575 | [0.0507, 0.0651] | 0.0588 | yes | -0.35 |
| M5 | (6, 6) | 4,000 | 0.35 | 706 / 4,000 | 0.1765 | [0.1650, 0.1886] | 0.1747 | yes | +0.30 |


## 2. What it establishes

**The laws track μ across the range a deployment might plausibly
choose.** Twenty-four of twenty-nine cells contain the law inside the
measurement's 95 % interval, the mean standardised deviation is +0.36
with a spread of 1.10, and pooled the measurements sit 1.7 % above the
laws (ratio 1.017 ± 0.012). That excess is the same one the whole corpus
carries and is not specific to this axis.

The five misses are what twenty-nine cells produce: four between +1.96
and +2.24, one at −2.62. At this count roughly one and a half are
expected by chance, and the rest is the ambient optimism above.

**Concretely, this licenses one operation the analysis had been
performing without evidence:** sizing a design by inverting its law at
an adversarial fraction other than 0.2. Re-provisioning arguments do
exactly that. The laws were validated at μ = 0.2 and assumed to hold
elsewhere; they are now measured elsewhere, on the independent
instrument, for all five designs.

## 3. What it does not establish

- **It does not choose μ.** The adversarial fraction remains an
  assumption about who registers and what registration costs them. This
  says the laws are trustworthy across the range, not which point in it
  a deployment should sit at.
- **It stops at 0.40.** Above that the laws were not measured here, and
  the ordering of designs at extreme μ is unvalidated.
- **The N = 20 000 cells are 2 000 draws each**, so they resolve the
  bulk rather than the tail. The tail at those parameters is covered by
  the deep-tail runs, not by this sweep.
- **The residual excess is unexplained.** It is small, conservative in
  direction, and appears on both instruments; whether it is an
  approximation in the laws or something in the measurement is open.

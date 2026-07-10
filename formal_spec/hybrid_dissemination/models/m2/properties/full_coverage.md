# M2 — full coverage: probability of a bad graph

**Verdict: HYBRID** — closed-form law, validated; exact finite-N values by
simulation. A publisher injects only through its serving set. Scripts:
`sim_p03_full_coverage.py --table m2` (in `../../m3/scripts/`),
`sweep_m2_cost.py --coverage` (here).

## 1. Property

M2 samples one pull graph per epoch. A graph is **good** iff every honest
node can, as publisher, reach all other honest nodes — i.e. the
honest pull digraph is strongly connected. On a bad graph either some
publisher is **muted** (its serving set is dead: an *out*-defect) or some node
is **eclipsed** (its forwarder picks are dead: an *in*-defect) for the whole
epoch:

$$P_{\text{bad}} \;=\; P(\text{honest pull digraph not strongly connected}).$$

## 2. Guiding formula

Both defect counts are ≈ Poisson:

$$\boxed{\;P_{\text{bad}} \;\approx\; 1-e^{-H[(1-\rho_f)\,+\,u]},\qquad
1-\rho_f \approx e^{-RF(1-\mu)}\ \text{(muted publishers)},\quad
u \approx \mu^{RF}\ \text{(eclipse floor)}.\;}$$

The out-defect dominates whenever e^{−RF(1−μ)} ≫ μ^RF (all μ ≤ 0.2 regimes of
interest), so the sizing rule is the muted-publisher requirement:

$$RF \;\ge\; \frac{\ln(H/\delta)}{1-\mu}.$$

| symbol | meaning |
|---|---|
| RF | pull fanout (forwarders per node) |
| μ = k/N, H | adversarial fraction; honest count |
| ρ_f | spread-survival fixed point ρ_f = 1−e^{−RF(1−μ)ρ_f} |
| u | eclipse fixed point u = (μ+(1−μ)u)^RF (smallest root) |
| δ | tolerated P(bad) per epoch |

**Validity**: mean-field; mildly conservative in the bulk (over-predicts
P_bad by ~10–15 % where P_bad ≳ 0.1 — bad-publisher events cluster in small
dead-end components); exact to leading order in the deep tail, where
singleton sinks dominate.

## 3. Validation

**N = 20 000 grid** — reported as P_good = 1 − P(bad) (the script's output
format), predicted / measured, 150 graphs per cell
(`sim_p03_full_coverage.py --table m2`):

| RF \ μ | 0 | 0.05 | 0.1 | 0.2 |
|---|---|---|---|---|
| 8 | 0.001 / 0.000 | 0.000 / 0.000 | 0.000 / 0.000 | 0.000 / 0.000 |
| 10 | 0.403 / 0.333 | 0.241 / 0.213 | 0.108 / 0.120 | 0.005 / 0.000 |
| 12 | 0.884 / 0.907 | 0.808 / 0.813 | 0.693 / 0.620 | 0.338 / 0.373 |
| 14 | 0.984 / 1.000 | 0.969 / 0.973 | 0.941 / 0.967 | 0.803 / 0.873 |
| 16 | 0.998 / 1.000 | 0.995 / 0.993 | 0.990 / 0.987 | 0.957 / 0.973 |
| 18 | 1.000 / 1.000 | 0.999 / 1.000 | 0.998 / 1.000 | 0.991 / 0.993 |

**Small-N tail ladder** (`sweep_m2_cost.py --coverage`, N = 4 000, μ = 0.2,
strong-connectivity check):

| RF | P(bad) predicted | P(bad) MC | bad / trials | z |
|---|---|---|---|---|
| 12 | 0.1950 | 0.1670 | 167 / 1000 | −2.4 |
| 14 | 0.0428 | 0.0475 | 95 / 2000 | +1.0 |
| 16 | 0.0088 | 0.0081 | 65 / 8000 | −0.7 |

## 4. Answer — RF for P(bad) = 10⁻⁴ (N = 20 000, μ = 0.2)

**RF = 24**: P_bad ≈ 7.3×10⁻⁵ ≤ 10⁻⁴; RF = 23 gives 1.6×10⁻⁴, above target.
At this point the eclipse term u is 8 orders of magnitude below the
muted-publisher term — the fanout is spent protecting *publishing*, not
reception (reception alone needs only RF ≥ ln(H/δ)/ln(1/μ) ≈ 12).

# M2 — re-provisioning (cheapest RF at higher design μ)

**Verdict: HYBRID** — the validated coverage law inverted at elevated design
adversarial fractions; costs by closed form, cross-checked by the flood
simulator; MC law checks at elevated μ. Script (in `../scripts/`):
`sweep_m2_reprovision.py`.

## 1. Property

What deploying M2 against a *design* adversarial fraction μ_design > 0.2
costs: for each μ_design ∈ {0.2, 0.225, 0.25, 0.3, 0.35}, the smallest RF
with P(bad) ≤ δ = 10⁻⁴ at N = 20 000, its bandwidth / state price, and the
μ-shift budget of the new operating point
([`mu_shift_robustness.md`](mu_shift_robustness.md) semantics). Also the
**+1-notch** question at μ = 0.2: what one step of headroom (RF = 25)
costs and buys.

## 2. Guiding formula

The mean-field coverage law ([`full_coverage.md`](full_coverage.md)),
inverted at μ_design:

$$E = H\bigl[(1-\rho_f) + u\bigr],\qquad
RF \;\ge\; \frac{\ln(H/\delta)}{1-\mu}$$

(ρ_f, u as in the μ-shift analysis; near δ the muted-publisher term
1−ρ_f ≈ e^{−RF(1−μ)} dominates, the exact mirror of M1's in-isolation).
The sizing moves as 1/(1−μ) against a shrinking ln(H/δ): RF rises slowly
and the absolute cost falls with H.

## 3. Results — law inversion and MC checks

`sweep_m2_reprovision.py` (defaults; integer point and fractional law
crossing RF*):

| μ_design | RF (RF*) | P(bad) | msgs/message | copies/honest | links mean (2RF) / max | budget μ_eff (Δμ) | churn p_max | collapse |
|---|---|---|---|---|---|---|---|---|
| 0.200 | **24** (23.61) | 7.3×10⁻⁵ | 307 196 | 19.2 | 48 / 41 | 0.214 (+0.014) | ~1.7 % | 0.61 |
| 0.225 | 25 (24.33) | 6.0×10⁻⁵ | 300 308 | 19.4 | 50 / 42 | 0.247 (+0.022) | ~2.8 % | 0.63 |
| 0.250 | 26 (25.10) | 5.1×10⁻⁵ | 292 495 | 19.5 | 52 / 44 | 0.277 (+0.027) | ~3.6 % | 0.64 |
| 0.300 | 27 (26.80) | 8.7×10⁻⁵ | 264 594 | 18.9 | 54 / 42 | 0.306 (+0.006) | ~0.8 % | 0.66 |
| 0.350 | 29 (28.74) | 8.5×10⁻⁵ | 245 043 | 18.9 | 58 / 44 | 0.356 (+0.006) | ~0.9 % | 0.68 |

Cost cross-check (`--mc-costs`, 40 graphs/cell, seed 20260806): closed
forms within 0.04 % of the simulator at every point. Link maxima re-measured
with `sim_m2_degrees.py --mu <μ> --RF <RF>` (25 graphs, seed 2024). Law vs
MC at elevated μ_eff (`--mc-law`, strong-connectivity check, seed
20260806) — each new frozen design at two cells with P(bad) ≈ 0.1 / 0.4:

| design | μ_eff | P(bad) law | P(bad) MC | bad / trials | z |
|---|---|---|---|---|---|
| RF = 25 | 0.545 | 0.101 | 0.105 | 42 / 400 | +0.2 |
| RF = 25 | 0.610 | 0.386 | 0.328 | 82 / 250 | −2.0 |
| RF = 26 | 0.565 | 0.104 | 0.118 | 47 / 400 | +0.8 |
| RF = 26 | 0.630 | 0.415 | 0.384 | 96 / 250 | −1.0 |
| RF = 27 | 0.580 | 0.098 | 0.095 | 38 / 400 | −0.2 |
| RF = 27 | 0.645 | 0.417 | 0.420 | 105 / 250 | +0.1 |
| RF = 29 | 0.610 | 0.095 | 0.070 | 28 / 400 | −2.0 |
| RF = 29 | 0.670 | 0.406 | 0.380 | 95 / 250 | −0.8 |

All cells |z| ≤ 2 (M2's samples mirror M1's — a same-seed M2 graph is
M1's edge reversal, so the counts coincide).

## 4. Answer — provisioning curve and the +1 notch (N = 20 000, δ = 10⁻⁴)

Identical economics to M1 (the models are edge-reversal mirrors):
RF = 24 → 29 across μ_design = 0.2 → 0.35 with msgs/message *falling*
20 % and ≈ 19 copies/honest node throughout; the price is state
(48 → 58 mean links). **+1 notch at μ = 0.2**: RF = 25 costs +4.2 %
bandwidth and raises the μ-budget from 0.214 to **0.247** (churn
~1.7 % → ~5.8 %) — 3.4× the shift tolerance for one pull-fanout step.
M2's marginal-latency edge over M1 (its one measured advantage) is
untouched by re-provisioning: both models buy robustness on the same
curve.

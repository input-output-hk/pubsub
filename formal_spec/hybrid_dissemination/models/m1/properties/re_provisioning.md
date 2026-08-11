# M1 — re-provisioning (cheapest F at higher design μ)

**Verdict: HYBRID** — the validated coverage law inverted at elevated design
adversarial fractions; costs by closed form, cross-checked by the flood
simulator; MC law checks at elevated μ. Script (in `../scripts/`):
`sweep_m1_reprovision.py`.

## 1. Property

What deploying M1 against a *design* adversarial fraction μ_design > 0.2
costs: for each μ_design ∈ {0.2, 0.225, 0.25, 0.3, 0.35}, the smallest F
with P(bad) ≤ δ = 10⁻⁴ at N = 20 000 (k = μ_design·N, H = N − k), its
bandwidth / state price, and the μ-shift budget the new operating point
carries ([`mu_shift_robustness.md`](mu_shift_robustness.md) semantics).
Also the **+1-notch** question at μ = 0.2: what one step of headroom
(F = 25) costs and buys.

## 2. Guiding formula

The coverage law ([`full_coverage.md`](full_coverage.md)) inverted at
μ_design:

$$E = H\Bigl[\bigl(1-\tfrac{F}{N-1}\bigr)^{H-1} + \tfrac{\binom{k}{F}}{\binom{N-1}{F}}\Bigr],
\qquad F \;\ge\; \frac{\ln(H/\delta)}{1-\mu}.$$

The in-isolation wall moves as 1/(1−μ) while the requirement ln(H/δ)
*shrinks* with H = (1−μ)N, so F rises slowly — and the absolute cost
H·F·(H−1)/(N−1) can even fall as μ_design rises: fewer honest nodes to
serve nearly offsets the larger fanout.

## 3. Results — law inversion and MC checks

`sweep_m1_reprovision.py` (defaults; integer point and fractional law
crossing F*):

| μ_design | F (F*) | P(bad) | msgs/message | copies/honest | links mean (2F) / max | budget μ_eff (Δμ) | churn p_max | collapse |
|---|---|---|---|---|---|---|---|---|
| 0.200 | **24** (23.60) | 7.3×10⁻⁵ | 307 196 | 19.2 | 48 / 41 | 0.214 (+0.014) | ~1.8 % | 0.61 |
| 0.225 | 25 (24.32) | 5.9×10⁻⁵ | 300 308 | 19.4 | 50 / 42 | 0.247 (+0.022) | ~2.9 % | 0.63 |
| 0.250 | 26 (25.09) | 5.0×10⁻⁵ | 292 495 | 19.5 | 52 / 44 | 0.278 (+0.028) | ~3.7 % | 0.64 |
| 0.300 | 27 (26.78) | 8.6×10⁻⁵ | 264 594 | 18.9 | 54 / 42 | 0.306 (+0.006) | ~0.9 % | 0.66 |
| 0.350 | 29 (28.72) | 8.4×10⁻⁵ | 245 043 | 18.9 | 58 / 44 | 0.357 (+0.007) | ~1.0 % | 0.68 |

Cost cross-check (`--mc-costs`, 40 graphs/cell, seed 20260806): closed
forms within 0.04 % of the simulator at every point. Link maxima re-measured
with `sim_m1_degrees.py --mu <μ> --F <F>` (25 graphs, seed 2024). Law vs MC
at elevated μ_eff (`--mc-law`, strong-connectivity check, seed 20260806) —
each new frozen design at two cells with P(bad) ≈ 0.1 / 0.4:

| design | μ_eff | P(bad) law | P(bad) MC | bad / trials | z |
|---|---|---|---|---|---|
| F = 25 | 0.545 | 0.101 | 0.105 | 42 / 400 | +0.3 |
| F = 25 | 0.610 | 0.385 | 0.328 | 82 / 250 | −1.9 |
| F = 26 | 0.565 | 0.103 | 0.118 | 47 / 400 | +0.9 |
| F = 26 | 0.630 | 0.413 | 0.384 | 96 / 250 | −1.0 |
| F = 27 | 0.580 | 0.098 | 0.095 | 38 / 400 | −0.2 |
| F = 27 | 0.645 | 0.415 | 0.420 | 105 / 250 | +0.2 |
| F = 29 | 0.610 | 0.095 | 0.070 | 28 / 400 | −1.9 |
| F = 29 | 0.670 | 0.404 | 0.380 | 95 / 250 | −0.8 |

All cells |z| ≤ 2. As with the μ-shift budgets, the 10⁻⁴ tail at the
new points is law-read, not directly measured — the MC cells validate
the bulk; the closest direct tail evidence is
[`full_coverage.md`](full_coverage.md) §3, where the law tracked MC
with no visible bias.

## 4. Answer — provisioning curve and the +1 notch (N = 20 000, δ = 10⁻⁴)

**Re-provisioning is almost free in bandwidth**: F = 24 → 29 across
μ_design = 0.2 → 0.35 while msgs/message *falls* 20 % (H shrinks faster
than F grows) and copies per honest node stay ≈ 19; the price is state
(48 → 58 mean links/node) — M1 pays its robustness up front through the
δ-forced large fanout.

**+1 notch at μ = 0.2**: F = 25 costs +4.2 % bandwidth (307 196 → 319 996
msgs; 48 → 50 links) and raises the μ-budget from 0.214 to **0.247**
(Δμ +0.014 → +0.047, churn ~1.8 % → ~5.9 %) — 3.4× the shift tolerance
for one fanout step, because each F step scales E by ≈ e^{−(1−μ)}
(×0.45) while the law climbs only ≈ e^{F·Δμ} per unit shift.

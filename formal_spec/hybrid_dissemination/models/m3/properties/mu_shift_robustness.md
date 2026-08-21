# M3 — μ-shift robustness (degradation at frozen parameters)

**Verdict: HYBRID** — the validated coverage law read at a shifted
adversarial fraction; MC spot-checks at elevated μ. Script (in
`../scripts/`): `sweep_m3_mu_shift.py`.

## 1. Property

How P(bad) grows when the effective adversarial fraction μ_eff rises above
the design value with the deployed parameters frozen: (RF, s) = (13, 7),
chosen at μ = 0.2 for δ = 10⁻⁴ held with the 2 % disturbance margin
([`full_coverage.md`](full_coverage.md)).
N = 20 000 stays fixed; k = μ_eff·N, H = N − k shrinks with the shift.
Honest churn reads this curve at μ_eff = μ + p(1−μ)
(p = per-epoch honest downtime).

## 2. Guiding formula

The coverage law with (RF, s) frozen, read at μ_eff:

$$P_{\text{bad}} \;\approx\; 1-e^{-E},\qquad
E = H\bigl[\mu_{\text{eff}}^{RF} + \mu_{\text{eff}}^{s-1}\,e^{-RF(1-\mu_{\text{eff}})}\bigr],\qquad
H = (1-\mu_{\text{eff}})\,N.$$

Log-sensitivity at the operating point: d ln E/dμ ≈ 48 — the in-term
carries RF/μ = 65 (the family's steepest per-term slope), the out-term
(s−1)/μ + RF = 43, and the defect budget splits 29 : 71 between them.
The δ-cheapest split (12, 8) concentrates 82 % of the budget in the
μ-power in-term and reads d ln E/dμ ≈ 57.

## 3. Results — law curve and MC spot-checks

`sweep_m3_mu_shift.py` (exact every-publisher check):

| μ_eff | P(bad) law | P(bad) MC | bad / trials | z |
|---|---|---|---|---|
| 0.20 | 4.4×10⁻⁵ | — | — | — |
| 0.25 | 4.3×10⁻⁴ | — | — | — |
| 0.30 | 3.3×10⁻³ | — | — | — |
| 0.35 | 0.020 | 0.015 | 12 / 800 | −1.2 |
| 0.40 | 0.095 | 0.080 | 32 / 400 | −1.1 |
| 0.45 | 0.337 | 0.324 | 81 / 250 | −0.5 |
| 0.50 | 0.766 | — | — | — |
| 0.55 | 0.989 | — | — | — |
| 0.60 | ≈ 1 | — | — | — |

MC tracks the law at all elevated cells (|z| ≤ 1.2); the tail — where the
budget is read — was validated in [`full_coverage.md`](full_coverage.md)
§3 (second-order small-component term measured absent, 0.994 ± 0.021).

## 4. Answer — budget and collapse (N = 20 000, δ = 10⁻⁴)

**Budget**: P(bad) ≤ 10⁻⁴ holds up to **μ_eff ≈ 0.217** (Δμ ≈ +0.017;
churn reading p_max = Δμ/0.8 ≈ 2.2 %) — the margin the split is selected
for, tied with M5 for the smallest among the family's selections. The
δ-cheapest split (12, 8) reads μ_eff ≈ 0.204 (Δμ ≈ +0.004;
p_max ≈ 0.5 %) — the inadmissibility under the 2 % bar that forces the
re-split.
**Collapse**: P(bad) = ½ at **μ_eff ≈ 0.47**, the earliest in the
family. The bandwidth winner is the μ-brittleness loser: its μ-power
terms climb ~2× faster per unit μ than M1/M2's exponential terms.

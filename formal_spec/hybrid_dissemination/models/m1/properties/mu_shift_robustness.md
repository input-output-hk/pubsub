# M1 — μ-shift robustness (degradation at frozen parameters)

**Verdict: HYBRID** — the validated coverage law read at a shifted
adversarial fraction; MC spot-checks at elevated μ. Script (in
`../scripts/`): `sweep_m1_mu_shift.py`.

## 1. Property

How P(bad) grows when the effective adversarial fraction μ_eff rises above
the design value with the deployed parameters frozen: F = 25, the
margin-selected operating point at μ = 0.2, δ = 10⁻⁴
([`full_coverage.md`](full_coverage.md)). N = 20 000
stays fixed; k = μ_eff·N, H = N − k shrinks with the shift. Honest churn
reads this curve at μ_eff = μ + p(1−μ)
(p = per-epoch honest downtime).

## 2. Guiding formula

The coverage law with F frozen, read at μ_eff:

$$P_{\text{bad}} \;\approx\; 1-e^{-E},\qquad
E = H\bigl[e^{-F(1-\mu_{\text{eff}})} + \mu_{\text{eff}}^{F}\bigr],\qquad
H = (1-\mu_{\text{eff}})\,N.$$

Log-sensitivity at the operating point: d ln E/dμ ≈ F = 25 (the
exponential in-isolation term dominates), the smallest in the family — the
budget is ≈ ln(δ/E₀)/F to first order.

## 3. Results — law curve and MC spot-checks

`sweep_m1_mu_shift.py` (strong-connectivity check):

| μ_eff | P(bad) law | P(bad) MC | bad / trials | z |
|---|---|---|---|---|
| 0.20 | 3.3×10⁻⁵ | — | — | — |
| 0.25 | 1.1×10⁻⁴ | — | — | — |
| 0.30 | 3.5×10⁻⁴ | — | — | — |
| 0.35 | 1.1×10⁻³ | — | — | — |
| 0.40 | 3.6×10⁻³ | — | — | — |
| 0.45 | 0.012 | — | — | — |
| 0.50 | 0.037 | 0.030 | 24 / 800 | −1.1 |
| 0.55 | 0.112 | 0.102 | 51 / 500 | −0.8 |
| 0.60 | 0.319 | 0.300 | 90 / 300 | −0.7 |
| 0.65 | 0.714 | — | — | — |

MC tracks the law at all elevated cells (|z| ≤ 1.1); the tail — where the
budget is read — was validated in [`full_coverage.md`](full_coverage.md).

## 4. Answer — budget and collapse (N = 20 000, δ = 10⁻⁴)

**Budget**: P(bad) ≤ 10⁻⁴ holds up to **μ_eff ≈ 0.247** (Δμ ≈ +0.047;
churn reading p_max = Δμ/0.8 ≈ 5.9 %). **Collapse**: P(bad) = ½ at
**μ_eff ≈ 0.62** (0.625), latest in the family alongside M2 (the two
laws coincide at equal fanout). Between the two the
degradation is gradual — the margin rule's F = 25 buys a cushion well
beyond its 2 % bar (the δ-cheapest F = 24 reads μ_eff ≈ 0.214,
p_max ≈ 1.8 %).

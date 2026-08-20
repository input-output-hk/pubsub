# M2 — μ-shift robustness (degradation at frozen parameters)

**Verdict: HYBRID** — the validated coverage law read at a shifted
adversarial fraction; MC spot-checks at elevated μ. Script (in
`../scripts/`): `sweep_m2_mu_shift.py`.

## 1. Property

How P(bad) grows when the effective adversarial fraction μ_eff rises above
the design value with the deployed parameters frozen: RF = 25, the
margin-selected operating point at μ = 0.2 for δ = 10⁻⁴
([`full_coverage.md`](full_coverage.md); the δ-cheapest fanout is
RF = 24). N = 20 000 stays fixed; k = μ_eff·N, H = N − k shrinks with the
shift. Honest churn reads this curve at μ_eff = μ + p(1−μ)
(p = per-epoch honest downtime).

## 2. Guiding formula

The coverage law with RF frozen, read at μ_eff:

$$P_{\text{bad}} \;\approx\; 1-e^{-H[(1-\rho_f)\,+\,u]},\qquad
1-\rho_f \approx e^{-RF(1-\mu_{\text{eff}})},\quad
u \approx \mu_{\text{eff}}^{RF},\qquad H = (1-\mu_{\text{eff}})\,N.$$

Log-sensitivity at the operating point: d ln E/dμ ≈ RF = 25 (the
muted-publisher term dominates), the smallest in the family alongside
M1 — the budget is ≈ ln(δ/E₀)/RF to first order.

## 3. Results — law curve and MC spot-checks

`sweep_m2_mu_shift.py` (strong-connectivity check):

| μ_eff | P(bad) law | P(bad) MC | bad / trials | z |
|---|---|---|---|---|
| 0.20 | 3.3×10⁻⁵ | — | — | — |
| 0.25 | 1.1×10⁻⁴ | — | — | — |
| 0.30 | 3.5×10⁻⁴ | — | — | — |
| 0.35 | 1.1×10⁻³ | — | — | — |
| 0.40 | 3.7×10⁻³ | — | — | — |
| 0.45 | 0.012 | — | — | — |
| 0.50 | 0.037 | 0.033 | 20 / 600 | −0.5 |
| 0.55 | 0.113 | 0.115 | 46 / 400 | +0.1 |
| 0.60 | 0.320 | 0.324 | 81 / 250 | +0.1 |
| 0.65 | 0.716 | — | — | — |

MC tracks the law at the elevated cells (|z| ≤ 0.5, mixed signs — the
known bulk quality of the mean-field law); the tail — where the budget is
read — was validated in [`full_coverage.md`](full_coverage.md).

## 4. Answer — budget and collapse (N = 20 000, δ = 10⁻⁴)

**Budget**: P(bad) ≤ 10⁻⁴ holds up to **μ_eff ≈ 0.247** (Δμ ≈ +0.047;
churn reading p_max = Δμ/0.8 ≈ 5.8 %, clearing the 2 % margin bar
~3×; the δ-cheapest RF = 24 holds Δμ ≈ +0.014, p_max ≈ 1.7 %).
**Collapse**: P(bad) = ½ at **μ_eff ≈ 0.62**, latest in the family
alongside M1 (the two laws coincide at equal fanout). The large fanout
that δ = 10⁻⁴ forces buys a wide cushion beyond the target.

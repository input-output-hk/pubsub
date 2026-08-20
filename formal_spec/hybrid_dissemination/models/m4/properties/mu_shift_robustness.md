# M4 — μ-shift robustness (degradation at frozen parameters)

**Verdict: HYBRID** — the validated coverage law read at a shifted
adversarial fraction; MC spot-checks at elevated μ. Script (in
`../scripts/`): `sweep_m4_mu_shift.py`.

## 1. Property

How P(bad) grows when the effective adversarial fraction μ_eff rises above
the design value with the deployed parameters frozen: RF = 9, selected at
μ = 0.2 under the disturbance-margin rule (δ = 10⁻⁴; the δ-cheapest point
is RF = 8 — [`full_coverage.md`](full_coverage.md)). N = 20 000
stays fixed; k = μ_eff·N, H = N − k shrinks with the shift. Honest churn
reads this curve at μ_eff = μ + p(1−μ)
(p = per-epoch honest downtime).

## 2. Guiding formula

The coverage law with RF frozen, read at μ_eff:

$$P_{\text{bad}} \;\approx\; 1-e^{-E_{\text{iso}}},\qquad
E_{\text{iso}} = H\,\mu_{\text{eff}}^{RF}\,e^{-RF(1-\mu_{\text{eff}})},\qquad
H = (1-\mu_{\text{eff}})\,N.$$

Log-sensitivity at the operating point: d ln E/dμ ≈ RF/μ + RF = 54 (the
single defect term mixes a μ-power and an exponential factor).

## 3. Results — law curve and MC spot-checks

`sweep_m4_mu_shift.py` (honest-subgraph connectivity check):

| μ_eff | P(bad) law | P(bad) MC | bad / trials | z |
|---|---|---|---|---|
| 0.20 | 6.1×10⁻⁶ | — | — | — |
| 0.25 | 6.7×10⁻⁵ | — | — | — |
| 0.30 | 5.0×10⁻⁴ | — | — | — |
| 0.35 | 2.9×10⁻³ | — | — | — |
| 0.40 | 0.014 | — | — | — |
| 0.45 | 0.057 | 0.055 | 44 / 800 | −0.3 |
| 0.50 | 0.195 | 0.216 | 108 / 500 | +1.2 |
| 0.55 | 0.514 | 0.577 | 173 / 300 | +2.2 |
| 0.60 | 0.889 | — | — | — |

MC tracks the law at the elevated cells (|z| ≤ 2.2, mixed signs — bulk
scatter of the isolated-vertex approximation); the tail — where the budget
is read — was validated in [`full_coverage.md`](full_coverage.md)
(~1.1× small-component under-count).

## 4. Answer — budget and collapse (N = 20 000, δ = 10⁻⁴)

**Budget**: P(bad) ≤ 10⁻⁴ holds up to **μ_eff ≈ 0.259** (Δμ ≈ +0.059;
churn reading p_max = Δμ/0.8 ≈ 7.4 %, and ≈ 7.2 % with the ~1.1× tail
correction) — ~6.6× the δ-cheapest RF = 8 headroom (Δμ ≈ +0.009,
p_max ≈ 1.1 %); this headroom is what the margin selection buys.
**Collapse**: P(bad) = ½ at **μ_eff ≈ 0.55**.

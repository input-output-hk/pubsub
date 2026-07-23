# M4 — μ-shift robustness (degradation at frozen parameters)

**Verdict: HYBRID** — the validated coverage law read at a shifted
adversarial fraction; MC spot-checks at elevated μ. Script (in
`../scripts/`): `sweep_m4_mu_shift.py`.

## 1. Property

How P(bad) grows when the effective adversarial fraction μ_eff rises above
the design value with the deployed parameters frozen: RF = 8, chosen at
μ = 0.2 for δ = 10⁻⁴ ([`full_coverage.md`](full_coverage.md)). N = 20 000
stays fixed; k = μ_eff·N, H = N − k shrinks with the shift. Honest churn
reads this curve at μ_eff = μ + p(1−μ)
(p = per-epoch honest downtime).

## 2. Guiding formula

The coverage law with RF frozen, read at μ_eff:

$$P_{\text{bad}} \;\approx\; 1-e^{-E_{\text{iso}}},\qquad
E_{\text{iso}} = H\,\mu_{\text{eff}}^{RF}\,e^{-RF(1-\mu_{\text{eff}})},\qquad
H = (1-\mu_{\text{eff}})\,N.$$

Log-sensitivity at the operating point: d ln E/dμ ≈ RF/μ + RF = 48 (the
single defect term mixes a μ-power and an exponential factor).

## 3. Results — law curve and MC spot-checks

`sweep_m4_mu_shift.py` (honest-subgraph connectivity check):

| μ_eff | P(bad) law | P(bad) MC | bad / trials | z |
|---|---|---|---|---|
| 0.20 | 6.8×10⁻⁵ | — | — | — |
| 0.25 | 5.6×10⁻⁴ | — | — | — |
| 0.30 | 3.4×10⁻³ | — | — | — |
| 0.35 | 0.016 | — | — | — |
| 0.40 | 0.063 | 0.043 | 34 / 800 | −2.8 |
| 0.45 | 0.203 | 0.208 | 104 / 500 | +0.3 |
| 0.50 | 0.511 | 0.467 | 140 / 300 | −1.5 |
| 0.55 | 0.872 | — | — | — |
| 0.60 | 0.996 | — | — | — |

MC tracks the law at the elevated cells (|z| ≤ 2.8, mixed signs — bulk
scatter of the isolated-vertex approximation); the tail — where the budget
is read — was validated in [`full_coverage.md`](full_coverage.md)
(~1.1× small-component under-count).

## 4. Answer — budget and collapse (N = 20 000, δ = 10⁻⁴)

**Budget**: P(bad) ≤ 10⁻⁴ holds up to **μ_eff ≈ 0.209** (Δμ ≈ +0.009;
churn reading p_max = Δμ/0.8 ≈ 1.1 %, and ≈ 0.8 % with the ~1.1× tail
correction) — about twice M3's headroom at ~half of M1/M2's.
**Collapse**: P(bad) = ½ at **μ_eff ≈ 0.50**.

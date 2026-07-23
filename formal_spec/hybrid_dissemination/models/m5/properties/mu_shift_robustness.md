# M5 — μ-shift robustness (degradation at frozen parameters)

**Verdict: HYBRID** — the validated coverage law read at a shifted
adversarial fraction; MC spot-checks at elevated μ. Script (in
`../scripts/`): `sweep_m5_mu_shift.py`.

## 1. Property

How P(bad) grows when the effective adversarial fraction μ_eff rises above
the design value with the deployed parameters frozen:
(k_in, k_out) = (9, 8), chosen at μ = 0.2 for δ = 10⁻⁴
([`full_coverage.md`](full_coverage.md)). N = 20 000 stays fixed;
k = μ_eff·N, H = N − k shrinks with the shift. Honest churn reads this
curve at μ_eff = μ + p(1−μ)
(p = per-epoch honest downtime).

## 2. Guiding formula

The coverage law with (k_in, k_out) frozen, read at μ_eff:

$$P_{\text{bad}} \;\approx\; 1-e^{-E},\qquad
E = H\bigl[\mu_{\text{eff}}^{k_{in}}e^{-k_{out}(1-\mu_{\text{eff}})}
+ \mu_{\text{eff}}^{k_{out}}e^{-k_{in}(1-\mu_{\text{eff}})}\bigr],\qquad
H = (1-\mu_{\text{eff}})\,N.$$

Log-sensitivity at the operating point: d ln E/dμ ≈ k/μ + k′ ≈ 50 (both
defect terms mix a μ-power and an exponential factor) — nearly M3-steep.

## 3. Results — law curve and MC spot-checks

`sweep_m5_mu_shift.py` (strong-connectivity check):

| μ_eff | P(bad) law | P(bad) MC | bad / trials | z |
|---|---|---|---|---|
| 0.20 | 4.4×10⁻⁵ | — | — | — |
| 0.25 | 4.1×10⁻⁴ | — | — | — |
| 0.30 | 2.7×10⁻³ | — | — | — |
| 0.35 | 0.014 | — | — | — |
| 0.40 | 0.059 | 0.083 | 66 / 800 | +2.4 |
| 0.45 | 0.208 | 0.184 | 92 / 500 | −1.4 |
| 0.50 | 0.546 | 0.487 | 146 / 300 | −2.1 |
| 0.55 | 0.913 | — | — | — |
| 0.60 | 0.999 | — | — | — |

MC tracks the law at the elevated cells (|z| ≤ 2.4, mixed signs — bulk
scatter of the isolated-vertex approximation); the deep tail — where the
budget is read — was validated in [`full_coverage.md`](full_coverage.md).

## 4. Answer — budget and collapse (N = 20 000, δ = 10⁻⁴)

**Budget**: P(bad) ≤ 10⁻⁴ holds up to **μ_eff ≈ 0.217** (Δμ ≈ +0.017;
churn reading p_max = Δμ/0.8 ≈ 2.2 %) — the largest in the family, but
mostly *margin*, not structure: the cheapest integer point (9, 8) lands
2.3× under δ, while the sensitivity ≈ 50 is nearly M3-steep. **Collapse**:
P(bad) = ½ at **μ_eff ≈ 0.49**.

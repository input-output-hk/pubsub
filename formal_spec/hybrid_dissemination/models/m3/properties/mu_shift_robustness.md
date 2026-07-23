# M3 — μ-shift robustness (degradation at frozen parameters)

**Verdict: HYBRID** — the validated coverage law read at a shifted
adversarial fraction; MC spot-checks at elevated μ. Script (in
`../scripts/`): `sweep_m3_mu_shift.py`.

## 1. Property

How P(bad) grows when the effective adversarial fraction μ_eff rises above
the design value with the deployed parameters frozen: (RF, s) = (12, 8),
chosen at μ = 0.2 for δ = 10⁻⁴ ([`full_coverage.md`](full_coverage.md)).
N = 20 000 stays fixed; k = μ_eff·N, H = N − k shrinks with the shift.
Honest churn reads this curve at μ_eff = μ + p(1−μ)
(p = per-epoch honest downtime).

## 2. Guiding formula

The coverage law with (RF, s) frozen, read at μ_eff:

$$P_{\text{bad}} \;\approx\; 1-e^{-E},\qquad
E = H\bigl[\mu_{\text{eff}}^{RF} + \mu_{\text{eff}}^{s-1}\,e^{-RF(1-\mu_{\text{eff}})}\bigr],\qquad
H = (1-\mu_{\text{eff}})\,N.$$

Log-sensitivity at the operating point: d ln E/dμ ≈ RF/μ = 60 (the μ-power
in-term dominates), the steepest in the family — the bandwidth-minimal
split concentrates the defect budget in the most μ-sensitive term.

## 3. Results — law curve and MC spot-checks

`sweep_m3_mu_shift.py` (exact every-publisher check):

| μ_eff | P(bad) law | P(bad) MC | bad / trials | z |
|---|---|---|---|---|
| 0.20 | 7.8×10⁻⁵ | — | — | — |
| 0.25 | 1.0×10⁻³ | — | — | — |
| 0.30 | 8.0×10⁻³ | — | — | — |
| 0.35 | 0.046 | 0.054 | 43 / 800 | +1.0 |
| 0.40 | 0.194 | 0.203 | 81 / 400 | +0.5 |
| 0.45 | 0.556 | 0.592 | 148 / 250 | +1.2 |
| 0.50 | 0.928 | — | — | — |
| 0.55 | 0.999 | — | — | — |
| 0.60 | ≈ 1 | — | — | — |

MC tracks the law at all elevated cells (|z| ≤ 1.2); the tail — where the
budget is read — was validated in [`full_coverage.md`](full_coverage.md)
(×1.11 small-component under-count).

## 4. Answer — budget and collapse (N = 20 000, δ = 10⁻⁴)

**Budget**: P(bad) ≤ 10⁻⁴ holds only up to **μ_eff ≈ 0.204**
(Δμ ≈ +0.004; churn reading p_max = Δμ/0.8 ≈ 0.5 %, and ≈ 0.3 % with the
×1.11 tail correction) — the tightest in the family. **Collapse**:
P(bad) = ½ at **μ_eff ≈ 0.44**, the earliest. The bandwidth winner is the
μ-brittleness loser: the μ^{12} in-term climbs ~2.5× faster per unit μ
than M1/M2's exponential terms.

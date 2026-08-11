# M4 — re-provisioning (cheapest RF at higher design μ)

**Verdict: HYBRID** — the validated coverage law inverted at elevated design
adversarial fractions; costs by closed form, cross-checked by the flood
simulator; MC law checks at elevated μ. Script (in `../scripts/`):
`sweep_m4_reprovision.py`.

## 1. Property

What deploying M4 against a *design* adversarial fraction μ_design > 0.2
costs: for each μ_design ∈ {0.2, 0.225, 0.25, 0.3, 0.35}, the smallest RF
with P(bad) ≤ δ = 10⁻⁴ at N = 20 000, its bandwidth / state price, and
the μ-shift budget of the new operating point
([`mu_shift_robustness.md`](mu_shift_robustness.md) semantics). Also the
**+1-notch** question at μ = 0.2: what RF = 9 — the backlog's suggested
notch — costs and buys.

## 2. Guiding formula

The coverage law ([`full_coverage.md`](full_coverage.md)) inverted at
μ_design:

$$E = H\cdot\frac{\binom{k}{RF}}{\binom{N-1}{RF}}\cdot\Bigl(1-\tfrac{RF}{N-1}\Bigr)^{H-1},
\qquad RF \;\ge\; \frac{\ln(H/\delta)}{\ln(1/\mu) + (1-\mu)}.$$

M4's single defect class is doubly protected (own picks × others'
picks), so each RF step scales E by ≈ μ·e^{−(1−μ)} (×0.09 at μ = 0.2) —
the largest per-notch factor in the family. The integer grid is
correspondingly coarse: one RF covers a wide μ_design band, entering it
with a large margin and leaving it with almost none.

## 3. Results — law inversion and MC checks

`sweep_m4_reprovision.py` (defaults; integer point and fractional law
crossing RF*):

| μ_design | RF (RF*) | P(bad) | ×1.1 corr. | msgs/message | copies/honest | links mean (2RF) / max | budget μ_eff (Δμ) | churn p_max | collapse |
|---|---|---|---|---|---|---|---|---|---|
| 0.200 | **8** (7.84) | 6.8×10⁻⁵ | 7.6×10⁻⁵ | 188 798 | 11.8 | 16 / 29 | 0.209 (+0.009) | ~1.1 % | 0.50 |
| 0.225 | 9 (8.32) | 2.1×10⁻⁵ | 2.3×10⁻⁵ | 200 723 | 13.0 | 18 / 34 | 0.259 (+0.035) | ~4.4 % | 0.55 |
| 0.250 | 9 (8.81) | 6.7×10⁻⁵ | 7.4×10⁻⁵ | 187 498 | 12.5 | 18 / 31 | 0.259 (+0.009) | ~1.3 % | 0.55 |
| 0.300 | 10 (9.85) | 7.5×10⁻⁵ | 8.2×10⁻⁵ | 181 997 | 13.0 | 20 / 32 | 0.307 (+0.007) | ~1.0 % | 0.59 |
| 0.350 | 11 (10.99) | 9.8×10⁻⁵ | **1.08×10⁻⁴ ✗** | 172 896 | 13.3 | 22 / 30 | 0.350 (+0.000) | ~0.1 % | 0.62 |

The ×1.1 column applies the deep-tail small-component correction measured
in [`full_coverage.md`](full_coverage.md) §3. **At μ_design = 0.35 the
corrected value exceeds δ**: RF = 11 sits on the law crossing
(RF* = 10.99) with zero margin, so the conservative choice there is
**RF = 12** (P(bad) ≈ 1.8×10⁻⁵, msgs ≈ 189 796, 14.6 copies/honest,
μ-budget 0.390) — at that price M4's bandwidth is back to its μ = 0.2
level. Cost cross-check (`--mc-costs`, 40 graphs/cell, seed 20260806):
closed forms within 0.02 % of the simulator at every point. Link maxima
re-measured with `sim_m4_degrees.py --mu <μ> --RF <RF>` (25 graphs, seed
2024). Law vs MC at elevated μ_eff (`--mc-law`, connectivity check, seed
20260806) — each new frozen design at two cells with P(bad) ≈ 0.1 / 0.4:

| design | μ_eff | P(bad) law | P(bad) MC | bad / trials | z |
|---|---|---|---|---|---|
| RF = 9 | 0.470 | 0.096 | 0.113 | 45 / 400 | +1.1 |
| RF = 9 | 0.535 | 0.398 | 0.424 | 106 / 250 | +0.8 |
| RF = 10 | 0.515 | 0.095 | 0.105 | 42 / 400 | +0.7 |
| RF = 10 | 0.580 | 0.418 | 0.440 | 110 / 250 | +0.7 |
| RF = 11 | 0.555 | 0.097 | 0.125 | 50 / 400 | +1.7 |
| RF = 11 | 0.615 | 0.411 | 0.456 | 114 / 250 | +1.4 |

All cells |z| ≤ 2 (uniformly slightly above the law — the small-component
under-count, consistent with the ~1.1× tail factor). As with the
μ-shift budgets, the 10⁻⁴ tail at the new points is law-read, not
directly measured — the MC cells validate the bulk, and the tail rests
on the ~1.1× factor (corrected budgets read ≈ 0.002 lower, e.g. RF = 9:
0.2572). A direct check in the high-μ regime (`--tail-check`:
N = 4 000, μ = 0.35, RF = 8, 60 000 graphs, seed 20260806) measured
MC/law = ×1.04 (199/60 000, z = +0.6) — consistent with both no
correction and the ×1.1 carry-over, so RF = 11 at μ_design = 0.35 sits
within noise of the target either way and RF = 12 stays the safe
choice. The dedicated measurement has since landed:
[`tail-correction.md`](../../../../../pubsub-node/docs/experiments/tail-correction.md)
(independent code and method) pools 370 000 fresh draws across both
designs — M3 (RF = 9, s = 5) at N = 4 000 and M4 (RF = 6) at
N = 20 000 — and reads the factor at **0.994 ± 0.021**, rejecting ×1.1
at z = −5.7; the cell the factor was originally measured on (M4 RF = 7,
169 failures / 200 000 graphs) does not reproduce. On the measured
basis RF = 11 clears δ as-is. The measurement sits near
P(bad) ≈ 5–8×10⁻³, two decades above the operating tail, so RF = 12 is
retained here as the conservative pick; retiring the ×1.1 correction —
and reverting this row to RF = 11 — is deferred to a dedicated pass
over the documents that carry it.

## 4. Answer — provisioning curve and the +1 notch (N = 20 000, δ = 10⁻⁴)

**Provisioning is cheap and chunky**: RF = 8 → 11 (12 corrected) across
μ_design = 0.2 → 0.35; absolute bandwidth is nearly flat (189 k → 173 k
raw, back to ~190 k with the corrected 0.35 point), copies/honest node
rise 11.8 → ~13–14.6, and state stays the family's smallest throughout
(16 → 22–24 mean links vs M3's 38 → 52). Because one RF spans
0.225–0.25, the same deployment covers a band of design points — but at
the top of each band the margin is spent (Δμ +0.035 at 0.225 vs +0.009
at 0.25 for the same RF = 9).

**+1 notch at μ = 0.2 — RF = 9, the backlog's suggestion, verified**:
+13.6 % bandwidth (188 798 → 214 398 msgs, 11.8 → 13.4 copies/honest,
16 → 18 links) buys a μ-budget of **0.259** (Δμ +0.009 → +0.059, churn
~1.1 % → ~7.4 %) — the largest single-notch robustness jump in the
family, ~7× the base tolerance. M4 buys robustness in coarse, expensive,
very effective steps.

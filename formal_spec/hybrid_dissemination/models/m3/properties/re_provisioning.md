# M3 — re-provisioning (cheapest (RF, s) at higher design μ; split economics)

**Verdict: HYBRID** — the validated coverage law inverted at elevated design
adversarial fractions; costs by closed form, cross-checked by the flood
simulator; MC law checks at elevated μ. Script (in `../scripts/`):
`sweep_m3_reprovision.py`.

## 1. Property

What deploying M3 against a *design* adversarial fraction μ_design > 0.2
costs: for each μ_design ∈ {0.2, 0.225, 0.25, 0.3, 0.35}, the smallest
total budget B = RF + (s−1) with a split meeting P(bad) ≤ δ = 10⁻⁴ at
N = 20 000, the **bandwidth-minimal split** (smallest feasible RF — the
model's documented rule) *and* the **robustness-optimal split** of the
same budget (largest μ-shift budget), plus the two **notch flavours** at
μ = 0.2, which are distinct:

- **A. re-split at fixed budget** — 13 + 6 = 12 + 7 = 19: move one link
  from the initiation side to the μ-sensitive pull side;
- **B. +1 total budget** — B = 20, every feasible split.

Unlike the single-knob models, M3's split choice moves bandwidth (which
follows RF only) *and* robustness (the μ^RF in-term carries the family's
steepest log-sensitivity RF/μ) in opposite directions — the split, not
just the budget, is a policy decision.

## 2. Guiding formula

The coverage law ([`full_coverage.md`](full_coverage.md)) inverted at
μ_design:

$$E = H\bigl[\mu^{RF} + \mu^{s-1}e^{-RF(1-\mu)}\bigr],\qquad
RF \;\ge\; \frac{\ln(2H/\delta)}{\ln(1/\mu)},\qquad
s-1 \;\ge\; \frac{\ln(2H/\delta) - RF(1-\mu)}{\ln(1/\mu)}.$$

Raising μ_design inflates the μ^RF in-term much faster than the
exponential out-term (log-sensitivities RF/μ vs ≈ RF + (s−1)/μ·μ^{s−1}
share), so re-provisioning is spent mostly on RF; s−1 stays ≈ 7
throughout the grid.

## 3. Results — law inversion and MC checks

`sweep_m3_reprovision.py` (defaults). Both splits of each budget; bw =
bandwidth-minimal (documented rule), rb = robustness-optimal at the same
budget; fractional crossings RF*, B* = RF* + (s−1)* shown for the trend:

| μ_design | B (B*) | split | P(bad) | msgs/message | copies/honest | links mean (2B) / max | budget μ_eff (Δμ) | churn p_max | collapse |
|---|---|---|---|---|---|---|---|---|---|
| 0.200 | 19 (18.29) | **(12, 8)** bw | 7.8×10⁻⁵ | 153 604 | 9.6 | 38 / ~36 | 0.204 (+0.004) | ~0.5 % | 0.44 |
| 0.200 | 19 | (13, 7) rb | 4.4×10⁻⁵ | 166 403 | 10.4 | 38 / ~36 | 0.217 (+0.017) | ~2.2 % | 0.47 |
| 0.225 | 20 (19.41) | **(13, 8)** bw | 7.7×10⁻⁵ | 156 166 | 10.1 | 40 / 38 | 0.230 (+0.005) | ~0.6 % | 0.47 |
| 0.225 | 20 | (14, 7) rb | 5.2×10⁻⁵ | 168 177 | 10.8 | 40 / 38 | 0.240 (+0.015) | ~1.9 % | 0.50 |
| 0.250 | 21 (20.54) | **(14, 8)** bw | 8.0×10⁻⁵ | 157 503 | 10.5 | 42 / 37 | 0.254 (+0.004) | ~0.6 % | 0.50 |
| 0.250 | 21 | (15, 7) rb | 6.1×10⁻⁵ | 168 752 | 11.2 | 42 / 37 | 0.262 (+0.012) | ~1.6 % | 0.52 |
| 0.300 | 23 (22.92) | **(17, 7)** (only) | 8.7×10⁻⁵ | 166 601 | 11.9 | 46 / 36 | 0.304 (+0.004) | ~0.5 % | 0.57 |
| 0.350 | 26 (25.49) | **(19, 8)** bw | 6.4×10⁻⁵ | 160 550 | 12.4 | 52 / 41 | 0.360 (+0.010) | ~1.6 % | 0.61 |
| 0.350 | 26 | (20, 7) rb | 6.3×10⁻⁵ | 168 999 | 13.0 | 52 / 41 | 0.362 (+0.012) | ~1.8 % | 0.62 |

(17, 7) at μ_design = 0.3 is the tightest point (8.7×10⁻⁵) — at
that grid point B = 23 has a single feasible split and no slack. Cost
cross-check (`--mc-costs`, 40 graphs/cell, seed 20260806): closed forms
within 0.03 % of the simulator at every point. Link maxima re-measured
with `sim_m3_degrees.py --mu <μ> --RF <RF> --s <s>` (25 graphs, seed
2024; the accepted side depends on the budget only, not the split). Law
vs MC at elevated μ_eff (`--mc-law`, exact every-publisher check, seed
20260806) — every new frozen design at two cells with P(bad) ≈ 0.1 / 0.4;
16 cells, all |z| ≤ 1.6 (worst (13, 7) at 0.400: 30/400, z = −1.5).

As with the μ-shift budgets, the 10⁻⁴ tail at the new points is
law-read, not directly measured — the MC cells validate the bulk, and
the second-order small-component term is measured absent
(0.994 ± 0.021,
[`tail-correction.md`](../../../../../pubsub-node/docs/experiments/tail-correction.md);
the measurement sits at P(bad) ≈ 5–8×10⁻³, two decades above the
operating tail, and constancy across that range is the same
extrapolation the laws already carry). The μ-shift budgets stand at the
table's raw crossings — (17, 7) at μ_design = 0.3 keeps its thin
+0.004 margin at the bandwidth-minimal (and only) split of its budget.
A direct check in the new points' regime
(`--tail-check`: N = 4 000, μ = 0.3, (12, 5) — the same ~3.5:1 out:in
defect mix as (17, 7) — 40 000 graphs, seed 20260806) measured
MC/law = ×0.97 (251/40 000, z = −0.4): no under-count visible at
elevated μ, consistent with the measured factor. The
script's stair-free frontier-trend section
(sizing rules of M3 and M4 evaluated at fractional knobs) puts the
M4/M3 bandwidth-parity point at **μ ≈ 0.64** — quoted by the
[comparison](../../comparison.md) §5 frontier verdict.

## 4. Answer — provisioning curve, notches, and the (13, 7) claim

**Provisioning curve**: budget 19 → 26 across μ_design = 0.2 → 0.35, all
of it spent on RF (12 → 19; s−1 stays 6–7 — the out-term is cheap to
close at any μ). Bandwidth rises only 5–10 % across the grid (153.6 k →
166.6 k msgs at bw-minimal splits; the shrinking H offsets most of the
larger RF); per-honest-node copies rise 9.6 → 12.4. State grows 38 → 52
mean links. Latency — M3's one weak axis — *improves*: full-coverage
depth falls from 5.9 to 5.0 hops across the grid (`--mc-costs`; larger
RF means shallower trees), erasing most of M3's hop deficit in the
[comparison](../../comparison.md) §2. M3's real re-provisioning price is neither bandwidth nor
state but **the persistent thinness of its μ-shift budget at the
bandwidth-minimal split**: Δμ ≈ +0.004–0.005 at every grid point — sizing
for a bigger μ_design does not buy slack *around* it, because the
bandwidth-minimal rule always parks the in-term just under δ.

**Notch A — re-split (13, 7), the backlog's suggestion, verified**: at
the same 19-link budget, (13, 7) has P(bad) 4.4×10⁻⁵ and μ-budget
**0.217** (Δμ +0.017, churn ~2.2 %) —
**4× the shift tolerance of (12, 8)** for +8.3 % bandwidth (+0.8
copies/node) and zero extra state.
This exceeds M4's base-point budget (+0.009) while still costing 12 %
less bandwidth than M4 (166.4 k vs 188.8 k msgs): the robustness gap in
the [comparison](../../comparison.md) §4 is a property of the
bandwidth-minimal *split*, not of M3's mechanism.

**Notch B — +1 budget (B = 20)**: under the documented bandwidth-minimal
rule the extra link goes to the out-term — (12, 9), zero bandwidth
change, and **almost zero robustness** (μ-budget 0.207): the notch is
spent on the wrong defect class. Spending it on RF instead gives
(13, 8): μ-budget 0.230 (Δμ +0.030) at +8.3 % bandwidth; the
robustness-optimal split of B = 20 is **(14, 7)**: μ-budget **0.240**
(Δμ +0.040, churn ~5.0 %) at +16.7 % bandwidth. A +1 notch only buys
robustness if the split rule is changed along with the budget.

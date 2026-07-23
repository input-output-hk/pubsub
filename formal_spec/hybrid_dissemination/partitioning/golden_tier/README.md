# Golden-tier eclipse analysis

Analytical and numerical study of how a small tier of never-failing **golden** nodes affects the per-target eclipse probability of a regular honest node, under two dissemination models.

## Models

| Model | Regular layer | Golden layer | d-links |
|---|---|---|---|
| **RandCast / golden** | each honest node pushes to F random targets | each golden pushes to F_g random targets | n/a |
| **M2** (RingCast pull + golden push) | each regular j *requests* RF forwarders (pull) | each golden pushes to F_g random targets | dropped (assumed adversary-controlled via ID grinding) |

Both models share the golden push contribution and the eight modelling assumptions (uniform without-replacement sampling, independent picks, silent adversaries, no `p_fail`, no grinding on the random layer, regular-honest target, single round, per-target ε primary).

## Files

| File | Role |
|---|---|
| [`golden_tier_eclipse_report.md`](golden_tier_eclipse_report.md) | RandCast / golden derivation: exact P, exponential approximation and validity, k_max formula, feasibility floor, whole-network bound, running example. |
| [`golden_tier_eclipse_check.py`](golden_tier_eclipse_check.py) | Numerical verification for RandCast / golden: P_exact vs. P_approx, analytical vs. bisection k_max, δ diagnostic. |
| [`m2_eclipse_report.md`](m2_eclipse_report.md) | M2 derivation: motivation for dropping d-links, push×pull factorisation, power-law approximation, k_max formula (no feasibility floor), running example, comparison with RandCast at equal fanout including the `(μ·e^(1−μ))^F` pointwise inequality. |
| [`m2_eclipse_check.py`](m2_eclipse_check.py) | Numerical verification for M2. Analytical checks: exact vs. approximation, analytical vs. bisection k_max, M2 vs. RandCast at equal fanout, pointwise inequality. Monte-Carlo checks (`--trials`, `--seed`): closed-form mean/variance of the eclipsed-node count vs. simulation, confirming the negative push-side covariance / under-dispersion (property #4), and heterogeneous per-node RF (property #10). |
| [`m2_coverage_mc.py`](m2_coverage_mc.py) | Multi-hop coverage Monte-Carlo on the M2 propagation graph (`--trials`, `--seed`). (A) single-source coverage ≈ giant-component ρ(RF) < 1 — M2 is **not** strongly connected; (B) coverage vs. the mean-field fixed point under Θ(N) golden seeds (property #3); (C) structural isolation: 0 in-degree-0 nodes for M2 pull vs. ~e^{−F}·N for RandCast push (property #5, "no ln N threshold"); (D) delivery-tree depth ~ log N (property #6); (E) no golden, RF=⌈ln N⌉: pure pull reaches full coverage on its own (sinks ~ e^{−RF}·N → O(1)), vs. fixed RF=2 stuck at ρ(2)≈0.8; (F) coverage design rule `G·F_g ≳ N·ln(H·μ^RF/ε_net)` for coverage w.h.p. (property #3). |
| [`golden_tier_eclipse_calculator.html`](golden_tier_eclipse_calculator.html) | Interactive single-file calculator. Toggles between models (RandCast / M2) and ε scopes (per-target / whole-network); shows analytical and exact-bisection k_max, feasibility floor, and approximation-regime diagnostics. |

## Headline results

**RandCast / golden** — exponential decay:

$$P(\text{eclipse}) \approx \exp\!\left(-\tfrac{G F_g + (N-G-k) F}{N}\right), \qquad k_{\max}(\varepsilon) = N\!\left(1 - \tfrac{\ln(1/\varepsilon)}{F}\right) + G \tfrac{F_g - F}{F}.$$

Has a feasibility floor ε_min = e^(−λ_j(0)) > 0; golden tier substitutes (F_g − F)/F regular honest nodes per golden node.

**M2** — multiplicative push × pull factorisation:

$$P(\text{eclipse}) \approx e^{-G F_g / N} \cdot (k/N)^{RF}, \qquad k_{\max}(\varepsilon) \approx N \cdot \varepsilon^{1/RF} \cdot \exp\!\left(\tfrac{G F_g}{N \cdot RF}\right).$$

No feasibility floor; polynomial in ε^(1/RF); golden tier acts as multiplicative bonus exp(λ_push / RF).

**Comparison at equal fanout F = RF:** M2 strictly dominates RandCast — pointwise ratio (μ · e^(1−μ))^F ≤ 1, often by orders of magnitude. The structural reason is that pull gives j a deterministic in-degree of RF, eliminating the Poisson-tail failure mode of push.

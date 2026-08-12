# M5 — re-provisioning (cheapest (k_in, k_out) at higher design μ)

**Verdict: HYBRID** — the validated coverage law inverted at elevated design
adversarial fractions; costs by closed form, cross-checked by the flood
simulator; MC law checks at elevated μ. Script (in `../scripts/`):
`sweep_m5_reprovision.py`.

## 1. Property

What deploying M5 against a *design* adversarial fraction μ_design > 0.2
costs: for each μ_design ∈ {0.2, 0.225, 0.25, 0.3, 0.35}, the smallest
total budget B = k_in + k_out with a split meeting P(bad) ≤ δ = 10⁻⁴ at
N = 20 000 and the most-balanced split (the model's documented rule —
balanced minimises the defect sum at fixed B), its bandwidth / state
price, and the μ-shift budget of the new operating point
([`mu_shift_robustness.md`](mu_shift_robustness.md) semantics). Also the
**+1-notch** question at μ = 0.2 (budget 17 → 18). Following the
[comparison](../../comparison.md) §4 caution, the robustness numbers
separate **margin** (how far under δ the integer point lands) from
**structure** (the law's log-sensitivity ≈ 50/unit μ, steeper than
M1/M2's ≈ 24): M5's headroom is mostly margin.

## 2. Guiding formula

The coverage law ([`full_coverage.md`](full_coverage.md)) inverted at
μ_design (balanced sizing shown; both classes doubly protected):

$$E = H\bigl[\mu^{k_{in}}e^{-k_{out}(1-\mu)} + \mu^{k_{out}}e^{-k_{in}(1-\mu)}\bigr],
\qquad K \;\ge\; \frac{\ln(2H/\delta)}{\ln(1/\mu) + (1-\mu)}.$$

Like M4, each budget step scales E by ≈ μ·e^{−(1−μ)}; unlike M4 the
budget moves in steps of one link on one side, so the grid is half as
coarse (alternating (K, K) and (K+1, K) points).

## 3. Results — law inversion and MC checks

`sweep_m5_reprovision.py` (defaults; fractional balanced crossing
b* per side, B* = 2b*):

| μ_design | B (B*) | split | P(bad) | δ/E margin | msgs/message | copies/honest | links mean (2B) / max | budget μ_eff (Δμ) | churn p_max | collapse |
|---|---|---|---|---|---|---|---|---|---|---|
| 0.200 | 17 (16.26) | **(9, 8)** | 4.4×10⁻⁵ | 2.3× | 217 597 | 13.6 | 34 / 33 | 0.217 (+0.017) | ~2.2 % | 0.49 |
| 0.225 | 18 (17.25) | **(9, 9)** | 4.3×10⁻⁵ | 2.4× | 216 222 | 14.0 | 36 / 34 | 0.244 (+0.019) | ~2.4 % | 0.52 |
| 0.250 | 19 (18.27) | **(10, 9)** | 4.8×10⁻⁵ | 2.1× | 213 746 | 14.2 | 38 / 31 | 0.266 (+0.016) | ~2.2 % | 0.54 |
| 0.300 | 21 (20.43) | **(11, 10)** | 6.0×10⁻⁵ | 1.7× | 205 796 | 14.7 | 42 / 33 | 0.312 (+0.012) | ~1.7 % | 0.58 |
| 0.350 | 23 (22.80) | **(12, 11)** | 8.5×10⁻⁵ | 1.2× | 194 345 | 15.0 | 46 / 32 | 0.354 (+0.004) | ~0.6 % | 0.62 |

The margin column is δ/E at the design point: the μ-shift budgets are
bought almost entirely by integer-grid margin (log-sensitivity stays
≈ 44–50 across the grid), and the margin *thins* as μ_design rises —
by 0.35 the balanced point lands nearly on the law crossing and the
budget collapses to +0.004. Cost cross-check (`--mc-costs`, 40
graphs/cell, seed 20260806): closed forms within 0.03 % of the simulator
at every point. Link maxima re-measured with
`sim_m5_degrees.py --mu <μ> --k_in <a> --k_out <b>` (25 graphs, seed
2024). Law vs MC at elevated μ_eff (`--mc-law`, strong-connectivity
check, seed 20260806) — each new frozen design at two cells with
P(bad) ≈ 0.1 / 0.4:

| design | μ_eff | P(bad) law | P(bad) MC | bad / trials | z |
|---|---|---|---|---|---|
| (9, 9) | 0.445 | 0.098 | 0.105 | 42 / 400 | +0.5 |
| (9, 9) | 0.505 | 0.388 | 0.380 | 95 / 250 | −0.3 |
| (10, 9) | 0.470 | 0.101 | 0.083 | 33 / 400 | −1.3 |
| (10, 9) | 0.530 | 0.406 | 0.412 | 103 / 250 | +0.2 |
| (11, 10) | 0.515 | 0.106 | 0.063 | 25 / 400 | −3.6 * |
| (11, 10) | 0.570 | 0.402 | 0.392 | 98 / 250 | −0.3 |
| (12, 11) | 0.550 | 0.100 | 0.088 | 35 / 400 | −0.9 |
| (12, 11) | 0.605 | 0.405 | 0.392 | 98 / 250 | −0.4 |

\* re-measured with two independent 800-trial runs
(`sample_bad(M5Params(N=20000, k=10300, k_in=11, k_out=10), 800,
random.Random(seed))`, seeds 777001 / 777002): 72/800 (z = −1.6) and
87/800 (z = +0.2), pooled 159/1600 (z = −0.9) — the default-seed cell is
a fluctuation, the law stands. As with the μ-shift budgets, the 10⁻⁴
tail at the new points is law-read, not directly measured — the MC
cells validate the bulk; the closest direct tail evidence is the
50 000-graph deep-tail row in [`full_coverage.md`](full_coverage.md)
§3 (no correction factor observed for M5).

## 4. Answer — provisioning curve and the +1 notch (N = 20 000, δ = 10⁻⁴)

**Provisioning curve**: budget 17 → 23 across μ_design = 0.2 → 0.35 in
balanced steps; absolute bandwidth *falls* 11 % (217.6 k → 194.3 k msgs
— H shrinks faster than B grows), copies/honest node rise 13.6 → 15.0,
state 34 → 46 mean links. M5 remains best on no axis: at every grid
point it costs 21–42 % more bandwidth than M3 and holds ~2.1× the
state of M4. Its once-distinctive μ-headroom erodes with μ_design as the
integer margin thins (+0.017 → +0.004), while its structural sensitivity
stays M3-like (≈ 44–50), not M1/M2-like.

**+1 notch at μ = 0.2**: budget 18, best split **(9, 9)** — +5.9 %
bandwidth (217 597 → 230 397 msgs, 13.6 → 14.4 copies/honest, 34 → 36
links) buys a μ-budget of **0.244** (Δμ +0.017 → +0.044, churn
~2.2 % → ~5.4 %). Splitting the same 18 unevenly is strictly worse
((10, 8): 0.239; (12, 6): 0.209) — balanced stays optimal for
robustness, unlike M3 where robustness wants the *un*balanced split.

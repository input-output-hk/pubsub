# M4 — full coverage: probability of a bad graph

**Verdict: HYBRID** — closed-form law for P(bad); exact finite-N values by
simulation. Scripts (in `../scripts/`): `m4_model.py` (self-test),
`sim_m4_coverage.py`.

## 1. Property

M4 samples one undirected graph per epoch. A message floods from any honest
source and reaches exactly the honest nodes in its connected component of the
**honest-induced subgraph** (adversaries silent). A sampled graph is **good**
if that subgraph is connected — then every honest source covers all other
honest nodes; **bad** otherwise (some honest node is stranded for the epoch):

$$P_{\text{bad}} \;=\; P(\text{honest-induced subgraph disconnected}).$$

## 2. Guiding formula

Disconnection is dominated by **isolated honest vertices** — a node cut off in
both directions at once:

$$\boxed{\;P_{\text{bad}} \;\approx\; 1-e^{-E_{\text{iso}}},\qquad
E_{\text{iso}} = H\cdot\frac{\binom{k}{RF}}{\binom{N-1}{RF}}\cdot\Bigl(1-\tfrac{RF}{N-1}\Bigr)^{H-1}
\;\approx\; H\,\mu^{RF}\,e^{-RF(1-\mu)}\;}$$

The two factors are independent: **all RF of the node's own picks land on
adversaries** (μ^RF) **and no honest node picked it** (e^{−RF(1−μ)}).

**Which RF gives full coverage w.h.p.** — for a per-epoch failure target δ:

$$RF \;\ge\; \frac{\ln(H/\delta)}{\ln(1/\mu) + (1-\mu)}.$$

At μ = 0 the isolated-vertex term vanishes and **RF ≥ 2** suffices (random
RF-out connectivity, Fenner–Frieze) — no ln N, no seeding.

| symbol | meaning |
|---|---|
| RF | peers each node picks (bidirectional) |
| μ = k/N, H = N−k | adversarial fraction; honest count |
| E_iso | expected number of isolated honest nodes |
| δ | tolerated P(bad) per epoch |

**Validity**: isolated vertices dominate near/above the connectivity threshold,
so the estimate is exact to leading order; ≥2-node components add a
second-order term, measured absent in the deep tail (factor 0.994 ± 0.021,
[`tail-correction.md`](../../../../../pubsub-node/docs/experiments/tail-correction.md)).
That measurement sits at P(bad) ≈ 5–8×10⁻³, two decades above the 10⁻⁴
operating tail; constancy across the range is the same extrapolation the
law already carries.

## 3. Validation — N = 20 000, μ = 0.2 (H = 16 000)

Predicted vs Monte-Carlo (`sim_m4_coverage.py`); RF ≤ 6 from the preset sweep,
RF = 7 from a 200 000-graph run:

| RF | E_iso | P(bad) predicted | P(bad) MC | bad / trials | z |
|---|---|---|---|---|---|
| 3 | 11.6 | 1.000 | 1.000 | 2000 / 2000 | +0.0 |
| 4 | 1.042 | 0.647 | 0.646 | 2585 / 4000 | −0.1 |
| 5 | 0.0936 | 0.0893 | 0.0936 | 749 / 8000 | +1.3 |
| 6 | 8.40×10⁻³ | 8.36×10⁻³ | 8.67×10⁻³ | 260 / 30000 | +0.6 |
| 7 | 7.54×10⁻⁴ | 7.53×10⁻⁴ | 8.45×10⁻⁴ | 169 / 200000 | +1.4 |
| 8 | 6.76×10⁻⁵ | 6.76×10⁻⁵ | — (formula) | — | — |
| 9 | 6.07×10⁻⁶ | 6.07×10⁻⁶ | — (formula) | — | — |

The formula tracks MC across five orders of magnitude in P(bad). The RF = 7
row reads 1.12 ± 0.09 against the law (z = +1.4) — once read as a ~1.1×
small-component under-count, but the dedicated measurement
([`tail-correction.md`](../../../../../pubsub-node/docs/experiments/tail-correction.md),
370 000 fresh draws across both designs) puts the factor at
**0.994 ± 0.021**, rejecting ×1.11 at z = −5.7, and this cell does not
reproduce (M4 RF = 6 pooled: 0.979 ± 0.029). The law is read as exact in
the tail.

## 4. Answer — δ-cheapest RF and the operating point (N = 20 000, μ = 0.2)

**δ-cheapest RF = 8** (cheapest fanout with P(bad) ≤ 10⁻⁴ alone):
P(bad) ≈ 6.8×10⁻⁵. RF = 7 gives 7.5×10⁻⁴ (measured 8.5×10⁻⁴), above target.

**Operating point RF = 9** (the disturbance-margin selection,
[`../../comparison.md`](../../comparison.md)): P(bad) ≈ 6.1×10⁻⁶ —
δ holds up to μ_eff ≈ 0.259
([`mu_shift_robustness.md`](mu_shift_robustness.md)), where RF = 8 holds
only to ≈ 0.209.

## 5. Failure severity — what a bad graph costs

Conditional on bad, the stranded set is almost always a single node.
`sim_m4_severity.py` measures d = H − |largest component| and the sizes
of the straggler components on bad graphs at elevated μ:

| μ_eff | bad graphs | d = 1 | d = 2 | d ≥ 3 | max d | islets ≥ 2 nodes |
|---|---|---|---|---|---|---|
| 0.50 | 117 | 95 % | 5 % | 0 % | 2 | none (123 stragglers) |
| 0.55 | 156 | 70 % | 25 % | 5 % | 4 | none (213 stragglers) |

Every straggler component observed was a single isolated node — the
d ≥ 2 rows are multiple simultaneous singletons (Poisson multiplicity at
these E), not larger islets. At the operating point (E ≈ 6×10⁻⁶)
multiplicity collapses to one: **a δ-event is one honest node cut off in
both directions**. No partition-scale fragment ever appeared, and the
independent μ-sweep
([`mu-sweep.md`](../../../../../pubsub-node/docs/experiments/mu-sweep.md))
confirms the isolated-vertex law across μ = 0.2–0.4 with no
small-component excess.

# M3 — full coverage: probability of a bad graph

**Verdict: HYBRID** — closed-form law for P(bad); exact finite-N values by
simulation. Scripts (in `../scripts/`): `m3_model.py` (self-test),
`sim_m3_coverage.py` (this study); per-message tables in
`sim_p03_full_coverage.py`, `sim_p03_tail.py`.

## 1. Property

M3 samples one graph per epoch: the pull picks plus each node's s−1 standing
initiation targets. A graph is **good** iff **every message of every honest
publisher reaches all other honest nodes** — a message from p spreads from
{p} ∪ (p's honest initiation targets) over the pull relay edges; initiation
links never relay:

$$P_{\text{bad}} \;=\; P(\text{some publisher's messages cannot cover all honest nodes}).$$

## 2. Guiding formula

Badness is dominated by two isolated-vertex defect classes:

$$\boxed{\;P_{\text{bad}} \;\approx\; 1-e^{-E},\qquad
E \;=\; H\Bigl[\underbrace{\mu^{RF}}_{\text{in-isolated: cannot receive}}
\;+\;\underbrace{\mu^{s-1}\,e^{-RF(1-\mu)}}_{\text{out-isolated: muted publisher}}\Bigr]\;}$$

- **in-isolated** — all RF pull picks adversarial (exact hypergeometric
  C(k,RF)/C(N−1,RF)). Initiation links cannot help reception: they deliver
  only their owner's own messages, so they cannot supply a node with *every*
  publisher's traffic — the eclipse floor is carried by RF alone.
- **out-isolated** — no honest node picked it as forwarder
  ((1−RF/(N−1))^{H−1}) **and** all s−1 initiation targets adversarial
  (C(k,s−1)/C(N−1,s−1)); independent (others' picks vs own picks).

Sizing for a per-epoch failure target δ (split between the classes):

$$RF \;\ge\; \frac{\ln(2H/\delta)}{\ln(1/\mu)},\qquad
s-1 \;\ge\; \frac{\ln(2H/\delta) - RF(1-\mu)}{\ln(1/\mu)}.$$

The boundary s = 1 recovers the pull-only law
([M2 full coverage](../../m2/properties/full_coverage.md)); every s ≥ 2
strictly shrinks the out-term by μ^{s−1} — initiation links attack exactly
the muted-publisher defect, at ~zero bandwidth
([`expected_number_of_messages.md`](expected_number_of_messages.md)).

| symbol | meaning |
|---|---|
| RF | pull fanout; s−1 = standing initiation links per node |
| μ = k/N, H | adversarial fraction; honest count |
| E | expected isolated-vertex defects (in- + out-) |
| δ | tolerated P(bad) per epoch |

**Validity**: isolated vertices dominate near/above the threshold; exact to
leading order (small dead-end components add a second-order term, measured
absent in the deep tail — 0.994 ± 0.021,
[`tail-correction.md`](../../../../../pubsub-node/docs/experiments/tail-correction.md);
see §3).

## 3. Validation — μ = 0.2

Predicted vs Monte-Carlo (`sim_m3_coverage.py`, exact every-publisher check):

| N | RF | s | E | P(bad) predicted | P(bad) MC | bad / trials | z |
|---|---|---|---|---|---|---|---|
| 4 000 | 6 | 4 | 0.412 | 0.337 | 0.348 | 209 / 600 | +0.6 |
| 4 000 | 8 | 3 | 0.219 | 0.197 | 0.195 | 195 / 1000 | −0.2 |
| 4 000 | 8 | 5 | 0.0164 | 0.0163 | 0.0150 | 60 / 4000 | −0.7 |
| 4 000 | 10 | 4 | 0.0088 | 0.0088 | 0.0092 | 74 / 8000 | +0.5 |
| 20 000 | 8 | 3 | 1.103 | 0.668 | 0.647 | 194 / 300 | −0.8 |
| 20 000 | 10 | 4 | 0.0445 | 0.0435 | 0.0362 | 29 / 800 | −1.1 |
| 4 000 | 9 | 5 | 0.0054 | 0.0053 | 0.0059 | 178 / 30000 | +1.3 |

The deep-tail row (30 000 graphs, `validate.py --tail m3`) reads 1.11 ± 0.08
against the law — once read as a small-component under-count, but the cell
is underpowered for a ten-percent factor (fine for the law). The dedicated
measurement pools it with 170 000 fresh draws at the same point
([`tail-correction.md`](../../../../../pubsub-node/docs/experiments/tail-correction.md)
§1) and reads the factor at 1.009 ± 0.029 — combined across both designs
**0.994 ± 0.021**, rejecting ×1.11 at z = −5.7. The law is read as exact
in the tail; the measurement sits at P(bad) ≈ 5×10⁻³, two decades above
the 10⁻⁴ operating tail, the same extrapolation the law already carries.

## 4. Answer — (RF, s) for P(bad) = 10⁻⁴ (N = 20 000, μ = 0.2)

Smallest total budget RF + (s−1) = **19**. Bandwidth follows RF only, so the
budget-19 Pareto points differ in how they spend it:

| (RF, s) | P(bad) | relay copies / node |
|---|---|---|
| (11, 9) | 3.3×10⁻⁴ ✗ | 8.8 |
| (12, 8) δ-cheapest | 7.8×10⁻⁵ | 9.6 |
| **(13, 7)** | **4.4×10⁻⁵** | **10.4** |
| (14, 6) | 7.2×10⁻⁵ | 11.2 |

**Operating point (RF = 13, s = 7)**: the split holding the 2 % disturbance
margin (churn p_max ≈ 2.2 %,
[`mu_shift_robustness.md`](mu_shift_robustness.md)), ~2× under target. The
**δ-cheapest point (12, 8)** is the bandwidth-minimal split meeting δ
alone — RF = 12 is forced by the in-term alone (H·μ^11 = 3.3×10⁻⁴ > δ),
and s−1 = 7 initiation links then close the out-term — but it holds only
~0.5 % churn margin; (13, 7) buys ~4× the shift tolerance and ~2× the
P(bad) headroom for +0.8 copies/node.

## 5. Per-message success (secondary metric)

The probability that one *given* message covers everyone is much higher than
the per-epoch guarantee (it conditions on one publisher and one seed draw):

$$P_{\text{msg}} \;\approx\; \Bigl(1-(1-\rho_f)\bigl(1-(1-\mu)\rho_f\bigr)^{s-1}\Bigr)\,e^{-H\mu^{RF}},
\qquad \rho_f = 1-e^{-RF(1-\mu)\rho_f}.$$

Validated deep-tail at (RF = 11, s = 3): P_fail-per-msg measured
2.8×10⁻⁴ ± 0.8×10⁻⁴ vs predicted 3.3×10⁻⁴, all observed failures single-node
eclipse-floor events (`sim_p03_tail.py`; RF × s grids in
`sim_p03_full_coverage.py`). This metric is not the guarantee: summed over an
epoch's many messages, only the standing structure of §2 bounds the worst
publisher.

## 6. Failure severity — what a bad graph costs

Conditional on strict-bad, the failing set is almost always a single
node. `sim_m3_severity.py` counts the actually failing nodes on bad
graphs at elevated μ, the δ-cheapest split (12, 8) frozen (bad graphs
are collectable there; the defect classes are the same) — deaf (some
publisher cannot reach them even via seeding) vs mute (a publisher whose
seed set cannot cover H):

| μ_eff | bad graphs | d = 1 | d = 2 | d ≥ 3 | max d | deaf : mute |
|---|---|---|---|---|---|---|
| 0.40 | 114 | 92 % | 6 % | 2 % | 3 | 110 : 15 |
| 0.45 | 159 | 61 % | 29 % | 10 % | 5 | 222 : 20 |

(The larger multiplicities just track E ≈ 0.2 / 0.8 at these μ — Poisson
conditioned on ≥ 1; the operating point's E ≈ 4.4×10⁻⁵ collapses it to
one.) **A δ-event is one stranded node**: at (13, 7) the defect budget
splits 29 : 71 between the classes, so it is usually one muted publisher
(the μ^{s−1}e^{−RF(1−μ)} class) and else one eclipsed node; the
independent μ-sweep
([`mu-sweep.md`](../../../../../pubsub-node/docs/experiments/mu-sweep.md))
confirms the isolated-vertex law across μ = 0.2–0.4 with no
small-component excess. The seeding mechanism is
directly visible: ~9–15 publishers per graph have no requester path to
the giant SCC (mute under pure pull), and initiation links rescue
> 99.5 % of them (5 832 → 15 and 4 247 → 20 actually mute at the two
cells).

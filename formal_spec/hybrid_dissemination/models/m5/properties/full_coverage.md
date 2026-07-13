# M5 — full coverage: probability of a bad graph

**Verdict: HYBRID** — closed-form law for P(bad); exact finite-N values by
simulation. Scripts (in `../scripts/`): `m5_model.py` (self-test),
`sim_m5_coverage.py`.

## 1. Property

M5 samples one k_in/k_out digraph per epoch. A graph is **good** iff every
honest node can, as publisher, reach all other honest nodes — i.e. the honest
propagation digraph is strongly connected; **bad** otherwise (some honest
node cannot receive, or some publisher is muted, for the epoch):

$$P_{\text{bad}} \;=\; P(\text{honest propagation digraph not strongly connected}).$$

## 2. Guiding formula

Badness is dominated by **two classes of isolated vertices**, each doubly
protected (own picks × others' picks), mirror images of each other:

$$\boxed{\;P_{\text{bad}} \;\approx\; 1-e^{-E},\qquad
E \;=\; H\Bigl[\underbrace{\mu^{k_{in}}e^{-k_{out}(1-\mu)}}_{\text{in-isolated: cannot receive}}
\;+\;\underbrace{\mu^{k_{out}}e^{-k_{in}(1-\mu)}}_{\text{out-isolated: muted publisher}}\Bigr]\;}$$

- **in-isolated** — all k_in own picks adversarial (μ^{k_in}, exact
  hypergeometric) *and* no honest node out-picked it ((1−k_out/(N−1))^{H−1});
- **out-isolated** — all k_out own picks adversarial *and* no honest node
  in-picked it.

Exact symmetry: reversing every edge of M5(k_in, k_out) is distributed as
M5(k_out, k_in) and strong connectivity is reversal-invariant, so
P_bad(k_in, k_out) = P_bad(k_out, k_in) exactly. At a fixed total budget
B = k_in + k_out the product of the two defect terms is fixed
(= μ^B e^{−B(1−μ)}), so their sum is minimised by the **balanced split**
k_in ≈ k_out; for k_in = k_out = K the sizing rule for a per-epoch failure
target δ is

$$K \;\ge\; \frac{\ln(2H/\delta)}{\ln(1/\mu) + (1-\mu)}.$$

At μ = 0 both terms vanish and **k_in = k_out = 2** suffices (the k-in/k-out
random digraph is strongly connected w.h.p. for k ≥ 2, Fenner–Frieze; at
k_in = k_out = 1 it fails at a non-vanishing rate). The boundary cases
recover the single-mechanism laws: k_out = 0 gives E = H[μ^{k_in} +
e^{−k_in(1−μ)}] (pull-only) and k_in = 0 its mirror (push-only).

| symbol | meaning |
|---|---|
| k_in, k_out | inbound / outbound links each node opens |
| μ, H = (1−μ)N | adversarial fraction; honest count |
| E | expected isolated-vertex defects (in- + out-) |
| δ | tolerated P(bad) per epoch |

**Validity**: isolated vertices dominate near/above the connectivity
threshold; exact to leading order (small ≥2-node dead-end components add a
second-order term).

## 3. Validation — μ = 0.2

Predicted vs Monte-Carlo (`sim_m5_coverage.py`, strong-connectivity check;
the (3,6)/(6,3) pair exercises the exact swap symmetry):

| N | (k_in, k_out) | E | P(bad) predicted | P(bad) MC | bad / trials | z |
|---|---|---|---|---|---|---|
| 4 000 | (4,4) | 0.415 | 0.340 | 0.374 | 187 / 500 | +1.6 |
| 4 000 | (5,5) | 0.0371 | 0.0364 | 0.0315 | 63 / 2000 | −1.3 |
| 4 000 | (6,6) | 0.0033 | 0.0033 | 0.0039 | 31 / 8000 | +0.8 |
| 4 000 | (3,6) | 0.228 | 0.204 | 0.204 | 306 / 1500 | +0.0 |
| 4 000 | (6,3) | 0.228 | 0.204 | 0.190 | 285 / 1500 | −1.4 |
| 4 000 | (2,7) | 0.479 | 0.381 | 0.390 | 390 / 1000 | +0.6 |
| 20 000 | (4,4) | 2.084 | 0.876 | 0.860 | 172 / 200 | −0.6 |
| 20 000 | (5,5) | 0.187 | 0.171 | 0.175 | 105 / 600 | +0.3 |
| 4 000 | (6,7) | 0.0011 | 0.00107 | 0.00096 | 48 / 50000 | −0.8 |

The deep-tail row (50 000 graphs, `validate.py --tail m5`) confirms the law
three decades below the bulk cells.

The law tracks MC at both network sizes and across balanced and skewed
splits, with no visible bias.

## 4. Answer — (k_in, k_out) for P(bad) = 10⁻⁴ (N = 20 000, μ = 0.2)

Best split per total budget B = k_in + k_out (balanced is always optimal):

| B | best split | P(bad) |
|---|---|---|
| 15 | (7,8) | 4.9×10⁻⁴ |
| 16 | (8,8) | 1.35×10⁻⁴ |
| **17** | **(9,8)** | **4.4×10⁻⁵** |
| 18 | (9,9) | 1.2×10⁻⁵ |

**The smallest budget meeting the target is k_in + k_out = 17, split (9,8)
(≡ (8,9)): P(bad) ≈ 4.4×10⁻⁵.** The symmetric (8,8) just misses
(1.35×10⁻⁴); (9,9) buys a ~4× margin for one more link.

# M1 — full coverage: probability of a bad graph

**Verdict: HYBRID** — closed-form law for P(bad); exact finite-N values by
simulation. Scripts (in `../scripts/`): `m1_model.py` (self-test),
`sim_m1_coverage.py`.

## 1. Property

A sampled graph is **good** iff every message of every honest publisher
reaches all other honest nodes — i.e. the honest push digraph is strongly
connected:

$$P_{\text{bad}} \;=\; P(\text{honest push digraph not strongly connected}).$$

## 2. Guiding formula

Badness is dominated by two isolated-vertex defect classes:

$$\boxed{\;P_{\text{bad}} \;\approx\; 1-e^{-E},\qquad
E \;=\; H\Bigl[\underbrace{\Bigl(1-\tfrac{F}{N-1}\Bigr)^{H-1}}_{\text{in-isolated}\ \approx\ e^{-F(1-\mu)}}
\;+\;\underbrace{\frac{\binom{k}{F}}{\binom{N-1}{F}}}_{\text{out-isolated}\ \approx\ \mu^{F}}\Bigr]\;}$$

- **in-isolated** — no honest node picked it: no honest in-edge exists, so it
  is unreachable from *every* publisher — **seed-proof** (no seeding or
  repetition can reach it);
- **out-isolated** — all F of its own picks adversarial: a muted publisher.

The in-term dominates at every μ ∈ (0,1) (ln(1/μ) > 1−μ always), so the sizing
rule is

$$F \;\ge\; \frac{\ln(H/\delta)}{1-\mu},$$

which binds even at μ = 0 (the in-degree-0 obstruction is structural:
F = ln N at μ = 0,
[`randcast_partition_report.md`](../../../partitioning/randcast_partition_report.md)).

| symbol | meaning |
|---|---|
| F | push fanout (targets per node per message) |
| μ = k/N, H = N−k | adversarial fraction; honest count |
| E | expected isolated-vertex defects (in- + out-) |
| δ | tolerated P(bad) per graph |

**Validity**: isolated vertices dominate near/above the threshold; exact to
leading order.

## 3. Validation — μ = 0.2

`sim_m1_coverage.py` (strong-connectivity check):

| N | F | E | P(bad) predicted | P(bad) MC | bad / trials | z |
|---|---|---|---|---|---|---|
| 4 000 | 10 | 1.064 | 0.655 | 0.660 | 660 / 1000 | +0.4 |
| 4 000 | 12 | 0.214 | 0.192 | 0.186 | 372 / 2000 | −0.7 |
| 4 000 | 14 | 0.0429 | 0.0420 | 0.0413 | 165 / 4000 | −0.3 |
| 4 000 | 16 | 0.0086 | 0.0086 | 0.0103 | 82 / 8000 | +1.5 |
| 20 000 | 12 | 1.081 | 0.661 | 0.670 | 268 / 400 | +0.4 |
| 20 000 | 14 | 0.218 | 0.196 | 0.209 | 209 / 1000 | +1.0 |

The law tracks MC at both network sizes with no visible bias, including the
H-scaling (same F, ×5 N ⇒ E ×5).

## 4. Answer — F for P(bad) = 10⁻⁴ (N = 20 000, μ = 0.2)

**δ-cheapest F = 24**: P_bad ≈ 7.3×10⁻⁵ ≤ 10⁻⁴; F = 23 gives 1.6×10⁻⁴,
above target. The margin-selected operating point **F = 25**
([comparison](../../comparison.md)) runs at P_bad ≈ 3.3×10⁻⁵. (The out-term
is 8 orders of magnitude below the in-term at both — M1's wall is entirely
the seed-proof in-isolation.)

## 5. Failure severity — what a bad graph costs

Conditional on bad, the stranded set is almost always a single node.
`sim_m1_severity.py` measures d = H − |giant SCC| on bad graphs at
elevated μ (where they are collectable) and classifies the stranded
nodes — deaf (unreachable from the giant) vs mute (cannot reach it):

| μ_eff | bad graphs | d = 1 | d = 2 | d ≥ 3 | max d | deaf : mute |
|---|---|---|---|---|---|---|
| 0.55 | 115 | 95 % | 4 % | 1 % | 3 | 116 : 6 |
| 0.60 | 155 | 72 % | 25 % | 3 % | 4 | 191 : 14 |

Multiplicity matches Poisson(E) conditioned on ≥ 1 defect; at the
operating point E ≈ 3×10⁻⁵ collapses it to one: **a δ-event is one deaf
node** (the seed-proof in-isolation class — 95 % of stranded nodes even
at these μ), with the other H − 1 nodes operating normally. No fragment
beyond 4 nodes appeared in 270 bad graphs; a partition needs Θ(H)
simultaneous defects, unreachably far beyond the isolated-vertex regime.

# M4 — expected number of hops (latency) to full coverage

**Verdict: SIMULATION ONLY** — the scaling law ~ ln H / ln(branching) is known,
but the constant and the straggler tail have no closed form. Script (in
`../scripts/`): `sweep_m4_cost.py`.

## 1. Property

The number of forwarding rounds (hops) for a flood from a single honest source
to reach honest nodes — BFS depth on the honest-induced subgraph:

- **full-coverage hops** — the deepest honest node (broadcast completion time,
  the number consensus timing cares about);
- **mean-node hops** — a typical node's latency.

## 2. Guiding law (asymptotic only)

Flooding on the honest subgraph is a breadth-first spread with effective
branching factor equal to the honest degree minus the arrival link:

$$\text{hops} \;\sim\; \frac{\ln H}{\ln(\text{branching})} \;+\; O(1)\ \text{tail},
\qquad \text{branching} \approx 2\,RF\,(1-\mu)-1.$$

The leading term matches the mean-node depth; the additive constant and the
full-coverage tail (the last few stragglers) come from the graph's local
structure and are obtained by simulation.

| symbol | meaning |
|---|---|
| RF | peers each node picks (bidirectional) |
| μ = k/N, H = N−k | adversarial fraction; honest count |
| branching ≈ 2·RF·(1−μ)−1 | honest degree seen by the spread, minus arrival |

**Validity**: requires the honest subgraph connected (RF above the coverage
threshold — [`full_coverage.md`](full_coverage.md)); single honest source.

## 3. Results — N = 20 000, μ = 0.2 (H = 16 000)

`sweep_m4_cost.py`, 60 graphs per RF (coverage ≈ 1.0 for RF ≥ 5):

| RF | full-coverage hops | mean-node hops | ln H / ln(branching) |
|---|---|---|---|
| 4 | 7.72 | 5.51 | 5.74 |
| 5 | 6.78 | 4.95 | 4.97 |
| 6 | 6.02 | 4.58 | 4.50 |
| 7 | 5.93 | 4.30 | 4.17 |
| 8 | 5.08 | 4.07 | 3.92 |
| 9 | 5.00 | 3.90 | 3.73 |
| 10 | 5.00 | 3.79 | 3.57 |
| 12 | 4.97 | 3.63 | 3.34 |

The mean-node depth tracks ln H / ln(branching) closely; the full-coverage
depth sits ~1–1.5 hops above it (the straggler tail) and flattens near 5 once
branching is large. At the δ = 10⁻⁴ operating point RF = 8: **5.1 hops to
full coverage, 4.1 typical**.

# M1 — expected number of hops (latency) to full coverage

**Verdict: SIMULATION ONLY** — scaling law ~ ln H / ln(F(1−μ)); constants and
the straggler tail by simulation. Script (in `../scripts/`):
`sweep_m1_cost.py`.

## 1. Property

Hops for a push cascade from a single honest source — BFS depth on the honest
push digraph: **full-coverage hops** (the last node) and **mean-node hops**
(a typical node).

## 2. Guiding law (asymptotic only)

$$\text{hops} \;\sim\; \frac{\ln H}{\ln(F(1-\mu))} \;+\; O(1)\ \text{tail}.$$

The leading term matches the mean-node depth; the additive constant and the
last-straggler tail are obtained by simulation.

| symbol | meaning |
|---|---|
| F | push fanout |
| μ = k/N, H = N−k | adversarial fraction; honest count |
| branching ≈ F(1−μ) | honest out-degree driving the spread |

**Validity**: full coverage ([`full_coverage.md`](full_coverage.md)); single
honest source.

## 3. Results — N = 20 000, μ = 0.2 (H = 16 000)

`sweep_m1_cost.py`, 40 graphs per F:

| F | full-coverage hops | mean-node hops | ln H / ln(F(1−μ)) |
|---|---|---|---|
| 12 | 6.20 | 4.52 | 4.28 |
| 16 | 5.30 | 4.04 | 3.80 |
| 20 | 5.00 | 3.77 | 3.49 |
| 24 | 5.00 | 3.60 | 3.28 |
| 28 | 4.05 | 3.44 | 3.11 |

Mean-node depth tracks the law with a ~+0.3 constant; full coverage costs a
further ~1–1.5 straggler hops. At the δ = 10⁻⁴ operating point F = 24:
**5.0 hops to full coverage, 3.6 typical**.

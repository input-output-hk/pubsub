# M5 — expected number of hops (latency) to full coverage

**Verdict: SIMULATION ONLY** — scaling law ~ ln H / ln((k_in+k_out)(1−μ));
constants and the straggler tail by simulation. Script (in `../scripts/`):
`sweep_m5_cost.py`.

## 1. Property

Hops for a cascade from a single honest publisher — BFS depth on the honest
propagation digraph: **full-coverage hops** (the last node) and **mean-node
hops** (a typical node). Publisher at depth 0; every relay costs 1.

## 2. Guiding law (asymptotic only)

The spread is breadth-first with effective branching equal to a node's honest
out-degree — its own live out-picks plus its honest requesters:

$$\text{hops} \;\sim\; \frac{\ln H}{\ln\bigl((k_{in}+k_{out})(1-\mu)\bigr)} \;+\; O(1)\ \text{tail}.$$

The leading term matches the mean-node depth; the additive constant and the
last-straggler tail are obtained by simulation. Only the sum k_in + k_out
enters the branching.

| symbol | meaning |
|---|---|
| k_in, k_out | inbound / outbound links each node opens |
| μ, H = (1−μ)N | adversarial fraction; honest count |
| branching ≈ (k_in+k_out)(1−μ) | honest out-degree driving the spread |

**Validity**: full coverage ([`full_coverage.md`](full_coverage.md)); single
honest publisher.

## 3. Results — N = 20 000, μ = 0.2 (H = 16 000)

`sweep_m5_cost.py`, 40 graphs per cell:

| k_in | k_out | full-coverage hops | mean-node hops | ln H / ln(branching) |
|---|---|---|---|---|
| 6 | 6 | 6.00 | 4.52 | 4.28 |
| 8 | 8 | 5.08 | 4.06 | 3.80 |
| 9 | 8 | 5.00 | 3.94 | 3.71 |
| 9 | 9 | 5.00 | 3.88 | 3.63 |
| 10 | 10 | 5.00 | 3.78 | 3.49 |
| 12 | 12 | 4.95 | 3.62 | 3.28 |

Mean-node depth tracks the law with a ~+0.3 constant; full coverage costs a
further ~1 straggler hop. At the δ = 10⁻⁴ operating point (9, 8):
**5.0 hops to full coverage, 3.9 typical**.

# M2 — expected number of hops (latency) to full coverage

**Verdict: SIMULATION ONLY** — scaling law ~ ln H / ln(RF(1−μ)); constants and
the straggler tail by simulation. Script:
`sweep_m2_cost.py` (in `../scripts/`).

## 1. Property

Hops from the publisher until honest nodes receive — BFS depth over the
directed pull edges (forwarder→requester): **full-coverage hops** and
**mean-node hops**. Publisher at depth 0; every relay costs 1.

## 2. Guiding law (asymptotic only)

$$\text{hops} \;\sim\; \frac{\ln H}{\ln(RF(1-\mu))} \;+\; O(1)\ \text{tail},$$

branching = a node's honest requester count ≈ RF(1−μ). The leading term
matches the mean-node depth; constants and the last-straggler tail are
obtained by simulation.

| symbol | meaning |
|---|---|
| RF | pull fanout |
| μ = k/N, H | adversarial fraction; honest count |
| branching ≈ RF(1−μ) | honest out-degree driving the spread |

**Validity**: good graph ([`full_coverage.md`](full_coverage.md)).

## 3. Results — N = 20 000, μ = 0.2 (H = 16 000)

`sweep_m2_cost.py`, 40 graphs per RF, publisher-only injection:

| RF | full-coverage hops | mean-node hops | ln H / ln(RF(1−μ)) |
|---|---|---|---|
| 16 | 5.05 | 4.04 | 3.80 |
| 20 | 5.00 | 3.78 | 3.49 |
| 24 (δ-cheapest) | 4.75 | 3.60 | 3.28 |
| 25 | 4.60 | 3.58 | 3.23 |

At the operating point RF = 25: **4.6 hops to full coverage, 3.6
typical**.

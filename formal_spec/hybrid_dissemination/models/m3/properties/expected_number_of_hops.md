# M3 — expected number of hops (latency) to full coverage

**Verdict: SIMULATION ONLY** — scaling law ~ ln H / ln(RF(1−μ)); constants and
the straggler tail by simulation. Scripts (in `../scripts/`):
`sweep_m3_cost.py`, `sim_p06_depth.py` (depth percentiles).

## 1. Property

Hops until honest nodes receive one message — BFS depth on the sampled graph:
publisher at depth 0, its honest initiation targets at depth 1, every pull
relay costs 1. Reported: **full-coverage hops** (the last node; what
consensus timing cares about) and **mean-node hops** (a typical node).

## 2. Guiding law (asymptotic only)

The spread is breadth-first over the pull relay edges with effective
branching equal to a node's honest requester count:

$$\text{hops} \;\sim\; \frac{\ln H}{\ln(RF(1-\mu))} \;+\; O(1)\ \text{tail}.$$

The leading term matches the mean-node depth; the additive constant and the
last-straggler tail are obtained by simulation. **s is not a latency lever**:
initiation links add depth-1 roots but s = 1 vs 10 changes depth by well
under a hop (`sim_p06_depth.py`) — they buy ignition, not depth. RF is the
latency lever, exactly as it is the coverage and bandwidth lever.

| symbol | meaning |
|---|---|
| RF | pull fanout; s−1 = standing initiation links |
| μ = k/N, H | adversarial fraction; honest count |
| branching ≈ RF(1−μ) | honest out-degree driving the spread |

**Validity**: good graph ([`full_coverage.md`](full_coverage.md)); depths
measured over covering runs.

## 3. Results — N = 20 000, μ = 0.2 (H = 16 000), s = 8

`sweep_m3_cost.py`, 40 graphs per RF:

| RF | full-coverage hops | mean-node hops | ln H / ln(RF(1−μ)) |
|---|---|---|---|
| 8 | 6.95 | 5.01 | 5.21 |
| 12 | 5.92 | 4.31 | 4.28 |
| 13 | 5.45 | 4.20 | 4.13 |
| 16 | 5.00 | 3.88 | 3.80 |

At the δ = 10⁻⁴ operating point (RF = 12, s = 8): **5.9 hops to full
coverage, 4.3 typical** — at WAN per-hop latencies (~100–300 ms eager),
~0.6–1.8 s to full coverage. Announce-then-fetch (lazy push) multiplies
per-hop time (~1 extra RTT), not hop count.

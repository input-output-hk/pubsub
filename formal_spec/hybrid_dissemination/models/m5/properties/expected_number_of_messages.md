# M5 — expected number of messages (bandwidth) per dissemination

**Verdict: CLOSED FORM** — exact in expectation, validated to <0.03 %.
Script (in `../scripts/`): `sweep_m5_cost.py`.

## 1. Property

Total transmissions to fully disseminate one message. Rule: a node fires
once, on first receipt, relaying on every outgoing propagation edge — its own
k_out targets plus the nodes that in-picked it — skipping a resend back to
its arrival node (which almost never coincides: a node's out-edges and its
arrival edge come from different pick sets). Counted: honest→honest
transmissions (copies delivered to honest nodes, duplicates included;
adversary-directed sends excluded).

## 2. Guiding formula

Every honest→honest edge carries the message exactly once. The honest
digraph has H·k_in(1−μ) live in-pick edges and H·k_out(1−μ) live out-pick
edges:

$$\boxed{\;T \;\approx\; H\,(1-\mu)\,(k_{in}+k_{out}),\qquad
\frac{T}{H} \;\approx\; (k_{in}+k_{out})(1-\mu)\ \text{ copies per honest node.}\;}$$

Each honest node receives ≈ k_in(1−μ) copies over its own inbound links plus
≈ k_out(1−μ) copies from nodes that out-picked it. No deduplication is
possible — every live in-link delivers one copy; that redundancy is the
paid-for isolation resistance in both directions.

| symbol | meaning |
|---|---|
| k_in, k_out | inbound / outbound links each node opens |
| μ, H = (1−μ)N | adversarial fraction; honest count |
| T | honest→honest transmissions per message |

**Validity**: exact in expectation given full coverage
([`full_coverage.md`](full_coverage.md)); on a rare bad graph T scales with
the reached set.

## 3. Results — N = 20 000, μ = 0.2 (H = 16 000)

`sweep_m5_cost.py`, 40 graphs per cell:

| k_in | k_out | T (Monte-Carlo) | T (formula) | per honest node |
|---|---|---|---|---|
| 6 | 6 | 153 616 | 153 600 | 9.6 |
| 8 | 8 | 204 768 | 204 800 | 12.8 |
| 9 | 8 | 217 562 | 217 600 | 13.6 |
| 9 | 9 | 230 366 | 230 400 | 14.4 |
| 10 | 10 | 256 021 | 256 000 | 16.0 |
| 12 | 12 | 307 210 | 307 200 | 19.2 |

At the δ = 10⁻⁴ operating point (k_in, k_out) = (9, 8):
**T ≈ 217 600 (13.6 / honest node)**. Only the *sum* k_in + k_out enters the
cost — the split is a pure coverage/latency choice.

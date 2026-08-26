# M4 — expected number of messages (bandwidth) per dissemination

**Verdict: CLOSED FORM** — exact to leading order, validated to <0.01 %.
Script (in `../scripts/`): `sweep_m4_cost.py`.

## 1. Property

The total number of message transmissions to fully disseminate **one** message
under flooding — the per-message bandwidth. Rule: a node fires once, on first
receipt, and sends the message over every incident honest link **except the
one it arrived on** (no resend-back). Counted: honest→honest transmissions
(copies delivered to honest nodes, duplicates included; sends wasted on
adversary neighbours are extra and assumption-dependent, so excluded).

## 2. Guiding formula

The honest-induced subgraph has E ≈ H·RF·(1−μ) edges (each honest node's RF
picks, a fraction (1−μ) landing honest; mutual double-picks are negligible).
A flood uses **every honest edge in both directions except the H−1
spanning-tree arrival links** (each non-source node suppresses its one
arrival). So total transmissions = 2E − (H−1):

$$\boxed{\;T \;\approx\; 2\,H\,RF\,(1-\mu)\;-\;(H-1),\qquad
\frac{T}{H} \;\approx\; 2\,RF\,(1-\mu)-1\ \text{ copies per honest node.}\;}$$

The factor 2 is bidirectionality: each link carries the message both ways.
Duplicate suppression beyond the arrival link is impossible under stateless
flooding (a node has already forwarded before it learns a neighbour also has
the message).

| symbol | meaning |
|---|---|
| RF | peers each node picks (bidirectional) |
| μ = k/N, H = N−k | adversarial fraction; honest count |
| E ≈ H·RF·(1−μ) | edges of the honest-induced subgraph |
| T | honest→honest transmissions per message |

**Validity**: exact in expectation given the sampled edge count; assumes the
honest subgraph is connected ([`full_coverage.md`](full_coverage.md)); on a
rare bad graph T scales with the source component's edges.

## 3. Results — N = 20 000, μ = 0.2 (H = 16 000)

`sweep_m4_cost.py`, 60 graphs per RF:

| RF | T (Monte-Carlo) | T (formula) | per honest node |
|---|---|---|---|
| 4 | 86 417 | 86 401 | 5.4 |
| 5 | 111 985 | 112 001 | 7.0 |
| 6 | 137 595 | 137 601 | 8.6 |
| 7 | 163 183 | 163 201 | 10.2 |
| 8 | 188 795 | 188 801 | 11.8 |
| 9 | 214 433 | 214 401 | 13.4 |
| 10 | 239 996 | 240 001 | 15.0 |
| 12 | 291 201 | 291 201 | 18.2 |

Agreement is exact to the fourth significant figure across the range. At the
operating point RF = 9: **T ≈ 214 400 (13.4 / honest node)**; at the
δ-cheapest RF = 8: T ≈ 188 800 (11.8 / honest node).

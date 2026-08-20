# M2 — expected number of messages (bandwidth) per dissemination

**Verdict: CLOSED FORM** — exact in expectation, validated to <0.02 %. M2
proper (pure pull, G = 0). Script: `sweep_m2_cost.py` (in `../scripts/`).

## 1. Property

Total transmissions to fully disseminate one message over the sampled pull
graph. Rule: a node fires once, on first receipt, relaying to its honest
requesters (directed edges forwarder→requester); resend-back essentially never
fires (a node's forwarders and requesters are disjoint samples). Counted:
honest→honest transmissions (duplicates included).

## 2. Guiding formula

Every honest→honest pull edge carries the message exactly once — each honest
node receives one copy from each of its ≈ RF(1−μ) honest forwarders:

$$\boxed{\;T \;\approx\; H\,RF\,(1-\mu),\qquad
\frac{T}{H} \;\approx\; RF\,(1-\mu)\ \text{ copies per honest node.}\;}$$

Pull cannot deduplicate: a requester receives from all its live forwarders —
that redundancy is the paid-for eclipse resistance.

| symbol | meaning |
|---|---|
| RF | pull fanout |
| μ = k/N, H | adversarial fraction; honest count |
| T | honest→honest transmissions per message |

**Validity**: exact in expectation given full coverage
([`full_coverage.md`](full_coverage.md)).

## 3. Results — N = 20 000, μ = 0.2 (H = 16 000)

`sweep_m2_cost.py`, 40 graphs per RF:

| RF | T (Monte-Carlo) | T (formula) | per honest node |
|---|---|---|---|
| 16 | 204 798 | 204 800 | 12.8 |
| 20 | 255 974 | 256 000 | 16.0 |
| 24 (δ-cheapest) | 307 153 | 307 200 | 19.2 |
| 25 | 319 992 | 320 000 | 20.0 |

At the operating point RF = 25: **T ≈ 320 000 (20.0 / honest node)** —
+4.2 % over the δ-cheapest RF = 24 reference.

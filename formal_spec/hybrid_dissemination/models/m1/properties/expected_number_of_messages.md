# M1 — expected number of messages (bandwidth) per dissemination

**Verdict: CLOSED FORM** — exact in expectation, validated to <0.01 %. Script
(in `../scripts/`): `sweep_m1_cost.py`.

## 1. Property

Total transmissions to fully disseminate one message. Rule: a node fires once,
on first receipt, pushing to its F targets. Counted: honest→honest
transmissions (copies delivered to honest nodes, duplicates included;
adversary-directed sends excluded).

## 2. Guiding formula

Every reached honest node fires exactly once, pushing F copies of which a
fraction (1−μ) land honest; on a good graph all H honest nodes fire:

$$\boxed{\;T \;\approx\; H\,F\,(1-\mu),\qquad
\frac{T}{H} \;\approx\; F\,(1-\mu)\ \text{ copies per honest node.}\;}$$

Each honest node receives ≈ F(1−μ) duplicate copies — its honest in-degree.
No duplicate suppression is possible under blind push (a sender does not know
who already holds the message).

| symbol | meaning |
|---|---|
| F | push fanout |
| μ = k/N, H = N−k | adversarial fraction; honest count |
| T | honest→honest transmissions per message |

**Validity**: exact in expectation given full coverage
([`full_coverage.md`](full_coverage.md)).

## 3. Results — N = 20 000, μ = 0.2 (H = 16 000)

`sweep_m1_cost.py`, 40 graphs per F:

| F | T (Monte-Carlo) | T (formula) | per honest node |
|---|---|---|---|
| 12 | 153 606 | 153 600 | 9.6 |
| 16 | 204 765 | 204 800 | 12.8 |
| 20 | 255 959 | 256 000 | 16.0 |
| 24 | 307 202 | 307 200 | 19.2 |
| 25 | 319 974 | 320 000 | 20.0 |
| 28 | 358 408 | 358 400 | 22.4 |

At the operating point F = 25: **T ≈ 320 000 (20.0 / honest node)**; the
δ-cheapest F = 24 costs 307 200 (19.2).

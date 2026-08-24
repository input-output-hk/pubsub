# M3 — expected number of messages (bandwidth) per dissemination

**Verdict: CLOSED FORM** — exact in expectation, validated to <0.03 %.
Script: `sweep_m3_cost.py` (in `../scripts/`).

## 1. Property

Total transmissions to fully disseminate one message: the publisher's s−1
initiation sends plus the relay traffic over the sampled pull graph. Rule: a
node fires once, on first receipt, relaying to its honest requesters
(forwarder→requester edges); resend-back essentially never fires. Counted:
honest→honest transmissions (duplicates included).

## 2. Guiding formula

Every honest→honest pull edge carries the message exactly once (each honest
node receives one copy from each of its ≈ RF(1−μ) honest forwarders), plus
the publisher's (s−1)(1−μ) honest-landing initiation sends:

$$\boxed{\;T \;\approx\; H\,RF\,(1-\mu)\;+\;(s-1)(1-\mu),\qquad
\frac{T}{H} \;\approx\; RF\,(1-\mu)\ \text{ copies per honest node.}\;}$$

**Initiation links are bandwidth-free**: they carry only their owner's own
publications, so per message they cost s−1 sends against ~10⁵ relay
transmissions — only the *relay* fanout RF enters the per-node cost. Pull
cannot deduplicate: the RF(1−μ) duplicate copies per node are the paid-for
eclipse redundancy; announce-then-fetch confines the duplicated bytes to
~100 B announcements.

| symbol | meaning |
|---|---|
| RF | pull fanout; s−1 = standing initiation links per node |
| μ = k/N, H | adversarial fraction; honest count |
| T | honest→honest transmissions per message |

**Validity**: exact in expectation given full coverage
([`full_coverage.md`](full_coverage.md)).

## 3. Results — N = 20 000, μ = 0.2 (H = 16 000), s = 7

`sweep_m3_cost.py`, 40 graphs per RF:

| RF | T (Monte-Carlo) | T (formula) | per honest node |
|---|---|---|---|
| 8 | 102 416 | 102 405 | 6.4 |
| 12 | 153 569 | 153 605 | 9.6 |
| 13 | 166 428 | 166 405 | 10.4 |
| 16 | 204 749 | 204 805 | 12.8 |

At the operating point (RF = 13, s = 7): **T ≈ 166 400 (10.4 / honest
node)** — +8.3 % over the δ-cheapest point (12, 8) at 153 600
(9.6 / node), buying ~2× the P(bad) headroom and ~4× the churn margin
([`mu_shift_robustness.md`](mu_shift_robustness.md)).

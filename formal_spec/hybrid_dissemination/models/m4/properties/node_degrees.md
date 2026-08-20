# M4 — node degrees (standing links per node)

**Verdict: CLOSED FORM** — per-node distribution exact; the network maximum
by simulation. Script (in `../scripts/`): `sim_m4_degrees.py`.

## 1. Property

The degree of an honest node. M4's links are undirected, so every link is
simultaneously an in- and an out-edge; the degree splits into the **chosen**
side (own RF picks — deterministic, held even when the counterpart is
adversarial) and the **accepted** side (others' picks of the node — random).

Adversarial *inbound* link-opening is bounded only by admission policy
(resource plane, out of scope); accepted counts below are from honest peers.

## 2. Closed forms

- **chosen**: RF held, deterministically; honest (useful) part
  ~ Hypergeometric — mean RF(1−μ).
- **accepted (honest)**: ~ Binomial(H−1, RF/(N−1)) ≈ Poisson(RF(1−μ)).
- **honest degree** = chosen-honest + accepted — mean 2·RF(1−μ), the
  branching/bandwidth degree of the flood; network maximum is a
  balls-in-bins tail, ≈ mean + 4–5 sd at N = 20 000.
- **compliant total**: mean total degree is exactly **2·RF**.

| symbol | meaning |
|---|---|
| RF | peers each node picks (bidirectional) |
| μ = k/N, H = N−k | adversarial fraction; honest count |

## 3. Results — N = 20 000, μ = 0.2, RF = 9 (25 graphs)

| quantity | mean | sd | max observed |
|---|---|---|---|
| degree (in = out, honest) | 14.40 | 2.94 | 34 |

Matches the closed form to the shown precision. Mean total held ≈ 16.2
(honest network) / 18 (compliant); the busiest node holds ~34 links (~2.4×
the mean) — the provisioning number, not the average.

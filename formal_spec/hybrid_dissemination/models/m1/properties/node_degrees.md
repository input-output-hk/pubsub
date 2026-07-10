# M1 — node degrees (standing links per node)

**Verdict: CLOSED FORM** — per-node distributions exact; the network maximum
by simulation. Script (in `../scripts/`): `sim_m1_degrees.py`.

## 1. Property

The in- and out-degree of an honest node: how many links it holds, split into
the **chosen** side (its own picks — deterministic, held even when the
counterpart is adversarial and the link is dead) and the **accepted** side
(others' picks of it — random). In M1 the chosen links are the message-flow
**out**-edges (push targets); the accepted links are the **in**-edges.

Adversarial *inbound* link-opening is bounded only by admission policy
(resource plane, out of scope); accepted counts below are from honest peers.

## 2. Closed forms

- **out (chosen)**: F held, deterministically; of them honest (useful)
  ~ Hypergeometric — mean F(1−μ), sd √(F·μ(1−μ)).
- **in (accepted, honest)**: ~ Binomial(H−1, F/(N−1)) ≈ Poisson(F(1−μ)) —
  mean F(1−μ), sd ≈ √(F(1−μ)); the network-wide maximum is a balls-in-bins
  tail, ≈ mean + 4–5 sd at N = 20 000.
- **compliant total** (if all N nodes follow the protocol): every link has a
  chooser and an acceptor, so the mean total degree is exactly **2F**.

| symbol | meaning |
|---|---|
| F | push fanout (chosen out-links per node) |
| μ = k/N, H = N−k | adversarial fraction; honest count |

## 3. Results — N = 20 000, μ = 0.2, F = 24 (25 graphs)

| quantity | mean | sd | max observed |
|---|---|---|---|
| out (chosen, honest part) | 19.20 (of 24 held) | 1.96 | 24 |
| in (accepted, honest) | 19.20 | 4.38 | 41 |

All match the closed forms to the shown precision. Mean total held ≈ 43
(honest network) / 48 (compliant); the busiest node accepts ~41 in-links
(~2.1× the mean) — the provisioning number, not the average.

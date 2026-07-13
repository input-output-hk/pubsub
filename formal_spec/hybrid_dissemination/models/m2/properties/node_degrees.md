# M2 — node degrees (standing links per node)

**Verdict: CLOSED FORM** — per-node distributions exact; the network maximum
by simulation. Script (in `../scripts/`): `sim_m2_degrees.py`.

## 1. Property

The in- and out-degree of an honest node: how many links it holds, split into
the **chosen** side (its own picks — deterministic, held even when the
counterpart is adversarial and the link is dead) and the **accepted** side
(others' picks of it — random). In M2 the chosen links are the message-flow
**in**-edges (the forwarders it pulls from); the accepted links are the
**out**-edges (the requesters it serves).

Adversarial *inbound* requests are bounded only by admission policy (the
serving-slot / resource plane, out of scope); accepted counts below are from
honest peers.

## 2. Closed forms

- **in (chosen)**: RF held, deterministically; of them honest (useful)
  ~ Hypergeometric — mean RF(1−μ), sd √(RF·μ(1−μ)).
- **out (accepted, honest requesters)**: ~ Binomial(H−1, RF/(N−1)) ≈
  Poisson(RF(1−μ)) — mean RF(1−μ); the network-wide maximum is a
  balls-in-bins tail, ≈ mean + 4–5 sd at N = 20 000 — the serving-load
  concentration a forwarder must provision for.
- **compliant total**: mean total degree is exactly **2·RF**.

| symbol | meaning |
|---|---|
| RF | pull fanout (chosen in-links per node) |
| μ = k/N, H = N−k | adversarial fraction; honest count |

## 3. Results — N = 20 000, μ = 0.2, RF = 24 (25 graphs)

| quantity | mean | sd | max observed |
|---|---|---|---|
| in (chosen, honest part) | 19.20 (of 24 held) | 1.96 | 24 |
| out (accepted, honest) | 19.20 | 4.38 | 41 |

All match the closed forms to the shown precision. Mean total held ≈ 43
(honest network) / 48 (compliant); the busiest forwarder serves ~41
requesters (~2.1× the mean) — the provisioning number, not the average.

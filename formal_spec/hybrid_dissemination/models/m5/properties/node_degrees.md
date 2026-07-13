# M5 — node degrees (standing links per node)

**Verdict: CLOSED FORM** — per-node distributions exact; the network maximum
by simulation. Script (in `../scripts/`): `sim_m5_degrees.py`.

## 1. Property

The in- and out-degree of an honest node. Both directions mix a **chosen**
side (own picks — deterministic, held even when the counterpart is
adversarial) and an **accepted** side (others' picks — random):

- **in** = own k_in picks (chosen) + others' out-picks hitting the node
  (accepted);
- **out** = own k_out picks (chosen) + others' in-picks hitting the node,
  i.e. requesters it must serve (accepted).

Adversarial *inbound* link-opening is bounded only by admission policy
(resource plane, out of scope); accepted counts below are from honest peers.

## 2. Closed forms

- **chosen**: k_in + k_out held, deterministically; honest (useful) parts
  ~ Hypergeometric with means k_in(1−μ), k_out(1−μ).
- **accepted (honest)**: into the in-side ~ Binomial(H−1, k_out/(N−1)); into
  the out-side ~ Binomial(H−1, k_in/(N−1)). Honest mean in-degree = mean
  out-degree = (k_in+k_out)(1−μ); network maxima are balls-in-bins tails,
  ≈ mean + 4–5 sd at N = 20 000.
- **compliant total**: mean total degree is exactly **2·(k_in + k_out)** —
  all of it relay-carrying (unlike M3's initiation links, every M5 link
  transports every message it can).

| symbol | meaning |
|---|---|
| k_in, k_out | inbound / outbound links each node opens |
| μ, H = (1−μ)N | adversarial fraction; honest count |

## 3. Results — N = 20 000, μ = 0.2, (k_in, k_out) = (9, 8) (25 graphs)

| quantity | mean | sd | max observed |
|---|---|---|---|
| in (chosen + accepted, honest) | 13.60 | 2.80 | 33 |
| out (chosen + accepted, honest) | 13.60 | 2.91 | 31 |

Both match the closed forms to the shown precision. Mean total held ≈ 30.6
(honest network) / 34 (compliant); the busiest node holds ~33 in-links —
the provisioning number, not the average.

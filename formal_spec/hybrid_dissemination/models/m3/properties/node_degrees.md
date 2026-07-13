# M3 — node degrees (standing links per node)

**Verdict: CLOSED FORM** — per-node distributions exact; the network maximum
by simulation. Script (in `../scripts/`): `sim_m3_degrees.py`.

## 1. Property

The in- and out-degree of an honest node, with M3's two link kinds. Each has
a **chosen** side (own picks — deterministic, held even when the counterpart
is adversarial) and an **accepted** side (others' picks — random):

- **in** = RF forwarders (chosen) + initiation links of others targeting the
  node (accepted);
- **out** = requesters that picked the node as forwarder (accepted) + its
  s−1 initiation links (chosen).

Adversarial *inbound* link-opening (requests, initiation targets) is bounded
only by admission policy (resource plane, out of scope); accepted counts
below are from honest peers.

## 2. Closed forms

- **chosen**: RF in-links + (s−1) out-links held, deterministically; honest
  (useful) parts ~ Hypergeometric with means RF(1−μ) and (s−1)(1−μ).
- **accepted (honest)**: requesters ~ Binomial(H−1, RF/(N−1)) ≈
  Poisson(RF(1−μ)); incoming initiation ~ Binomial(H−1, (s−1)/(N−1)) ≈
  Poisson((s−1)(1−μ)). Network-wide maxima are balls-in-bins tails,
  ≈ mean + 4–5 sd at N = 20 000.
- **compliant total**: mean total degree is exactly **2·(RF + s−1)**.
- Mean in-degree = mean out-degree = (RF + s−1)(1−μ) in the honest network —
  but note only the RF-side links carry relay traffic
  ([`expected_number_of_messages.md`](expected_number_of_messages.md));
  initiation links are held state, not bandwidth.

| symbol | meaning |
|---|---|
| RF | pull fanout; s−1 = standing initiation links per node |
| μ = k/N, H = N−k | adversarial fraction; honest count |

## 3. Results — N = 20 000, μ = 0.2, (RF, s) = (12, 8) (25 graphs)

| quantity | mean | sd | max observed |
|---|---|---|---|
| in: forwarders (chosen, honest part) | 9.60 (of 12 held) | 1.38 | 12 |
| in: initiation (accepted, honest) | 5.60 | 2.37 | 19 |
| out: requesters (accepted, honest) | 9.60 | 3.10 | 27 |
| out: initiation (chosen, honest part) | 5.60 (of 7 held) | 1.06 | 7 |

All match the closed forms to the shown precision. Mean total held ≈ 34
(honest network) / 38 (compliant); the busiest node accepts ~36 links
(requesters + incoming initiation) — the provisioning number.

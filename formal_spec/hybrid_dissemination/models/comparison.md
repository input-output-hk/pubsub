# Model comparison — bandwidth and latency at P(bad) ≤ 10⁻⁴

Comparison of the five dissemination models at the shared operating point:

- **N = 20 000, μ = 0.2** (H = 16 000 honest, 4 000 silent adversaries);
- **full-coverage criterion** ([README](README.md)): a sampled graph is good
  iff every message of every honest publisher reaches all other honest nodes —
  only standing per-epoch structure counts;
- **P(bad graph) ≤ 10⁻⁴ per epoch**, each model at its cheapest parameters
  meeting the target;
- counting conventions as everywhere in this folder: fire-once relaying, no
  resend on the arrival link, transmissions = honest→honest copies
  (duplicates included).

All values below are **measured** (40–200 graphs per cell) by each model's
`scripts/`; the coverage laws behind the parameter choices are Monte-Carlo
validated in each model's [`full_coverage.md`](m1/properties/full_coverage.md).

## 1. Best parameters per model

| model | mechanism | best parameters | why these | P(bad) |
|---|---|---|---|---|
| [M1](m1/properties/README.md) | push | F = 24 | smallest F (F = 23 → 1.6×10⁻⁴) | 7.3×10⁻⁵ |
| [M2](m2/properties/README.md) | pull | RF = 24 | smallest RF (RF = 23 → 1.6×10⁻⁴) | 7.3×10⁻⁵ |
| [M3](m3/properties/README.md) | pull + initiation links | RF = 12, s = 8 | bandwidth-minimal split of the smallest budget RF+(s−1) = 19 | 7.8×10⁻⁵ |
| [M4](m4/properties/README.md) | undirected flood | RF = 8 | smallest RF (RF = 7 → 7.5×10⁻⁴) | 6.8×10⁻⁵ |
| [M5](m5/properties/README.md) | directed k_in/k_out | (k_in, k_out) = (9, 8) | most-balanced split of the smallest budget k_in+k_out = 17 | 4.4×10⁻⁵ |

## 2. The comparison

| model | parameters | msgs / message | copies / honest node | hops (full) | hops (mean) |
|---|---|---|---|---|---|
| **M3** | RF = 12, s = 8 | **153 570** | **9.6** | 5.9 | 4.3 |
| M4 | RF = 8 | 188 795 | 11.8 | 5.1 | 4.1 |
| M5 | (9, 8) | 217 562 | 13.6 | 5.0 | 3.9 |
| M1 | F = 24 | 307 202 | 19.2 | 5.0 | 3.6 |
| M2 | RF = 24 | 307 153 | 19.2 | **4.8** | **3.6** |

**Bandwidth: M3 wins decisively** — 19 % below M4, 29 % below M5, half of
M1/M2. **Latency: M2 wins, marginally** — the whole field spans only ~1.2
hops (4.8–5.9 full coverage; ~0.1–0.4 s at WAN per-hop times of 100–300 ms),
while bandwidth spans 2×.

## 3. Node degrees (standing links per node)

From each model's [`node_degrees.md`](m3/properties/node_degrees.md): the
mean total degree under protocol-compliant link opening is exactly 2× the
nominal budget (every link has a chooser and an acceptor); the maximum is a
balls-in-bins tail over the accepted side (measured, 25 graphs):

| model | chosen (held, det.) | honest in / out (mean) | max observed | compliant total (mean) |
|---|---|---|---|---|
| **M4** | 8 | 12.8 / 12.8 (same links) | 29 | **16** |
| M5 | 9 in + 8 out | 13.6 / 13.6 | 33 | 34 |
| M3 | 12 in + 7 out | 15.2 / 15.2 | ~36 accepted | 38 |
| M1 | 24 out | 19.2 / 19.2 | 41 | 48 |
| M2 | 24 in | 19.2 / 19.2 | 41 | 48 |

**On this axis the ordering flips: M4 wins decisively** — 16 links per node
vs M3's 38 (2.4×) and M1/M2's 48 (3×), with the smallest worst-case node
(29 vs 33–41). Note M3's degree exceeds its bandwidth: 14 of its 38 links
(the initiation kind) carry only their owner's publications — cheap in
traffic, but still held state, connection slots, and churn surface. In M4
and M5 every held link also carries relay traffic.

## 4. Degradation under μ-shift (frozen parameters)

From each model's
[`mu_shift_robustness.md`](m3/properties/mu_shift_robustness.md): the
operating points frozen, the effective adversarial fraction swept upward
(law-read, MC-validated at elevated μ). Reported: the **budget** (largest
μ_eff keeping P(bad) ≤ 10⁻⁴; churn reading p_max = Δμ/(1−μ)) and the
**collapse point** (P(bad) = ½):

| model | parameters | budget μ_eff (Δμ) | churn p_max | collapse μ_eff |
|---|---|---|---|---|
| M5 | (9, 8) | 0.217 (+0.017) | ~2.2 % | 0.49 |
| M1 | F = 24 | 0.214 (+0.014) | ~1.8 % | **0.61** |
| M2 | RF = 24 | 0.214 (+0.014) | ~1.7 % | **0.61** |
| M4 | RF = 8 | 0.209 (+0.009) | ~1.1 % | 0.50 |
| **M3** | (12, 8) | **0.204 (+0.004)** | **~0.5 %** | 0.44 |

**The robustness ordering is roughly the bandwidth ordering reversed: M3,
the bandwidth winner, is the most μ-brittle** — its μ^RF in-term has
log-sensitivity RF/μ = 60 vs 24 for M1/M2's exponential terms. M5's
top budget is mostly margin (its cheapest integer point lands 2.3× under
δ), not structure (sensitivity ≈ 50). M1/M2 degrade most gracefully
(collapse ≈ 0.61) — a cushion bought by their 2× bandwidth. On the live
frontier: **M4 tolerates ~2× more shift than M3** (~1.1 % vs ~0.5 % churn)
before leaving the target.

## 5. Bottom line

At P(bad) ≤ 10⁻⁴, N = 20 000, μ = 0.2: **M3 (RF = 12, s = 8) is the most
efficient model in bandwidth** — cheapest by 19–50 %, within ~1 hop
(~0.1–0.4 s) of the fastest, and continuously tunable toward the latency
corner without leaving the target. **M4 (RF = 8) is the most efficient in
per-node state** — 2.4× fewer standing links than M3 with a single mechanism
and one link type — at ~23 % more bandwidth and near-identical latency. The
practical choice is M3 if bandwidth is the binding resource, M4 if
connection count / simplicity is. Of the rest, M1 is weakly dominated by M2;
M2's marginal latency win (0.2 hops) costs 2× the bandwidth and 3× the
standing links of the leaders; M5 is best on no measured axis.

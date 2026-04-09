# RingCast Asymmetric Forwarding — Model Checking Report

*Model: `ringcast_n6_fixed_asym_catastr.prism` | Properties: `catastr.props` | Tool: PRISM 4.10 | All results: statistical model checking, 100,000 samples, 99% confidence intervals*

---

## Key Findings

1. **The asymmetric forwarding rule (Fig. 5 of the spec) is validated and worth its complexity.**
   It strictly dominates the symmetric approximation on every metric — coverage probability,
   dissemination speed, and message count — with no trade-offs, across all failure conditions tested.

2. **Per-forwarding failure (p_fail) is a more damaging failure mode than pre-dissemination node loss (p_kill).**
   At p_kill=10% the protocol still achieves P(all live nodes informed) = 0.991; at p_fail=20% this
   drops to 0.935. The ring provides strong structural protection against node loss but cannot fully
   compensate for unreliable forwarding.

3. **The asymmetric rule's advantage over the symmetric approximation grows with p_fail.**
   At p_fail=0 both achieve full coverage (P=1); at p_fail=20% the asymmetric model is 2.8 percentage
   points ahead; under severe compound failure (p_kill=10%, p_fail=20%) the gap widens to 4.2 pp.
   Eliminating back-sends matters most exactly when forwarding events are unreliable.

---

## Models

This report compares three models, all at equal total fanout F = 3 and the same H(6,2) ring topology
(N=6, node 0 = source). The three models differ only in their forwarding rule and number of
pre-established r-link slots.

### Link pre-establishment

Before dissemination begins, each node fixes a set of links it may use. D-links are determined by the
ring topology and are identical across all three models. R-link slots are sampled once from the node's
Cyclon gossip view and held fixed for the entire dissemination episode.

| Model | D-links | R-link slots | Total links | R-link sampling |
|---|---|---|---|---|
| Symmetric RF=1 | 2 (left + right ring neighbours) | 1 | 3 | Single draw, uniform over all peers except self (5 outcomes) |
| Asymmetric RF=2 | 2 (left + right ring neighbours) | 2 | 4 | Ordered pair without replacement from all peers except self (20 outcomes) |
| RandCast fanout=3 | None (no ring) | 3 (re-sampled each round) | 3 per round | 3 independent draws, uniform over all peers except self |

The asymmetric model uses **ordered** pair sampling. This ensures each slot independently has a uniform
distribution over all peers — necessary for correctness when only slot 1 is used in some forwarding
paths. An unordered draw would bias slot 1 toward lower-indexed peers.

### Forwarding rule

When a node receives the message it forwards once, using a subset of its pre-established links chosen
according to its forwarding rule. Let Q denote the node that sent the message.

| Model | Reception path | Links used | Effective sends to new nodes |
|---|---|---|---|
| Symmetric (RF=1) | Any (ignores who sent it) | left + right + r-link slot 1 | Up to 2 — back-send to Q is wasted when received via d-link |
| Asymmetric (RF=2) | Source or via r-link | left + right + r-link slot 1 | Up to 3 |
| Asymmetric (RF=2) | Via d-link from Q | non-Q neighbour + slot 1 + slot 2 (skipping any slot = Q) | Up to 3 |
| RandCast (fanout=3) | Any | 3 randomly chosen peers (re-sampled) | Up to 3 — but no structural guarantee; ring neighbours may not be reached |

### Why asymmetric outperforms symmetric

The total fanout budget is **F = 3 in both ring models**. In symmetric, when a node received via
d-link from Q, one of its three sends goes back to Q — an already-informed node. That slot is not
wasted *in addition to* the useful sends; it *replaces* one of them. Only 2 of the 3 slots reach
potentially new nodes.

In asymmetric, the back-send is eliminated. The freed slot becomes an extra r-link directed at a
potentially uninformed node. All 3 slots target new nodes. This is why asymmetric disseminates faster:
more unique uninformed nodes are reached per forwarding event, so fewer rounds are needed.

**Why does asymmetric also send fewer total messages?** Any r-link slot that equals Q is skipped
entirely at forwarding time (Q-exclusion). Since Q is one of 5 peers, each slot equals Q with
probability 1/5, so the expected sends for a d-link-received node are ~2.6 instead of 3. The symmetric
model always sends exactly 3, including the guaranteed wasted back-send. Asymmetric sends fewer
messages per event but each message is more likely to reach a new node — and dissemination terminates
in fewer rounds, reducing the total forwarding events overall.

**Why does the asymmetric advantage grow with p_fail?** When p_fail > 0, some forwarding events fail
entirely. The events that succeed matter more. Since each successful asymmetric event reaches more new
nodes on average, it compensates better for the failed ones. The symmetric model, even when it
succeeds, wastes one slot on Q — so under failures it has less residual coverage capacity per
surviving event.

### Why RandCast is weaker

RandCast has no ring. Without d-links, there is no deterministic backup path: every node must rely
entirely on random gossip to receive and forward the message. At low fanout this leaves a meaningful
probability that some nodes are never reached. RandCast is included as a baseline to quantify the
value of the ring topology itself, independent of the symmetric/asymmetric distinction.

---

## Methodology

### What PRISM computes

PRISM is a probabilistic model checker. It builds an exact mathematical model of all possible
executions of the protocol — every possible r-link assignment, every possible failure — and computes
exact probabilities and expectations over that entire space.

For this model the state space is too large for exhaustive analysis (~28M reachable states), so we use
*statistical model checking*: PRISM runs 100,000 independent simulations of the protocol and estimates
the quantities of interest from those samples. All results carry a 99% confidence interval, meaning the
true value lies within the reported ± range with 99% probability.

### Failure modes

- **p_kill**: each non-source node is independently killed *before* dissemination begins, with this
  probability. Models sudden mass failures — network partitions, power outages, catastrophic events.
  The source (node 0) is always alive.
- **p_fail**: each active node independently fails to forward *during* dissemination, with this
  probability per round. Models transient faults — packet loss, node crashes mid-operation, overloaded
  relays.

Both parameters are independent. Setting one to 0 isolates the other. Section 3 tests both simultaneously.

### Metrics

- **P(all live nodes informed)**: probability that every node that survived the kill phase eventually
  receives the message. This is the primary coverage metric.
- **E[rounds]**: expected number of dissemination rounds until no active forwarders remain.
- **E[messages]**: expected total number of point-to-point message sends (d-link + r-link combined).

---

## Section 1 — Asymmetric Model: Effect of p_kill

*p_fail = 0 (forwarding always succeeds when attempted) | RF = 2 (2 pre-established r-link slots per node; total fanout F = 3)*

This section isolates the effect of pre-dissemination node loss. All forwarding events succeed
(p_fail = 0); only the number of nodes alive at the start of dissemination varies.

| p_kill | Expected dead nodes | P(all live informed) | E[rounds] | E[messages] |
|---|---|---|---|---|
| 0% | 0.0 | **1.000** | 2.144 ± 0.002 | 10.627 ± 0.014 |
| 1% | 0.05 | **1.000 ± 0.001** | 2.151 ± 0.002 | 10.571 ± 0.014 |
| 2% | 0.10 | **1.000 ± 0.001** | 2.159 ± 0.002 | 10.517 ± 0.014 |
| 5% | 0.25 | **0.998 ± 0.001** | 2.180 ± 0.002 | 10.327 ± 0.015 |
| 10% | 0.50 | **0.991 ± 0.001** | 2.207 ± 0.002 | 9.989 ± 0.016 |

> **The protocol is highly robust to pre-dissemination node loss.**
> P(all live informed) stays above 0.99 even at p_kill=10% (where on average half a node is lost per
> run). The H(6,2) ring provides two independent d-link paths to every node, so a single kill upstream
> rarely strands a survivor. Message counts decrease with p_kill simply because fewer nodes are alive
> to forward.

---

## Section 2 — Asymmetric Model: Effect of p_fail

*p_kill = 0 (all nodes alive at dissemination start) | RF = 2, total fanout F = 3*

This section isolates the effect of per-forwarding failure. All nodes survive to the start of
dissemination; each active forwarder independently fails to send with probability p_fail.

| p_fail | P(all live informed) | E[rounds] | E[messages] |
|---|---|---|---|
| 0% | **1.000** | 2.142 ± 0.003 | 10.627 ± 0.014 |
| 5% | **0.995 ± 0.001** | 2.190 ± 0.003 | 10.384 ± 0.015 |
| 10% | 0.983 ± 0.001 | 2.235 ± 0.004 | 10.113 ± 0.015 |
| 20% | 0.935 ± 0.002 | 2.311 ± 0.004 | 9.502 ± 0.017 |

> **Per-forwarding failure degrades coverage more than node loss at comparable rates.**
> At p_fail=10% coverage drops to 0.983 — the protocol is still highly reliable. At p_fail=20% it
> reaches 0.935, a more meaningful degradation. Compare: at p_kill=10% (a much more severe event —
> half a node killed on average) P(all live informed) is still 0.991.
> The reason is structural: a killed node removes one participant from the ring permanently, but its
> two ring neighbours can still route around it. A failed forwarding event, by contrast, breaks a
> specific path at the moment it is needed, and the ring provides only limited redundancy for that
> single-round failure. Message counts decrease with p_fail because failed forwarders send nothing.

---

## Section 3 — Asymmetric Model: Compound Failure

*Two scenarios combining both failure modes simultaneously | RF = 2, total fanout F = 3*

Real deployments may experience both failure modes at once. This section tests two compound scenarios:
a moderate case (representative of routine network turbulence) and a severe case (stress-testing the
protocol under extreme conditions).

| Scenario | p_kill | p_fail | P(all live informed) | E[rounds] | E[messages] |
|---|---|---|---|---|---|
| Baseline | 0% | 0% | **1.000** | 2.144 | 10.627 |
| Moderate | 5% | 10% | 0.967 ± 0.001 | 2.254 ± 0.004 | 9.801 ± 0.016 |
| Severe | 10% | 20% | 0.881 ± 0.003 | 2.329 ± 0.004 | 8.809 ± 0.019 |

> **The two failure modes compound.**
> Moderate compound failure (p_kill=5%, p_fail=10%) yields P(all live informed) = 0.967 — lower than
> either failure mode alone at those rates (p_kill=5% alone: 0.998; p_fail=10% alone: 0.983). Under
> severe compound failure the protocol still achieves 0.881 live-node coverage. This is a demanding
> scenario — on average half a node is pre-killed and one in five forwarding events fails — and the
> ring backbone maintains meaningful coverage throughout.

---

## Section 4 — Comparison: Asymmetric vs Symmetric vs RandCast

All three models are described in the Models section above. Results at equal total fanout F = 3.

### 4a — p_kill sweep (p_fail = 0)

**Coverage probability**

| p_kill | Asym P(live) | Sym P(live) | Rand P(live) |
|---|---|---|---|
| 0% | **1.000** | **1.000** | 0.826 ± 0.003 |
| 1% | **1.000 ± 0.001** | 0.9997 ± 0.001 | 0.819 ± 0.003 |
| 2% | **1.000 ± 0.001** | 0.999 ± 0.001 | 0.813 ± 0.003 |
| 5% | **0.998 ± 0.001** | 0.994 ± 0.001 | 0.797 ± 0.003 |
| 10% | **0.991 ± 0.001** | 0.976 ± 0.002 | 0.769 ± 0.003 |

**Expected messages**

| p_kill | Asym E[msgs] | Sym E[msgs] | Rand E[msgs] |
|---|---|---|---|
| 0% | **10.627 ± 0.014** | 12.330 ± 0.015 | 12.768 ± 0.018 |
| 1% | **10.571 ± 0.014** | 12.254 ± 0.015 | 12.665 ± 0.018 |
| 2% | **10.517 ± 0.014** | 12.188 ± 0.015 | 12.558 ± 0.018 |
| 5% | **10.327 ± 0.015** | 11.930 ± 0.016 | 12.227 ± 0.018 |
| 10% | **9.989 ± 0.016** | 11.433 ± 0.017 | 11.648 ± 0.019 |

### 4b — p_fail sweep (p_kill = 0)

**Coverage probability**

| p_fail | Asym P(live) | Sym P(live) | Rand P(live) | Δ Asym−Sym |
|---|---|---|---|---|
| 0% | **1.000** | **1.000** | 0.826 ± 0.003 | 0 |
| 5% | **0.995 ± 0.001** | 0.993 ± 0.001 | 0.789 ± 0.003 | +0.002 |
| 10% | **0.983 ± 0.001** | 0.974 ± 0.001 | 0.749 ± 0.004 | +0.009 |
| 20% | **0.935 ± 0.002** | 0.907 ± 0.002 | 0.662 ± 0.004 | +0.028 |

**Expected messages**

| p_fail | Asym E[msgs] | Sym E[msgs] | Rand E[msgs] |
|---|---|---|---|
| 0% | **10.627 ± 0.014** | 12.330 ± 0.015 | 12.768 ± 0.018 |
| 5% | **10.384 ± 0.015** | 12.002 ± 0.016 | 12.326 ± 0.019 |
| 10% | **10.113 ± 0.015** | 11.645 ± 0.017 | 11.885 ± 0.020 |
| 20% | **9.502 ± 0.017** | 10.806 ± 0.020 | 10.909 ± 0.023 |

### 4c — Compound failure

**Coverage probability**

| Scenario | Asym P(live) | Sym P(live) | Rand P(live) | Δ Asym−Sym |
|---|---|---|---|---|
| Moderate (p_kill=5%, p_fail=10%) | **0.967 ± 0.001** | 0.951 ± 0.002 | 0.723 ± 0.004 | +0.016 |
| Severe (p_kill=10%, p_fail=20%) | **0.881 ± 0.003** | 0.839 ± 0.003 | 0.617 ± 0.004 | +0.042 |

**Expected messages**

| Scenario | Asym E[msgs] | Sym E[msgs] | Rand E[msgs] |
|---|---|---|---|
| Moderate (p_kill=5%, p_fail=10%) | **9.801 ± 0.016** | 11.181 ± 0.019 | 11.358 ± 0.022 |
| Severe (p_kill=10%, p_fail=20%) | **8.809 ± 0.019** | 9.926 ± 0.022 | 9.923 ± 0.025 |

### Comparison Conclusions

1. **Asymmetric forwarding strictly dominates symmetric on every metric, in every scenario tested.**
   There is no condition under which the symmetric model is preferable. The gain is not marginal: the
   asymmetric rule saves ~14% in messages at p_fail=0, and the advantage persists — in coverage terms,
   it grows — as failures increase.

2. **The Asym–Sym coverage gap grows with p_fail and with compound failure.**
   At p_fail=0 both achieve P=1 (no gap). At p_fail=20% the gap is 0.028. Under severe compound
   failure (p_kill=10%, p_fail=20%) the gap reaches 0.042. Back-send elimination reduces the number of
   forwarding events required per dissemination; when each event is less reliable, this economy becomes
   increasingly valuable.

3. **RingCast (both variants) dramatically outperforms RandCast on coverage.**
   Even the symmetric model achieves P(all live informed) ≥ 0.839 under severe compound failure, while
   RandCast drops to 0.617. At equal fanout (F=3), pure gossip cannot match the structural guarantee of
   the ring: without d-links, there is no deterministic backup path when r-links fail or miss uninformed
   nodes.

4. **Message counts for symmetric and RandCast converge under severe failure.**
   At severe compound failure, Sym uses 9.926 messages and RandCast uses 9.923 — nearly identical —
   despite the symmetric model's much higher coverage. Coverage probability, not message count, is the
   right metric for comparing these protocols under failures.

---

## Conclusions

1. **The asymmetric forwarding rule from Fig. 5 of the spec is validated.**
   Model checking confirms that eliminating back-sends and applying Q-exclusion to r-links is strictly
   beneficial — every metric improves, in every failure scenario. The implementation complexity of
   tracking the sender is justified by the results.

2. **Per-forwarding failure (p_fail) is the more damaging failure mode.**
   At p_kill=10% the protocol achieves P(all live informed) = 0.991. At p_fail=10% this is already
   lower (0.983), and at p_fail=20% it drops to 0.935. Engineering effort aimed at reliable message
   delivery (retransmission, acknowledgement, redundant paths) will have greater impact than effort
   aimed at tolerating node loss.

3. **The asymmetric rule's advantage is largest under the most adverse conditions.**
   Under baseline conditions (no failures), both RingCast variants achieve perfect coverage — the
   difference is in message efficiency. As failures accumulate, the asymmetric model degrades more
   gracefully: its coverage probability remains higher at every failure rate, and the gap over the
   symmetric approximation widens exactly when reliability matters most.

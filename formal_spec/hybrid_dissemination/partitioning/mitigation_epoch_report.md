# Subscriber-Chosen Delivery Agents: Mitigation Analysis

*Formal methods analysis | Tool: PRISM 4.10 probabilistic model checker*
*Companion to: `adversarial_partition_report.md`*

---

## Summary

The adversarial partitioning attack on RingCast H(N,2) requires only **2 controlled nodes**
to isolate a target subscriber with probability **e^{−RF} ≈ 13.5%** (RF=2), independent
of N. This report analyses a proposed countermeasure: instead of relying on the ring's
deterministic neighbour links for delivery, each subscriber **j** selects **RF delivery
agents** per epoch uniformly at random from the other N−1 subscribers.

**Key result:** Under this mitigation, the adversary's only viable strategy is a blind
Sybil flood — maintaining k adversarial nodes and hoping all RF of j's chosen agents are
adversarial. The per-epoch isolation probability is:

$$P(\text{isolated}) = \frac{\binom{k}{\text{RF}}}{\binom{N-1}{\text{RF}}}$$

This is verified exactly by a PRISM DTMC for N=10, RF=2, k=0..9. The result has two
properties absent from the original attack:

1. **The adversarial cost scales as O(N).** To maintain the original 13.5% floor, the
   adversary needs k ≈ 0.37N nodes — a constant fraction of the ring, compared to k=2.

2. **Security improves with N.** For fixed k, P(isolated) ∝ (k/N)^RF → 0 as N grows.
   In the original attack, growing the ring provides no protection at all.

---

## The Mitigation

### Mechanism

At the start of each epoch, subscriber j selects RF delivery agents uniformly at
random without replacement from the other N−1 subscribers on the topic. j communicates
this selection to each chosen agent privately (encrypted with the agent's public key).
When an event is disseminated on the topic, each of j's RF agents forwards the event
directly to j.

### Simplifying assumption

All nodes know each other and can establish direct communication when needed —
information assumed to be available from lower layers (Vicinity/Cyclon). This eliminates
routing as a variable and isolates the agent-selection mechanism as the object of study.

### Epoch randomness

The epoch corresponds to a Cardano epoch. The selection seed is derived from j's private
key combined with verifiable on-chain randomness, making the selection unpredictable
before the epoch begins and verifiable after.

### Effect on dissemination metrics

The rest of the ring's dissemination process is unchanged — all other nodes continue to
use the existing H(N,2) ring + r-link structure. The only addition is RF messages per
event from j's agents to j directly. Since the expected number of incoming r-links to
any node is already RF (each of N−1 nodes picks RF r-link targets uniformly, giving
E[in-degree] = RF), the mitigation replaces random expected incoming r-links with RF
guaranteed ones. Message count and round count are unchanged in expectation.

---

## Adversarial Strategies

### The only viable strategy: blind Sybil flood

The adversary maintains k Sybil nodes in the network. Since j's agent selection is
encrypted, the adversary does not know who j's agents are before the epoch begins.
Isolation requires all RF of j's chosen agents to be adversarial — a hypergeometric draw:

$$P(\text{all RF agents adversarial}) = \frac{\binom{k}{\text{RF}}}{\binom{N-1}{\text{RF}}}$$

### Why other strategies fail

**Within-epoch traffic analysis.** After the first dissemination event in epoch t, the
adversary observes who forwarded to j and learns the current agents. But this knowledge
arrives after the epoch's agent set is already fixed. Under the direct-communication
assumption, the adversary cannot retroactively insert a Sybil as one of j's pre-selected
agents. The information is only useful if the adversary can compromise an honest agent
mid-epoch — a significantly stronger capability than Sybil insertion.

**Cross-epoch prediction.** The adversary observes j's agents in epoch t. Epoch t+1
agents are computed from j's private key combined with the next epoch's on-chain
randomness, which is unpredictable. Knowing epoch t's agents gives zero information
about epoch t+1.

**Targeted ring positioning.** In the original ring attack, the adversary ground key pairs
to become j's immediate ring neighbours. In this mitigation, agent selection is uniform
across all N−1 nodes — there is no "adjacent" position to target. All nodes are
equally likely to be selected regardless of their ID or ring position.

---

## Formal Model

### Model

`mitigation_epoch.prism` is a PRISM 4.10 DTMC capturing the RF-step sequential selection
process. State: `(step, a)` where `step` is the number of agents selected so far and `a`
is the count of adversarial agents selected. At each step, one node is drawn uniformly
from the remaining N−1−step candidates.

Parameters (defaults, overridable via `-const`):

| Parameter | Default | Meaning |
|---|---|---|
| N | 10 | Total subscribers; j selects from N−1 |
| RF | 2 | Agents selected per epoch |
| k | (required) | Adversarial nodes among the N−1 candidates |

The guard on the selection transition restricts PRISM to reachable states — three
conditions: `a ≤ step` (adversarial picked ≤ total picked), `a ≤ k` (adversarial picked
≤ adversarial available), and `k−a ≤ N−1−step` (adversarial remaining ≤ pool remaining).
Without all three, PRISM's symbolic engine evaluates probability expressions outside
[0,1] at unreachable states.

**Labels:** `"isolated"` fires when `step=RF & a=RF` (all agents adversarial).

### Results: N=10, RF=2

`mitigation_sweep.sh` runs PRISM for each k and compares against the formula. All values
match exactly (tolerance 10^{−9}):

| k | P(isolated) — PRISM | C(k,2)/C(9,2) — formula |
|---|---|---|
| 0 | 0.000000 | 0 |
| 1 | 0.000000 | 0 |
| 2 | 0.027778 | 1/36 |
| 3 | 0.083333 | 3/36 = 1/12 |
| 4 | 0.166667 | 6/36 = 1/6 |
| 5 | 0.277778 | 10/36 |
| 6 | 0.416667 | 15/36 |
| 7 | 0.583333 | 21/36 |
| 8 | 0.777778 | 28/36 |
| 9 | 1.000000 | 36/36 |

PRISM verifies the formula exactly. The model state space is 6 states and 9 transitions
for RF=2 — trivially small.

---

## Analysis

### Per-epoch isolation probability

For large N, with k adversarial nodes out of N−1:

$$P(\text{isolated per epoch}) = \frac{\binom{k}{\text{RF}}}{\binom{N-1}{\text{RF}}}
\approx \left(\frac{k}{N}\right)^{\text{RF}}$$

The probability is proportional to (k/N)^RF — the RF-th power of the adversarial
fraction. For fixed k, this tends to 0 as N grows. For fixed adversarial fraction k/N,
increasing RF reduces the probability exponentially.

### Adversarial budget required

To achieve isolation probability ε with RF=2 (large N):

$$k \approx \sqrt{\varepsilon} \cdot N$$

| Target ε | Required k (RF=2, large N) |
|---|---|
| 50% | ≈ 0.71 N |
| 13.5% (original attack floor) | ≈ 0.37 N |
| 5% | ≈ 0.22 N |
| 1% | ≈ 0.10 N |

At RF=3, the required k is larger still: k ≈ ε^{1/3} · N.

### Comparison with original attack

| Property | Original ring attack | Mitigation (per epoch) |
|---|---|---|
| Adversarial nodes required | **2** (constant) | **≈ 0.37 N** for ε = 13.5% |
| P(isolated), large N | e^{−RF} (constant) | (k/N)^RF (→ 0 for fixed k) |
| Dependence on N | **None** | **Improves with N** |
| Key grinding required | Yes — O(N) per node | **No** |
| Authorisation required | None | None |

The mitigation converts a constant-cost attack into one whose cost scales linearly with
N. There is no ring-position advantage to exploit, so no grinding is required — but the
adversary must maintain k = O(N) active Sybil subscriptions.

### Multi-epoch behavior

Agent sets are chosen independently each epoch (independent on-chain seeds). The
probability of isolating j in every epoch over T epochs is:

$$P(\text{isolated in all T epochs}) = \left(\frac{\binom{k}{\text{RF}}}{\binom{N-1}{\text{RF}}}\right)^T$$

This decays exponentially in T. For k = 0.37N (matching the original attack's 13.5%
floor per epoch), P(all T isolated) = 0.135^T. After 3 epochs, this falls below 0.25%.
An adversary seeking to persistently suppress j — across every event for multiple
epochs — needs a significantly higher budget or the attempt is likely to fail quickly.

---

## Conclusions

1. **The mitigation raises adversarial cost from O(1) to O(N).** The original ring attack
   requires 2 nodes and key grinding. The mitigation requires approximately 0.37N active
   Sybil subscriptions to maintain the same 13.5% per-epoch isolation probability.

2. **Security improves with N.** For a fixed adversarial budget k, P(isolated) ∝ (k/N)^RF
   decreases as more honest subscribers join. This is the inverse of the original attack,
   where adding subscribers provides no protection.

3. **The formula C(k,RF)/C(N-1,RF) is verified exact by PRISM** for N=10, RF=2, k=0..9.
   All 10 values match the analytical formula to within floating-point precision.

4. **Blind Sybil flood is the adversary's only viable strategy.** Traffic analysis,
   cross-epoch prediction, and targeted ring positioning are all ineffective under the
   model's assumptions (encrypted preferences, independent epoch randomness, uniform
   agent selection).

5. **The mitigation does not change dissemination metrics.** Message count and round
   count are unchanged in expectation; the RF agent messages replace the RF expected
   incoming r-links that j would have received anyway.

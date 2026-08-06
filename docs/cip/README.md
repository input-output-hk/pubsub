---
CIP: "?"
Title: Decentralised Pub/Sub Message Dissemination
Category: Network
Status: Proposed
Authors:
    - Will Wolff <william.wolff@iohk.io>
    - Ezequiel Postan <ezequiel.postan@iohk.io>
    - Denis Firsov <denis.firsov@gmail.com>
    - Jesus Diaz Vico <jesus.diaz.vico@gmail.com>
    - Dana Alibrandi <dalibrandi@gmail.com>
Implementors: []
Discussions:
    - Original PR: https://github.com/cardano-foundation/CIPs/pull/?
Created: 2026-07-21
License: CC-BY-4.0
---

<!-- Existing categories:

- Meta      | For meta-CIPs which typically serves another category or group of categories.
- Wallets   | For standardisation across wallets (hardware, full-node or light).
- Tokens    | About tokens (fungible or non-fungible) and minting policies in general.
- Metadata  | For proposals around metadata (on-chain or off-chain).
- Tools     | A broad category for ecosystem tools not falling into any other category.
- Plutus    | Changes or additions to Plutus
- Ledger    | For proposals regarding the Cardano ledger (including Reward Sharing Schemes)
- Consensus | For proposals affecting implementations of the Cardano Consensus layer and algorithms
- Network   | Specifications and implementations of Cardano's network protocols and applications

-->

## Abstract
<!-- A short (\~200 word) description of the proposed solution and the technical issue being addressed. -->

The Cardano ecosystem lacks a decentralised layer for messages that do not belong on the chain itself. Emergency alerts to stake pool operators, notifications from pools to their delegators, dApp and wallet messaging, and governance communication all rely on centralised infrastructure today. Their operators can censor, fabricate, or silently drop messages. This undermines the trust guarantees of the underlying blockchain. Existing peer-to-peer solutions such as GossipSub do not close this gap. Their security rests on a discovery layer with freely created identities, which leaves them vulnerable to Sybil-based eclipse attacks.

We propose a decentralised topic-based publish/subscribe protocol anchored on Cardano. The chain serves as the protocol's trust root. Nodes register on-chain, which makes identities verifiable and costly to mass-produce. Each epoch, verifiable randomness derives a fresh, degree-bounded dissemination topology over the registered nodes. No participant can choose or predict its position in the graph. Topics carry arbitrary application content. The chain anchors trust, not the payload. The protocol tolerates an adversary controlling a bounded fraction of nodes. With overwhelming probability, every message published by an honest node reaches all honest subscribers of its topic. The design is grounded in formal analysis and large-scale simulation of candidate topologies, balancing security, latency, and bandwidth.

## Motivation: Why is this CIP necessary?
<!-- A clear explanation that introduces the reason for a proposal, its use cases and stakeholders. If the CIP changes an established design then it must outline design issues that motivate a rework. For complex proposals, authors must write a Cardano Problem Statement (CPS) as defined in CIP-9999 and link to it as the `Motivation`. -->

## Specification
<!-- The technical specification should describe the proposed improvement in sufficient technical detail. In particular, it should provide enough information that an implementation can be performed solely on the basis of the design in the CIP. This is necessary to facilitate multiple, interoperable implementations. This must include how the CIP should be versioned, if not covered under an optional Versioning main heading. If a proposal defines structure of on-chain data it must include a CDDL schema in its specification.-->

## Rationale: How does this CIP achieve its goals?
<!-- The rationale fleshes out the specification by describing what motivated the design and what led to particular design decisions. It should describe alternate designs considered and related work. The rationale should provide evidence of consensus within the community and discuss significant objections or concerns raised during the discussion.

It must also explain how the proposal affects the backward compatibility of existing solutions when applicable. If the proposal responds to a CPS, the 'Rationale' section should explain how it addresses the CPS, and answer any questions that the CPS poses for potential solutions.
-->

### Evidence

This section states the metrics by which a dissemination design is judged, the candidate designs that were evaluated against them, the agreement between the closed-form analysis and independent simulation, and the operating point that follows. It closes with the limits of what this evidence establishes.

Two independent instruments produce the results below. The first is a set of closed-form coverage laws with accompanying Monte-Carlo simulators, developed from the protocol's abstract model. The second is a measurement framework that drives populations of the reference implementation's own node cores — the same state-transition function and message vocabulary the node runs — under a deterministic round-based scheduler. The two were developed separately and agree; that agreement, rather than either result alone, is the evidence offered here.

Every figure in this section is reproducible byte-for-byte from a tool commit and a master seed, at any degree of parallelism.[^reproduction]

#### Performance metrics

A dissemination design is characterised by the probability that it fails to deliver, what it costs to run at that probability, and how much degradation it absorbs before the probability changes. The metrics below are measured per epoch, since the guarantee is a property of the standing per-epoch topology rather than of any single message.

<!-- Table numbering: renumber if earlier sections introduce tables before this point. -->

| Category | Metric | Measurement |
| :--: | --- | --- |
| Coverage | Epoch failure probability, $p_\text{bad}$ | Probability that a drawn epoch topology fails to carry some honest publisher's messages to every honest subscriber |
| | Design target, $\delta$ | The value of $p_\text{bad}$ a configuration is sized to meet |
| Cost | Transmissions per publication, $m$ | Honest-to-honest message copies sent per published message, duplicates included |
| | Copies per honest node, $c$ | Copies of each published message received by an average honest node |
| | Standing links per node, $d$ and $\hat{d}$ | Connections held open for the whole epoch, mean and maximum, counting a node's own picks and the links others opened to it |
| Latency | Hops to full coverage, $h_\text{full}$ | Forwarding depth at which the last honest subscriber receives |
| | Mean first receipt, $h_\text{mean}$ | Forwarding depth at which a typical honest subscriber first receives |
| Resilience | Adversarial fraction, $\mu$ | Share of registered nodes that accept their links and forward nothing |
| | Churn budget, $p_\text{max}$ | Largest honest downtime fraction for which a deployed configuration still meets $\delta$ |

<em>Table N: Performance metrics</em>

**_Epoch failure probability._** A drawn topology is *good* if every message of every honest publisher can reach every other honest node over the standing links, and *bad* otherwise. This is deliberately an all-or-nothing property of the graph rather than an average over messages: a design that delivers reliably for most publishers but mutes one of them has failed for that publisher every time it publishes, for the whole epoch. Because it is a property of the draw, it is estimated by sampling topologies and counting.

$$p_\text{bad} = P(\text{some honest publisher cannot reach every honest subscriber over the epoch's standing links})$$

**_Design target._** Parameters are chosen as the cheapest configuration meeting a stated $\delta$. This proposal uses $\delta = 10^{-4}$ per epoch. The target is a parameter of the design rather than a property of it, and the appropriate value depends on epoch length: the same per-epoch probability yields very different long-run frequencies at hourly and multi-day epochs.

**_Transmissions per publication and copies per honest node._** The bandwidth cost, counted as honest-to-honest copies with duplicates included, since duplicate suppression happens on receipt and the copy has already crossed the network. The two are related by the honest population size $H$, so either may be quoted:

$$c = m / H$$

**_Standing links per node._** The state cost, distinct from bandwidth because a link may be held without carrying traffic. The maximum matters as well as the mean: connection slots are provisioned for the worst-case node, and the maximum grows with network size even where the mean does not.

**_Hops._** Latency in forwarding steps rather than wall-clock time, so the figure is independent of the deployment's per-hop latency. Both the tail and the typical case are reported, since a design may reach most subscribers quickly and the last one slowly.

**_Adversarial fraction._** As specified in [The adversary this proposal defends against](#the-adversary-this-proposal-defends-against): registered nodes that accept their allotted links and forward nothing.

**_Churn budget._** Honest downtime enters the analysis as a shift in the adversarial fraction, so the churn budget is the largest $p$ satisfying the design target at the shifted value:

$$p_\text{max} = \max \{\, p : p_\text{bad}(\mu + p(1-\mu)) \le \delta \,\}$$

Downtime relates to the departure rate and the epoch length by $p = 1 - e^{-\lambda_d T_\text{epoch}}$, which is what makes $p_\text{max}$ an upper bound on epoch length as well as a resilience figure.

#### Designs evaluated

Five dissemination designs were analysed against the metrics above. They differ in the direction links are opened, whether a link carries traffic in both directions, and whether a node has a dedicated means of seeding its own publications.

| Design | Mechanism | Tuning parameters |
| :--: | --- | --- |
| M1 | Push: each node forwards to $F$ randomly drawn targets | $F$ |
| M2 | Pull: each node draws $RF$ forwarders and receives from them | $RF$ |
| M3 | M2, plus $s-1$ standing initiation links carrying only their owner's own publications | $RF$, $s$ |
| M4 | Each node draws $RF$ peers; links are bidirectional and flood | $RF$ |
| M5 | Directed: each node opens $k_\text{in}$ inbound and $k_\text{out}$ outbound links | $k_\text{in}$, $k_\text{out}$ |

<em>Table N+1: Dissemination designs evaluated</em>

M1 and M2 are the single-mechanism boundaries of M5, which provides a consistency check: M5 at $k_\text{in} = 0$ must reproduce M1, and at $k_\text{out} = 0$ must reproduce M2.

<!-- TODO(evidence): the three results subsections below are scaffolded. Filling them
     requires, in order:
       1. a committed machine-readable results file, so tables and figures are
          generated from one source rather than transcribed;
       2. the churn sweep (experiment E13) at all five operating points, without
          which "Robustness" states law extrapolation rather than measurement;
       3. depth histograms at the five operating points, so the latency claim is a
          distribution rather than four means.
     Figures follow the CIP convention of committed SVGs under images/. -->

#### Agreement between analysis and simulation

<!-- TODO(evidence): law-vs-measurement across all cells, both network sizes, three
     decades of p_bad. Report raw counts, an interval, and the aggregate agreement
     across cells — the per-cell comparison is weaker evidence than the absence of
     systematic bias over the whole set. Figure: measured against predicted, log-log,
     with the identity line. -->

#### Comparison at the design target

<!-- TODO(evidence): each design at its cheapest configuration meeting delta, compared
     on m, c, d, h_full, h_mean. Figure: cost against state, showing which designs are
     jointly non-dominated and which are beaten on every axis at once. -->

#### Robustness

<!-- TODO(evidence): p_bad against honest downtime at each design's operating point,
     and the resulting p_max. Blocked on E13; until it runs, this reads as law
     extrapolation and must say so. -->

#### Limits of this evidence

The following are stated so that a reader can judge what the numbers above do and do not establish.

**The measured range and the operating point are not the same range.** Sampling resolves failure probabilities only as low as the number of trials allows. The configurations that meet the design target fail so rarely that measuring them directly is impractical, so the evidence for those configurations is the coverage laws, validated where measurement is feasible and extrapolated by roughly three orders of magnitude to the operating point. The extrapolation is over a regime the laws are expected to describe well — isolated-vertex defects dominate there — but it is an extrapolation.

**A second-order tail correction is unresolved.** The coverage laws account for isolated nodes exactly and small multi-node dead-end components only to leading order, so they are expected to be mildly optimistic in the deep tail. Independent samples disagree on the size of that correction, and none is large enough to settle it: distinguishing a ten-percent effect at these probabilities requires on the order of $10^5$ trials per configuration. Where a configuration's margin against the design target is smaller than this uncertainty, the margin should be read as approximate.

**The state axis is measured less precisely than the cost axis.** Transmission counts are reproduced between the two instruments to within a small fraction of a percent. Standing-link counts are not measured to comparable precision, and links that carry no propagation traffic are not captured in the measured degree distributions at all. Where a comparison turns on state rather than bandwidth, it rests on the weaker of the two axes.

**One adversarial fraction.** All results are at a single value of $\mu$. That value is an assumption about the deployment, not a measurement of it, and the designs do not degrade at equal rates as it varies.

**Correlated failure is out of scope.** Downtime is modelled as independent across nodes and epochs. Region outages and upgrade waves violate both assumptions, in the direction that makes the guarantee weaker, and are not quantified here.

### Trade-offs and Limitations

#### The adversary this proposal defends against

The protocol is analysed against an adversary controlling a bounded fraction **μ** of registered nodes, each of which is *silent*: it registers legitimately, accepts its allotted share of links, and then forwards nothing. This is deliberately the weakest adversary that still defeats delivery. A node that never emits a message cannot be distinguished from an honest node that has nothing to forward, so it is also the cheapest attack to mount and the hardest to observe. An eclipse attack against a specific subscriber reduces to this behaviour among that subscriber's upstream peers.

Not modelled, and out of scope for this proposal: an adversary that forwards selectively or forwards corrupted content, resource exhaustion and denial of service, and an adaptive adversary that re-registers between epochs in order to re-target a chosen victim.

Honest node churn is not a separate threat model. An honest node that is offline for an epoch is indistinguishable, to every other node, from a silent adversary: it holds its allotted links and forwards nothing. Independent honest downtime with per-epoch probability *p* therefore enters the analysis above as a shift in the adversarial fraction, from μ to μ + *p*(1−μ), and the same coverage results apply at the shifted value. What remains preliminary is the validation rather than the model — the shifted-μ prediction has not yet been checked against a simulation that marks nodes down, and correlated downtime such as upgrade waves or region outages is not captured by a single independent *p*.

#### Two classes of fault, with different guarantees

The protocol distinguishes faults that are attributable from faults that are not, and the boundary between them is not a matter of engineering effort. Accountability for the *presence* of an incorrect message and accountability for the *absence* of a message are formally different problems.[^accountable-liveness]

**Attributable faults** are evidenced by a message that was actually sent, and any recipient can verify them without cooperation from anyone else:

- content that is malformed under, or contradicts, the publisher's signature, checkable against the publisher's registered key;
- a message sent by a peer outside the connections permitted to it for the current epoch, checkable against the obligation graph, which any participant can derive from the on-chain registry together with the epoch's public randomness.

**Non-attributable faults** consist of the absence of messages. Attributing these is provably impossible without both a network that is more often synchronous than asynchronous and an honest majority among the parties able to attest.[^accountable-liveness] This proposal assumes neither. The dissemination analysis makes no timing assumption at all, and attestation here is inherently local: the only parties who can speak to whether a given relay forwarded a given message to a given subscriber are those two nodes. With two potential attesters there is no majority to appeal to, and a subscriber's entire upstream set can be adversarial even when the network-wide fraction μ is small — that case is precisely the residual failure probability quantified elsewhere in this Rationale.

Two consequences follow, and this proposal states them rather than working around them. The protocol does not claim to identify which node silenced a message. A registration deposit therefore cannot be made conditional on relaying behaviour, and this proposal specifies deposits as a Sybil-resistance cost rather than as a bond forfeitable for poor service.

#### What the protocol guarantees instead

Rather than punishing silence, the design bounds its duration and makes it observable.

**Bounded duration.** The dissemination topology is re-derived every epoch from fresh public randomness, so a subscriber receives an independently drawn set of upstream peers each epoch. Being surrounded entirely by adversarial peers in one epoch is already improbable; remaining so across successive epochs requires that improbable draw to repeat, and the probability falls geometrically in the number of epochs. Muting is therefore bounded in duration by the epoch length, with no evidence, accusation, or attribution required.

This guarantee carries three qualifications that must be stated plainly. First, shortening the epoch shortens each episode of muting but proportionally increases how often episodes begin, leaving total expected exposure approximately unchanged: the epoch length redistributes risk from rare long outages to frequent short ones, rather than reducing it. For time-critical topics that redistribution is nonetheless valuable, since a brief interruption is tolerable where a prolonged one is not. Second, the argument depends entirely on successive draws being independent, which requires that the randomness source resist grinding and that registration for an epoch close before that epoch's randomness is determined. Without both, an adversary can influence which nodes it is positioned to silence. Third, the independence the argument needs is independence of outcomes, not merely of topology draws. Whether a subscriber is muted depends both on the peers it draws and on whether those peers are live, and liveness is not redrawn each epoch. A correlated outage raises the effective adversarial fraction across consecutive epochs at once, so the geometric decay describes a network whose downtime is independent between epochs rather than one in the middle of an upgrade wave.

**Detectability.** A subscriber cannot establish that it is being silenced from the dissemination channel alone. If its upstream peers are entirely silent, no later messages arrive either, so there is no gap in the received sequence to observe and the situation is indistinguishable from a topic with no recent activity. Detection requires a reference that remains reachable *while* the subscriber is being silenced. Two mechanisms satisfy this:

- **On-chain position commitments.** A publisher periodically commits its current sequence position for a topic, together with a commitment to the messages published in that period. Any subscriber compares this against what it holds. Because the commitment is public and durable, it also supports later verification by third parties, which an in-network mechanism cannot provide.
- **An adjacent epoch's peer set.** Because each epoch's topology is drawn independently, the peers a subscriber holds in the neighbouring epoch — during the handover overlap, or immediately after rotation — constitute an independent sample that can be queried for each publisher's current position. This costs nothing on-chain, at the price of a detection delay of up to one epoch and no durable record.

The two compose: the peer-set mechanism gives cheap detection and recovery, while the on-chain mechanism additionally supports a cadence independent of the epoch length and leaves evidence that outlives the epoch.

**Recovery.** Messages are identified by the triple (topic, publisher, sequence number), so a subscriber that has established what it is missing can request precisely those messages once it holds honest upstream peers. Recovery therefore requires messages to be retained for at least the detection interval, which makes retention a protocol parameter rather than an implementation detail.

**Bounding duration is not a latency guarantee.** A message delivered after the next rotation is still late. Topics carrying urgent traffic must obtain redundancy within the epoch — publishing along several independent paths — rather than relying on rotation to repair a missed delivery.

### Open Questions

- Whether a deposit should decay in the absence of positively supplied evidence of participation, following the approach Ethereum's inactivity leak takes to liveness faults,[^accountable-liveness] or remain a static Sybil-resistance cost with detection used only for recovery. Deterrence requires a record a third party can check after the fact, which an in-network mechanism does not produce.
- The bounds on epoch length, which is constrained from both directions. From below by convergence: connection establishment must complete comfortably within an epoch, and the analytical results assume a converged standing topology, so the validity of the analysis itself constrains how short an epoch may be. From above by churn: unrepaired honest downtime accumulates over the epoch, so a longer epoch means a larger effective adversarial fraction at the moment the topology is judged. The width of the admissible window is therefore a property of the chosen operating point rather than a free parameter, and it narrows as that point is tuned for efficiency.
- Whether dissemination parameters should be selected for minimum cost at the target failure probability, or for tolerance to downtime at a small cost premium. The two criteria do not agree, and they can select different configurations at an identical link budget: a point sized to sit just inside the target is by construction the one with least room to absorb a shifted adversarial fraction.
- The cadence of on-chain position commitments against their cost, and whether topics carrying urgent traffic require a cadence finer than the epoch.
- Whether adding a partial-synchrony assumption is acceptable, given that the analysis presented here deliberately avoids one, and what it would buy.
- How many node identities a single trust anchor may derive, which bounds the residual Sybil surface that the deposit alone must price.

## Path to Active

### Acceptance Criteria
<!-- Describes what are the acceptance criteria whereby a proposal becomes 'Active' -->

<!-- For core categories (Ledger, Plutus, Network, Consensus) the following SHOULD be included:
- [ ] Implementation present within block producing nodes used by 80%+ of stake
-->

### Implementation Plan
<!-- A plan to meet those criteria or `N/A` if an implementation plan is not applicable. -->

<!--
OPTIONAL SECTIONS (see CIP-0001 > Document > Structure table for details):
These may appear here, between 'Path to Active' and 'Copyright', in any order
and at author/editor discretion. To use one, add it as an H2 below.

## Versioning            — if versioning is not addressed in Specification
## References            — external documents, prior art, related CIPs/CPSs
## Appendices            — supplementary material
## Acknowledgements      — contributors and discussion participants

Note: 'Open Questions' is a CPS-only section. Unresolved design questions in a
CIP belong in the Rationale section, e.g. as an '### Open Questions' subsection.
-->

## References

[^accountable-liveness]: Andrew Lewis-Pye, Joachim Neu, Tim Roughgarden and Luca Zanolini. *Accountable Liveness.* IACR ePrint Archive, Report 2025/693. <https://eprint.iacr.org/2025/693>. Establishes accountability for liveness violations as a distinct problem from accountability for safety violations, and proves it unattainable both in networks that are more often asynchronous than synchronous and under an adversarial majority — neither restriction applying to safety accountability. Also formalises the guarantees underlying Ethereum's inactivity-leak mechanism.

[^reproduction]: Reproducing the measurements. Each result is identified by a tool commit, a sweep configuration, and a master seed; those three reproduce the output files byte-for-byte, independently of how many runs execute in parallel. The configurations and the procedure are documented with the measurement framework. <!-- TODO(evidence): pin the commit and link the configuration directory once the results file is committed. -->

## Copyright
<!-- The CIP must be explicitly licensed under acceptable copyright terms. Uncomment the license you wish to use (delete the other one) and ensure it matches the License field in the header.

If AI/LLMs were used in the creation of the copyright text, the author may choose to include a disclaimer to describe their application within the proposal.
-->

This CIP is licensed under [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/legalcode).

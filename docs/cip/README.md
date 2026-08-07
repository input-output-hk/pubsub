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

The Cardano ecosystem lacks a decentralised layer for messages that must be trustworthy but do not belong on the chain itself. Emergency alerts to stake pool operators, notifications from pools to their delegators, dApp and wallet messaging, and governance communication all run on centralised infrastructure today, whose operators can censor, fabricate, or silently drop messages — so coordination around a Byzantine-fault-tolerant chain does not inherit its guarantees. Existing peer-to-peer solutions such as GossipSub do not close the gap: their resistance to eclipse rests on a discovery layer that admits freely created identities.

We propose a decentralised topic-based publish/subscribe protocol anchored on Cardano. The chain serves as the protocol's trust root. Nodes register on-chain, which makes identities verifiable and costly to mass-produce. Each epoch, verifiable on-chain randomness derives a fresh, degree-bounded dissemination topology that any participant can recompute but none can influence. Topics carry arbitrary application content: the chain anchors trust, not the payload. Against an adversary controlling a bounded fraction of nodes, the per-epoch probability that any honest publisher fails to reach every honest subscriber is a tunable design target. The design is grounded in formal analysis and simulation at deployment scale, cross-validated between independent implementations.

## Motivation: Why is this CIP necessary?
<!-- A clear explanation that introduces the reason for a proposal, its use cases and stakeholders. If the CIP changes an established design then it must outline design issues that motivate a rework. For complex proposals, authors must write a Cardano Problem Statement (CPS) as defined in CIP-9999 and link to it as the `Motivation`. -->

### The gap

Cardano does not run itself. Behind the protocol sits a network of people and services that must hear from one another for the system to function: stake pool operators must learn of a critical vulnerability before it is exploited, delegators must learn their pool is retiring before it affects them, voters must learn a governance action is open while there is still time to act on it. The chain's security and governance models quietly presume this communication happens: incident response assumes operators can be reached; accountability in governance assumes constituents hear from their representatives.

Cardano has no standard way to deliver a message that must be trustworthy but does not belong in a transaction. The chain settles state; it is not a medium for the operational and time-sensitive traffic around that state. Today that traffic runs on infrastructure outside the ecosystem's trust model: mailing lists, Discord and Telegram channels, vendor push services, and each provider's own backend.

That arrangement has a specific consequence. Traffic of this kind needs three of the four classic communication-security properties. Authenticity: the recipient can verify who sent a message. Integrity: it arrived as written. Availability: it reaches everyone it should, when it should. Confidentiality, the fourth, is not required for the needs identified here, since these messages are broadcasts, meant to be read.

Existing channels each provide some of these properties, none all three. An end-to-end encrypted messenger preserves integrity and confidentiality, but its notion of identity has no connection to Cardano's: a stake pool operator receiving an urgent notice cannot verify that it came from the protocol team it appears to come from, that it is the current version, or that other operators received it too. Availability fares worse, because each channel is a single privately run service. Messages can be dropped, delayed, or delivered selectively, through outage, compromise, or policy, and the recipient cannot tell which. The chain beneath is Byzantine-fault-tolerant; the channel used to coordinate around it is not. The weaker layer sets the effective guarantee.

### Why existing peer-to-peer messaging does not close it

Substituting a peer-to-peer protocol for the centralised channel removes the operator but does not, on its own, supply the missing guarantee.

Mature gossip protocols — GossipSub being the widely deployed example — are engineered against message-level attacks such as flooding and spam, and mitigate them with peer scoring and mesh hardening. Their resistance to *eclipse*, in which a victim's every neighbour is adversarial and its view of the network is controlled, ultimately rests on the peer discovery layer beneath. In the common libp2p deployment that layer admits freely created identities. An adversary willing to run many of them can influence which peers a target connects to, and neither peer scoring nor mesh hardening restores a guarantee that has been lost at the point of neighbour selection.

The missing ingredient is therefore not a better gossip mechanism. It is a peer set whose membership is costly to inflate and whose topology no participant can steer. Cardano already maintains the first: an on-chain registry with an associated cost is exactly a Sybil-resisted membership list. It also maintains the second: verifiable, unpredictable per-epoch randomness. A dissemination layer anchored on both can offer what neither a centralised broker nor an unanchored gossip mesh can.

### Use cases and stakeholders

The design is motivated by four standing scenarios, drawn from a [broader survey of candidate use cases](https://github.com/input-output-hk/pubsub/blob/main/docs/actor-use-case-analysis.md). They are listed with the participant counts that drive the design, because those counts, not the size of the eventual audience, determine what the protocol must sustain.

<div align="center">
<a name="table-1" id="table-1"></a>

| Scenario | Publishers | Direct protocol participants | Delivery requirement |
| --- | --- | --- | --- |
| Protocol developer teams → stake pool operators: emergency alerts and operational coordination | ~10 | ~3,000 SPO nodes, always-on | High; a missed critical alert has operational cost |
| Stake pools → delegators: operational announcements | Hundreds | Wallet backends, on behalf of a mediated audience of hundreds of thousands | Best-effort |
| Governance bodies and DReps → community: proposal notifications, voting alerts, and voting-intent disclosure | Tens to hundreds | Wallet backends, mediated | Medium to high; tied to voting deadlines |
| dApps → users: position alerts and protocol notifications | Tens | Wallet backends, mediated, with delivery targeted by address | High; alerts are financially consequential |

<em>Table 1: the four standing scenarios</em>

</div>

Two properties of this table shape the proposal.

First, **the audience is large but the participant set is not.** Where recipients number in the hundreds of thousands, they are reached through wallet infrastructure providers, of which there are on the order of ten. The nodes that must participate in dissemination are the always-on operators: stake pools, wallet backends, dApp and governance infrastructure. That population is in the low thousands today, dominated by the roughly three thousand stake pools. The Rationale sizes its evaluation accordingly, at four thousand nodes to match it and twenty thousand as headroom for growth well beyond it. At this scale a topology bounded in connections per node, and derivable in full by every participant, is tractable rather than aspirational.

Second, **the participants are already registered on-chain, or can be.** Stake pool operators are registered by construction. This is what makes an on-chain trust root a natural fit rather than an imposition: the registry the protocol needs substantially exists, and the identities in it are already backed by a cost.

The stakeholders are correspondingly: stake pool operators, as the largest set of direct participants and the recipients in the most delivery-critical scenario; wallet and infrastructure providers, whose integration is what connects the protocol to end users; governance bodies, DReps, and dApp teams as publishers; and protocol developer teams, who currently lack any authenticated broadcast channel to operators at all.

### What a solution has to provide

The scenarios above, together with the failure mode in the previous section, imply requirements that jointly rule out both incumbent options:

- **Authenticity and integrity.** A recipient must be able to verify that a message originated with the claimed publisher and reached them as written, without trusting the path it arrived over. A signature that verifies establishes both, so integrity requires no separate mechanism.
- **Censorship resistance.** This is availability restated against an adversary that chooses its target: suppressing a message must require luck rather than choice. No participant may be able to place itself where it can silence a chosen publisher or subscriber. Isolation cannot be prevented in every draw, since a subscriber whose every upstream peer happens to be adversarial receives nothing however small the adversarial fraction. The requirement is therefore that such isolation be rare, that it end when the topology is next drawn, and that it not be repeatable at will. The Rationale quantifies the first two.
- **Non-influenceable neighbour selection.** Which peers a node disseminates with must be set by the protocol, not negotiated between participants, and no participant may be able to steer that choice: not by registering additional identities, not by timing its own registration, not by influencing the randomness the assignment derives from. This is what a discovery layer with freely created identities fails to provide, and what makes the censorship requirement achievable at all.
- **Bounded cost per node.** Participation must not require a node to hold connections, or carry traffic, in proportion to the size of the network. The Rationale measures these as *standing links per node* and *copies per honest node*. Both must stay bounded as the network grows, or only well-resourced operators will participate, reintroducing informally the centralisation the proposal removes.
- **Openness to arbitrary payloads.** The scenarios differ widely in content and cadence. The protocol carries topics, and does not interpret what those topics transport.

The Specification that follows defines a protocol meeting these requirements: on-chain registration for the peer set, per-epoch verifiable randomness for the topology drawn over it, and topics as the unit of subscription. The Rationale then examines where the guarantees stop.

<!-- TODO before submission upstream: CIP-0001 asks that complex proposals link a
Cardano Problem Statement as the Motivation. Confirm whether an existing CPS
covers ecosystem messaging; if not, decide whether to author one or to argue
this section stands alone. -->

## Specification
<!-- The technical specification should describe the proposed improvement in sufficient technical detail. In particular, it should provide enough information that an implementation can be performed solely on the basis of the design in the CIP. This is necessary to facilitate multiple, interoperable implementations. This must include how the CIP should be versioned, if not covered under an optional Versioning main heading. If a proposal defines structure of on-chain data it must include a CDDL schema in its specification.-->

## Rationale: How does this CIP achieve its goals?
<!-- The rationale fleshes out the specification by describing what motivated the design and what led to particular design decisions. It should describe alternate designs considered and related work. The rationale should provide evidence of consensus within the community and discuss significant objections or concerns raised during the discussion.

It must also explain how the proposal affects the backward compatibility of existing solutions when applicable. If the proposal responds to a CPS, the 'Rationale' section should explain how it addresses the CPS, and answer any questions that the CPS poses for potential solutions.
-->

<!-- Cross-reference convention: a FORWARD-REF comment marks prose that will point at
     a section not yet written. Each names the target section and what must exist there,
     so the reference can be completed without recovering the intent. Grep FORWARD-REF
     before declaring a section finished. -->

Throughout this Rationale an **epoch** means one dissemination period — the interval for which a drawn topology stands and over which the guarantees below are stated. Its length is a parameter of this proposal and is not required to coincide with the ledger epoch; the bounds on it are an open question below.

### The adversary this proposal defends against

The protocol is analysed against an adversary controlling a bounded fraction **μ** of registered nodes, each of which is *silent*: it registers legitimately, accepts its allotted share of links, and then forwards nothing. This is deliberately the weakest adversary that still defeats delivery. A node that never emits a message cannot be distinguished from an honest node that has nothing to forward, so it is also the cheapest attack to mount and the hardest to observe. An eclipse attack against a specific subscriber reduces to this behaviour among that subscriber's upstream peers.

Not modelled, and out of scope for this proposal: an adversary that forwards selectively or forwards corrupted content, resource exhaustion and denial of service, and an adaptive adversary that re-registers between epochs in order to re-target a chosen victim.

Honest node churn is not a separate threat model. An honest node that is offline for an epoch is indistinguishable, to every other node, from a silent adversary: it holds its allotted links and forwards nothing. Independent honest downtime with per-epoch probability *p* therefore enters the coverage analysis as a shift in the adversarial fraction, from μ to μ + *p*(1−μ), and the same results apply at the shifted value. What remains preliminary is the validation rather than the model — the shifted-μ prediction has not yet been checked against a simulation that marks nodes down, and correlated downtime such as upgrade waves or region outages is not captured by a single independent *p*.

### Evidence

This section sets out what was measured, how, and what the results do and do not establish. It proceeds in that order: first the quantity being predicted and the two instruments that predict it, then the metrics and the designs compared, then the results, then the limits.

#### What is being measured

Recall the structure the measurements apply to. Each epoch, the protocol derives a dissemination topology over the registered nodes: every node is assigned a bounded set of peers to exchange messages with, and that assignment stands for the whole epoch. Nodes that follow the protocol are *honest*; the rest accept their assigned links and forward nothing — the adversary set out in the subsection above. On any given topic some nodes publish and others subscribe.

The guarantee is therefore a property of the drawn topology rather than of an individual message. For a given epoch's assignment, either every honest publisher's messages can reach every honest subscriber over the links that exist, or some publisher's cannot — in which case that publisher is cut off for the whole epoch, every time it publishes. A draw of the first kind is called **good** and one of the second **bad**.

This is deliberately an all-or-nothing criterion rather than an average over messages, because averaging hides the failure mode that matters. A design delivering 99.99 % of all messages might be dropping a uniform trickle, which is tolerable, or silencing one publisher completely, which is not. The two are indistinguishable in an average and are distinguished exactly by this criterion.

The central quantity is then the probability that a draw is bad, written *p*<sub>bad</sub>, and the design problem is to make it small at acceptable cost. **Everything else in this section is either a way of estimating *p*<sub>bad</sub>, a cost paid to lower it, or a condition under which it rises.**

#### How it is measured

Two independent instruments estimate *p*<sub>bad</sub>, and were built separately.

The first is **analysis**. For each candidate design, an expression called a *coverage law* predicts *p*<sub>bad</sub> in closed form, given the network size, the adversarial fraction, and the design's own parameters. Each law is derived from an abstract model of the design and comes with its own simulator, which samples topologies at random and checks each one against the good/bad criterion directly, so the law can be checked against sampling wherever sampling is feasible.

The second is **measurement**. A framework builds populations of the reference implementation's own node logic — the same code the node runs, driven by a deterministic scheduler in place of a network — draws a topology, disseminates real messages over it, and counts what happens: whether coverage was achieved, how many copies crossed the network, how many forwarding steps were needed.

Neither on its own would be convincing. A closed-form law can be a good approximation of the wrong model; an implementation can faithfully run a subtly incorrect protocol. They fail in unrelated ways, so **agreement between them is the evidence offered here** — not either result alone.

Every measurement is reproducible byte-for-byte from a tool commit, a configuration, and a master seed, independently of how many runs execute in parallel.[^reproduction]

#### Performance metrics

A design is characterised by three things: how often a draw fails, what it costs to run at that failure rate, and how much degradation it absorbs before the failure rate changes. The metrics below express those three, and are stated per epoch throughout, since the guarantee is a property of each epoch's standing assignment.

Two of them are design inputs rather than outcomes: *μ*, the fraction of nodes assumed adversarial, and *δ*, the failure probability a configuration is required to meet. This proposal uses *δ* = 10⁻⁴ per epoch.

<div align="center">
<a name="table-2" id="table-2"></a>

| Category | Metric | Measurement |
| :--: | --- | --- |
| Coverage | Epoch failure probability, *p*<sub>bad</sub> | Probability that a drawn epoch topology fails to carry some honest publisher's messages to every honest subscriber |
| | Design target, *δ* | The value of *p*<sub>bad</sub> a configuration is sized to meet |
| Cost | Transmissions per publication, *m* | Honest-to-honest message copies sent per published message, duplicates included |
| | Copies per honest node, *c* | Copies of each published message received by an average honest node |
| | Standing links per node, *d* and *d̂* | Connections held open for the whole epoch, mean and maximum, counting a node's own picks and the links others opened to it |
| Latency | Hops to full coverage, *h*<sub>full</sub> | Forwarding depth at which the last honest subscriber receives |
| | Mean first receipt, *h*<sub>mean</sub> | Forwarding depth at which a typical honest subscriber first receives |
| Resilience | Adversarial fraction, *μ* | Share of registered nodes that accept their links and forward nothing |
| | Churn budget, *p*<sub>max</sub> | Largest honest downtime fraction for which a deployed configuration still meets *δ* |

<em>Table 2: performance metrics</em>

</div>

**_Epoch failure probability._** The probability that a drawn assignment is bad in the sense defined above. Because it is a property of the draw rather than of a message, it is estimated by sampling many topologies and counting how many fail.

$$p_\text{bad} = P(\text{some honest publisher cannot reach every honest subscriber over the epoch's links})$$

**_Design target._** A configuration is chosen as the cheapest one whose *p*<sub>bad</sub> meets *δ*. Note that *δ* is a choice, not a property of any design, and that the same per-epoch value means different things at different epoch lengths: one failure in ten thousand epochs is roughly once a century at multi-day epochs and roughly annual at hourly ones.

**_Transmissions per publication and copies per honest node._** The bandwidth cost. Both count copies sent between honest nodes, duplicates included — a duplicate is suppressed on receipt, but it has already crossed the network and been paid for. The two differ only in what they are divided by, so either may be quoted; with *H* the number of honest nodes,

$$c = m / H$$

**_Standing links per node._** The state cost: connections a node keeps open for the epoch whether or not traffic flows over them. This is a separate axis from bandwidth, because a design can be frugal with messages while still requiring many open connections. The maximum matters as well as the mean, since connection slots must be provisioned for the worst-case node, and the worst case grows with network size even when the average does not.

**_Hops._** Latency measured in forwarding steps rather than seconds, so the figure does not depend on any particular deployment's link latencies. Both the typical case and the tail are reported: a design can reach most subscribers quickly and the last one slowly, and for time-sensitive topics it is the last one that binds.

**_Adversarial fraction._** The assumed share of registered nodes that accept their assigned links and forward nothing, as defined in [The adversary this proposal defends against](#the-adversary-this-proposal-defends-against) above, which also records what is deliberately not covered.

**_Churn budget._** Honest downtime raises the effective adversarial fraction to *μ* + *p*(1−*μ*), for the reason given in the adversary subsection above, so a design's own coverage law can simply be read at that higher value. The churn budget is the largest downtime a deployed configuration absorbs while still meeting the target:

$$p_\text{max} = \max \{\, p : p_\text{bad}(\mu + p(1-\mu)) \le \delta \,\}$$

Downtime relates to the rate at which nodes drop out and to the epoch length by *p* = 1 − e<sup>−λ·T</sup> for a drop-out rate λ over an epoch of length T: the longer an epoch runs without repairing dead links, the more downtime accumulates within it. This is why *p*<sub>max</sub> is not only a resilience figure but also an upper bound on epoch length.

#### Designs evaluated

Five dissemination designs were analysed against the metrics above. They were not arbitrary alternatives: each varies one structural choice, so that the comparison isolates what that choice costs.

The choices are: whether a node *pushes* messages to peers it selected, or *pulls* from peers it selected — the difference matters because it determines which failure a node can suffer, being unable to receive or being unable to be heard; whether a link carries traffic in one direction or both; and whether a node has a dedicated way to seed its own publications separate from the links it relays over. Each design's tuning parameter is the number of peers a node selects, which is the knob that trades cost against *p*<sub>bad</sub>.

<div align="center">
<a name="table-3" id="table-3"></a>

| Design | Mechanism | Tuning parameters |
| :--: | --- | --- |
| M1 | Push: each node forwards to *F* randomly drawn targets | *F* |
| M2 | Pull: each node draws *RF* forwarders and receives from them | *RF* |
| M3 | M2, plus *s*−1 standing initiation links carrying only their owner's own publications | *RF*, *s* |
| M4 | Each node draws *RF* peers; links are bidirectional and flood | *RF* |
| M5 | Directed: each node opens *k*<sub>in</sub> inbound and *k*<sub>out</sub> outbound links | *k*<sub>in</sub>, *k*<sub>out</sub> |

<em>Table 3: the dissemination designs evaluated</em>

</div>

M1 and M2 are the two halves of M5 taken separately: switching off M5's inbound links leaves pure push, and switching off its outbound links leaves pure pull. That gives a free consistency check on both the analysis and the implementation — M5 configured at those boundaries must reproduce M1's and M2's results exactly, and any discrepancy is a defect in one of the three rather than a property of the protocol.

<!-- Figures are generated, not hand-drawn: pubsub-node/docs/experiments/cells.json is
     the single source, and make_cip_figures.py regenerates images/*.svg from it.
     `make_cip_figures.py --check` fails if a committed SVG is stale, so the figures
     cannot drift from the data.

     TODO(evidence) still outstanding:
       1. the churn sweep (experiment E13) at the five operating points, which is what
          "Robustness" below is waiting on;
       2. depth histograms at those points, so the latency column becomes a
          distribution rather than five rounded means;
       3. cells.json emitted by the experiments tool directly, retiring the one-time
          transcription from the comparison documents. -->

#### Agreement between analysis and simulation

The laws were checked against the measurement framework at 23 configurations, spanning all five designs, three orders of magnitude in *p*<sub>bad</sub>, and two network sizes: *N* = 4,000, which is the order of today's stake-pool population, and *N* = 20,000 as headroom above it. Each configuration draws between 200 and 30 000 topologies and counts the bad ones; the count is compared against what that design's law predicts.

<div align="center">
<a name="figure-1" id="figure-1"></a>

![Measured against predicted epoch failure probability](images/coverage-validation.svg)

<em>Figure 1: measured against predicted epoch failure probability. Each point is one tested configuration: its horizontal position is the failure rate the coverage law predicts, its vertical position the rate actually observed, and its bar the Wilson 95 % interval around that observation. Both axes are logarithmic, so the configurations span from failing in roughly one epoch in three hundred (lower left) to failing in almost every epoch (upper right). <strong>The result to read off is that the points sit on the diagonal throughout</strong> — the prediction matches the measurement at every failure rate tested, with no drift at either end.</em>

</div>

The points lie on the diagonal across the whole range. Per configuration, the law falls inside the measurement's 95 % interval in 22 of the 23 — the exception being one 1 500-draw configuration whose independent 6 000-draw resample brings it inside.

Per-configuration agreement is the weaker claim, though, because with 23 comparisons a few near-misses are expected and a consistent small bias would hide behind them. The stronger check is aggregate: across the 22 non-degenerate configurations the mean standardised deviation from the laws is +0.21, which over 22 comparisons is not distinguishable from zero. The spread of those deviations is 0.83 against the 1.0 that pure sampling noise would produce, so the agreement is if anything closer than chance alone would give.

The same comparison against the analysis team's own independent simulators gives a mean standardised deviation of +0.05 over 22 paired configurations. **The two implementations are statistically indistinguishable from each other and from the laws**, which is the claim this section exists to support.

<!-- TODO(evidence): per-configuration table generated from cells.json, rather than
     restating the figure in prose. -->

#### Comparison at the design target

Each design was then tuned to its cheapest configuration meeting *δ* = 10⁻⁴ at *N* = 20 000, *μ* = 0.2, and the costs compared. Because every entry is equally safe by construction, the table is a pure cost comparison.

<div align="center">
<a name="table-4" id="table-4"></a>

| Design | Parameters | Messages per publication | Copies per node | Standing links | Hops (full) | Hops (mean) |
| :--: | --- | ---: | ---: | ---: | ---: | ---: |
| M3 | RF = 12, *s* = 8 | **153,577** | **9.6** | 38 | 5.9 | 4.3 |
| M4 | RF = 8 | 188,751 | 11.8 | **16** | 5.1 | 4.1 |
| M5 | (9, 8) | 217,530 | 13.6 | 34 | 5.0 | 4.0 |
| M1 | *F* = 24 | 307,201 | 19.2 | 48 | 5.0 | 3.6 |
| M2 | RF = 24 | 307,162 | 19.2 | 48 | **4.8** | **3.6** |

<em>Table 4: cost at equal safety — every design tuned to the same failure target, so the rows differ only in what that safety costs. Bold marks the best value in each column. Measured values; see the reproduction note.</em>

</div>

<div align="center">
<a name="figure-2" id="figure-2"></a>

![Bandwidth cost against state cost at equal safety](images/cost-vs-state.svg)

<em>Figure 2: bandwidth cost against state cost. Every design here is tuned to the same failure target, so the points differ only in what that safety costs: horizontally in connections a node must hold open all epoch, vertically in message copies it receives, and in marker size by how many forwarding steps the last subscriber waits. Both axes are costs, so lower and further left is better. <strong>The result to read off is that M3 and M4 sit on a frontier no other design reaches</strong> — M3 spends the least bandwidth, M4 the fewest connections, neither beats the other on both, and M1, M2 and M5 are beaten on both at once.</em>

</div>

Three things follow, and the third is the one that matters for the choice.

**Latency does not discriminate.** The whole field spans 4.8 to 5.9 forwarding steps. At wide-area per-hop times this is a difference of a few hundred milliseconds between the best and worst design, which is unlikely to decide anything for the use cases in the Motivation.

**Bandwidth and state disagree about the winner.** M3 is cheapest in traffic and M4 in held connections, and neither beats the other on both. M3's standing links exceed what its traffic would suggest because 14 of its 38 links carry only their owner's own publications — cheap to run, but still connection slots to provision and still exposed to churn.

**M1, M2 and M5 are beaten on every axis at once**, so no weighting of bandwidth against state selects them. The choice is between M3 and M4, and it turns on which resource binds in the deployment. The remaining subsection is what stops that from being the whole answer.

The design this proposal adopts, and the parameters it fixes, are given in the [Specification](#specification); this subsection establishes only what each candidate costs, and the one below establishes what each gives up under degradation.<!-- FORWARD-REF(specification): the Specification must name the adopted design (expected M3 or M4) and its parameters, so this sentence resolves to a concrete choice. Selection is blocked on the Robustness subsection below; see input-output-hk/pubsub#85. -->

#### Robustness

The comparison above holds every design at the same failure probability *under the assumption that all honest nodes are up*. Since honest downtime enters as a shift in the adversarial fraction, each design also has a churn budget — the downtime it absorbs before leaving the target — and those budgets are not equal.

**This subsection is deliberately incomplete.** The churn budgets can be obtained today by reading each design's coverage law at the shifted fraction, but that is a prediction, and no measurement has yet marked nodes down and re-checked coverage. Publishing the predicted figures here as though they were measured would misrepresent them.

What can be said now is that the ordering is expected to differ from the one in Table 4, so **the cost comparison above should not be read as settling the choice of design until this subsection is filled in**.

<!-- TODO(evidence): fill from the churn sweep (experiment E13) at the five operating
     points — p_bad against honest downtime per design, and the resulting p_max, as a
     third generated figure. Provisional law-derived values and the reasoning:
     input-output-hk/pubsub#19. -->

#### Limits of this evidence

The following are stated so that a reader can judge what the numbers above do and do not establish.

**The configurations that were measured are not the configurations that are proposed.** Sampling can only resolve a failure probability down to roughly one over the number of trials: observing a one-in-ten-thousand event enough times to estimate its rate takes far more than ten thousand draws. The configurations that meet the design target are, by construction, ones that almost never fail, so measuring them directly is impractical. What was measured is a range of deliberately weaker configurations, where failures are common enough to count; the laws are checked there, and then relied on to predict the proposed configurations roughly three orders of magnitude further down. The laws are expected to be accurate in that range — the dominant failure mode there is the simplest one, a single node with no usable links, which they model exactly — but this remains an extrapolation rather than a measurement.

**A known small correction to the laws is unresolved in size.** Beyond single cut-off nodes, a draw can also fail because a small *group* of nodes is collectively cut off. The laws count the first case exactly and the second only approximately, so they are expected to be slightly optimistic — to predict marginally fewer failures than really occur — in the range where failures are rare. Independent samples disagree on how much: none is large enough to settle it, because distinguishing a ten-percent difference at these probabilities needs on the order of 10⁵ draws per configuration. Where a configuration's margin against the target is no larger than this uncertainty, the margin should be read as approximate rather than exact.

**The state axis is measured less precisely than the cost axis.** Transmission counts are reproduced between the two instruments to within a small fraction of a percent. Standing-link counts are not measured to comparable precision, and links that carry no propagation traffic are not captured in the measured degree distributions at all. Where a comparison turns on state rather than bandwidth, it rests on the weaker of the two axes.

**One adversarial fraction.** All results are at a single value of *μ*. That value is an assumption about the deployment, not a measurement of it, and the designs do not degrade at equal rates as it varies.

**Correlated failure is out of scope.** Downtime is modelled as independent across nodes and epochs. Region outages and upgrade waves violate both assumptions, in the direction that makes the guarantee weaker, and are not quantified here.

### Trade-offs and Limitations

#### Two classes of fault, with different guarantees

The protocol distinguishes faults that are attributable from faults that are not, and the boundary between them is not a matter of engineering effort. Accountability for the *presence* of an incorrect message and accountability for the *absence* of a message are formally different problems.[^accountable-liveness]

**Attributable faults** are evidenced by a message that was actually sent, and any recipient can verify them without cooperation from anyone else:

- content that is malformed under, or contradicts, the publisher's signature, checkable against the publisher's registered key;
- a message sent by a peer outside the connections permitted to it for the current epoch, checkable against the obligation graph, which any participant can derive from the on-chain registry together with the epoch's public randomness.

**Non-attributable faults** consist of the absence of messages. Attributing these is provably impossible without both a network that is more often synchronous than asynchronous and an honest majority among the parties able to attest.[^accountable-liveness] This proposal assumes neither. The dissemination analysis makes no timing assumption at all, and attestation here is inherently local: the only parties who can speak to whether a given relay forwarded a given message to a given subscriber are those two nodes. With two potential attesters there is no majority to appeal to, and a subscriber's entire upstream set can be adversarial even when the network-wide fraction μ is small — that case is one of the failure modes making up the residual per-epoch failure probability that the Evidence subsection quantifies.<!-- FORWARD-REF(evidence): resolves once the Evidence subsection lands; link it directly. -->

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
- The adversarial fraction the deployment should be sized against. The analysis is carried out at a single value throughout, and that value is an assumption about who registers and what registration costs them rather than a result of the analysis. It should be justified against the registry's actual cost structure — and against the observation that a subscriber only needs its own upstream set captured, not the network — before parameters are fixed.
- The per-epoch failure probability to target, which is likewise a choice rather than a derived quantity. It cannot be read independently of epoch length: the same per-epoch figure is a rare event at multi-day epochs and a routine one at short epochs, so the target and the epoch length have to be argued together.
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

[^reproduction]: Reproducing the measurements. Each result is identified by a tool commit, a sweep configuration, and a master seed; those three reproduce the output files byte-for-byte, independently of how many runs execute in parallel. All three are recorded per configuration in [`cells.json`](https://github.com/input-output-hk/pubsub/blob/main/pubsub-node/docs/experiments/cells.json), which is also the source the figures in this section are generated from; the configurations themselves are under [`configs/experiments/`](https://github.com/input-output-hk/pubsub/tree/main/pubsub-node/configs/experiments) and the per-design comparisons, including the statistical conventions, under [`docs/experiments/`](https://github.com/input-output-hk/pubsub/tree/main/pubsub-node/docs/experiments).

## Copyright
<!-- The CIP must be explicitly licensed under acceptable copyright terms. Uncomment the license you wish to use (delete the other one) and ensure it matches the License field in the header.

If AI/LLMs were used in the creation of the copyright text, the author may choose to include a disclaimer to describe their application within the proposal.
-->

This CIP is licensed under [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/legalcode).

---
CPS: "?"
Title: Trustworthy Off-chain Message Dissemination
Status: Open
Category: Network
Authors:
    - Will Wolff <william.wolff@iohk.io>
    - Ezequiel Postan <ezequiel.postan@iohk.io>
    - Denis Firsov <denis.firsov@gmail.com>
    - Jesus Diaz Vico <jesus.diaz.vico@gmail.com>
    - Dana Alibrandi <dalibrandi@gmail.com>
    - Mauro Jaskelioff <mauro.jaskelioff@iohk.io>
Proposed Solutions:
    - ../cip/README.md
Discussions:
    - Original PR: https://github.com/cardano-foundation/CIPs/pull/?
Created: 2026-08-20
License: CC-BY-4.0
---

## Abstract

Cardano's security and governance models presume that the people and services around the chain can reach one another. Stake pool operators must learn of a critical vulnerability before it is exploited; delegators that their pool is retiring; voters that a governance action is open while there is still time to act. None of that traffic belongs in a transaction, and today all of it runs on infrastructure outside the ecosystem's trust model: mailing lists, chat platforms, vendor push services, each provider's own backend.

Nothing binds such a message to the on-chain identity of its sender, and delivery depends on a single service. The chain beneath is Byzantine-fault-tolerant; the channel used to coordinate around it is not, and the weaker layer sets the effective guarantee. Substituting a peer-to-peer protocol removes the operator but does not supply the missing guarantee, because the mature gossip protocols rest on a discovery layer that admits freely created identities.

This problem statement sets out the gap, the scenarios that motivate closing it, and the properties any solution has to have. It states those properties as outcomes, not as mechanisms, and does not specify a solution.

## Problem

### The gap

Cardano does not run itself. Behind the protocol sits a network of people and services that must hear from one another for the system to function. The chain's security and governance models quietly presume this communication happens: incident response assumes operators can be reached, accountability assumes constituents hear from their representatives.

Yet Cardano has no standard way to deliver a message that must be trustworthy but does not belong in a transaction. The chain settles state; it is not a medium for the operational and time-sensitive traffic around that state.

Traffic of this kind needs three of the four classic communication-security properties:

- **Authenticity.** The recipient can verify who sent a message.
- **Integrity.** It arrived as written.
- **Availability.** It reaches everyone it should, when it should.

Confidentiality, the fourth, is not required: these messages are broadcasts, meant to be read.

Existing channels each provide some of these, none all three. An end-to-end encrypted messenger preserves integrity and confidentiality, but its notion of identity has no connection to Cardano's: an operator receiving an urgent notice cannot verify that it came from the protocol team it appears to come from, that it is the current version, or that other operators received it too. Availability fares worse, because each channel is a single privately run service: messages can be dropped, delayed, or delivered selectively, through outage, compromise, or policy, and the recipient cannot tell which.

### Why existing peer-to-peer messaging does not close it

Mature gossip protocols, of which GossipSub is the widely deployed example, are engineered against message-level attacks such as flooding and spam, and mitigate them with peer scoring and mesh hardening.[^gossipsub] Their resistance to *eclipse* — a victim whose every neighbour is adversarial, and whose view of the network is therefore controlled — rests on the peer discovery layer beneath, and in the common libp2p deployment that layer admits freely created identities.[^libp2p] An adversary willing to run many of them can influence which peers a target connects to, and neither peer scoring nor mesh hardening restores a guarantee lost at the point of neighbour selection.

Hardening the layer beneath has been tried, and it is the closest existing work to this problem. SecureCyclon is the Byzantine-hardened descendant of CYCLON, designed to keep peer sampling dependable under attack, and it carries nine separate defences.[^securecyclon] Analysis of it under the weakest adversary that still defeats delivery — one that stays rate-honest and well-formed, and varies only which peers it contacts, which descriptors it passes on, and what it withholds — found a reliable targeted eclipse of a chosen victim at a low adversarial share.[^cyclonreport] The defences cannot reach that behaviour, because none of them can prove what a peer chose to send or to withhold, and a peer that does not respond is indistinguishable from one that has churned.

That result is what shapes this statement. The missing ingredient is not a better gossip mechanism, and not a better sampler either. It is a peer set whose membership is costly to inflate and whose topology no participant can steer — including no participant who is prepared to behave impeccably except for what it quietly declines to pass on.

### Why this is hard to solve on the chain it protects

One scenario makes the problem circular, and it is the scenario with the strongest delivery requirement. A channel that warns stake pool operators about a problem with the chain, and that takes its trust root from that same chain, is unavailable in part of the case it exists for. If the chain halts, an on-chain membership list stops updating and any chain-derived randomness stops advancing. If the chain forks, participants reading different branches may disagree about who is a member.

This does not rule out anchoring on Cardano — the alternative is anchoring on some other Byzantine-fault-tolerant system, which relocates the dependency rather than removing it. It does mean a solution should state which of its components are substitutable, and what degrades when the anchor is unavailable.

## Use cases

Four scenarios drove this statement, drawn from a [broader survey of candidate use cases](https://github.com/input-output-hk/pubsub/blob/main/docs/actor-use-case-analysis.md).

<div align="center">

| Scenario | Publishers | Direct participants | What it asks |
| --- | --- | --- | --- |
| Protocol teams → stake pool operators: emergency alerts | ~10 | ~3,000 SPO nodes, always-on | The strongest delivery guarantee |
| Stake pools → delegators: announcements | Hundreds | Wallet backends, mediated | Best-effort only |
| Governance bodies and dReps → community: proposals, voting alerts, voting-intent disclosure | Tens to hundreds | Wallet backends, mediated | Delivery inside a voting deadline |
| dApps → users: position and protocol alerts | Tens | Wallet backends, mediated | Delivery targeted by address |

<em>Table 1: the four motivating scenarios</em>

</div>

The rows are not equally demanding, and they do not scale alike. Three things follow.

**The audience is large but the participant set is not.** Recipients in the hundreds of thousands are reached through wallet infrastructure providers, of which there are on the order of ten. The nodes that must disseminate are the always-on operators — stake pools, wallet backends, dApp and governance infrastructure — in the low thousands today, dominated by the roughly three thousand stake pools.

**The number of participants on one topic varies by two orders of magnitude.** The first row puts thousands of always-on nodes on a single topic. The other three reach their audience through wallet backends, so the nodes directly on such a topic may number in the tens. A solution evaluated at one end of that range has not been shown to work at the other, and any proposal should be explicit about which end its evidence covers.

**The participants are already registered on chain, or can be.** Stake pool operators are registered by construction, so an on-chain membership list substantially exists and the identities in it are already backed by a cost.

### Stakeholders

Stake pool operators are the largest set of direct participants and the recipients in the delivery-critical scenario. Wallet and infrastructure providers connect the protocol to end users. Governance bodies, dReps and dApp teams are publishers. Protocol developer teams today have no authenticated broadcast channel to operators at all.

## Goals

A solution must provide the following. The first three are what the failure of existing channels demands; the last two are what makes a solution deployable.

- **Authenticity and integrity.** A recipient must be able to verify that a message originated with the claimed publisher and reached them as written, without trusting the path it arrived over.
- **Censorship resistance.** Availability restated against an adversary that chooses its target: suppressing a message must require luck rather than choice. Isolation cannot be prevented in every draw — a subscriber whose every peer happens to be adversarial receives nothing, however small the adversarial fraction. The requirement is therefore that such isolation be rare, that it end without intervention, and that it not be repeatable at will.
- **No participant may choose who it is adjacent to.** Whatever determines which peers a node exchanges messages with, no participant may steer it — not by acquiring additional identities, not by timing when it joins, and not by influencing whatever the determination reads. This is what a discovery layer with freely created identities fails to provide, and it is what makes the requirement above achievable at all.
- **Bounded cost per node.** No node may have to hold connections, or carry traffic, in proportion to the size of the network. Both must stay bounded as the network grows, or only well-resourced operators will participate and the centralisation returns informally.
- **Openness to arbitrary payloads.** The scenarios differ widely in content and cadence. A solution should carry named streams of messages without interpreting what they transport.
- **Addressing left to the application.** One scenario asks for delivery targeted at a particular recipient rather than broadcast to all subscribers. A solution need not provide targeting itself, but must not prevent a publisher from addressing a payload that only its intended recipient acts on.

### Non-goals

- **Confidentiality.** The traffic is broadcast.
- **Message persistence.** Recovering content long after publication is a storage problem, separable from delivery, and is not required by the scenarios above.
- **Consensus on what was delivered.** Recipients need to know that what they received is authentic, not to agree with one another on a delivery log.

## Open Questions

- **What adversarial fraction is realistic, given what registration costs?** Every delivery guarantee is conditional on it, and it is a property of who registers rather than of any protocol. It should be justified against a registry's actual cost structure, and against the observation that a subscriber only needs its own peer set captured rather than the network.
- **What delivery guarantee do these scenarios actually require?** A per-period failure probability cannot be read independently of how long that period lasts: the same figure is a rare event at multi-day periods and a routine one at short ones.
- **How reliable is the population?** Honest downtime is indistinguishable from adversarial silence to the rest of a network, so the rate at which participants drop out sizes a solution as much as the adversary does. It has not been measured.
- **What population do the topics that matter actually draw from?** Whether the mediated scenarios put tens or hundreds of nodes on a topic decides whether one mechanism can serve all four rows.
- **Do the small scenarios need a different mechanism?** Several use cases put tens of nodes on a topic. Whether that regime is served by the same design, by a degenerate case of it, or by something else, is unsettled.
- **What should an identity cost, and should that cost decay?** A static cost prices Sybil identities. A cost that decays in the absence of evidence of participation would deter withholding — but deterrence requires a record a third party can check after the fact, which an in-network mechanism does not produce.
- **How much of a solution must be substitutable?** Given the circularity above, which components may depend on the chain, and what is required to keep working when it is not available.

## References

### Prior art

- Vyzovitis, Napora, McCormick, Dias and Psaras. *GossipSub: Attack-Resilient Message Propagation in the Filecoin and ETH2.0 Networks.* arXiv:2007.02754. <https://arxiv.org/abs/2007.02754>
- *gossipsub v1.1 — Security extensions to improve on attack resilience and bootstrapping.* <https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.1.md>
- libp2p. <https://libp2p.io> — and its Kademlia DHT, the peer discovery layer in the usual deployment: <https://github.com/libp2p/specs/tree/master/kad-dht>
- Antonov and Voulgaris. *SecureCyclon: Dependable Peer Sampling.* 43rd IEEE International Conference on Distributed Computing Systems, ICDCS 2023, pp. 1–12. <https://doi.org/10.1109/ICDCS57875.2023.00041>
- The peer-sampling survey this statement draws on, and the analysis of SecureCyclon under a silent adversary: <https://github.com/input-output-hk/pubsub/blob/main/formal_spec/related_work/related_peersampling.md> and <https://github.com/input-output-hk/pubsub/blob/main/formal_spec/peer_sampling/secure_cyclon/REPORT.md>

### Related documents

- A proposed solution to this statement: [CIP](../cip/README.md), in this repository.
- CIP-0137, *Decentralized Message Queue*. <https://github.com/cardano-foundation/CIPs/tree/master/CIP-0137> — an existing Network-category proposal for
  topic-based message diffusion on Cardano. It addresses part of this problem for stake pool
  operators, authenticating participants by their operational certificates so that Sybil
  resistance follows from active stake. It states no delivery guarantee and no resistance to
  targeted censorship, which is what this statement asks a solution to supply.
- The broader survey the four scenarios were drawn from: <https://github.com/input-output-hk/pubsub/blob/main/docs/actor-use-case-analysis.md>
- *PubSub Technical Report 1: Three-Layer Stack Findings and a Path Forward* — the evaluation
  that led to this statement:
  <https://github.com/input-output-hk/pubsub/blob/main/docs/technical-report-1.md>

### Method notes

[^gossipsub]: Dimitris Vyzovitis, Yusef Napora, Dirk McCormick, David Dias and Yiannis Psaras. *GossipSub: Attack-Resilient Message Propagation in the Filecoin and ETH2.0 Networks.* arXiv:2007.02754. <https://arxiv.org/abs/2007.02754>. The peer scoring and mesh hardening referred to here are specified in gossipsub v1.1, *Security extensions to improve on attack resilience and bootstrapping*: <https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.1.md>.

[^securecyclon]: Antonov and Voulgaris. *SecureCyclon: Dependable Peer Sampling.* 43rd IEEE International Conference on Distributed Computing Systems, ICDCS 2023, pp. 1–12. <https://doi.org/10.1109/ICDCS57875.2023.00041> The hardened descendant of CYCLON, and the peer-reviewed state of the art in Byzantine-resilient partial-view peer sampling.

[^cyclonreport]: Silent attacks on SecureCyclon. The adversary is rate-honest and sends only well-formed, single-chain descriptors; it varies only which peer it contacts, which descriptors it forwards, and what it withholds. Four attack shapes were measured, of which three are targeted at a chosen victim. Method and results: <https://github.com/input-output-hk/pubsub/blob/main/formal_spec/peer_sampling/secure_cyclon/REPORT.md>. This is the project's own analysis and has not been separately peer-reviewed.

[^libp2p]: libp2p, the modular networking stack GossipSub is most widely deployed on. <https://libp2p.io>. Peer discovery in the usual deployment is its Kademlia DHT, in which a peer identity is a self-generated key pair rather than an entry in any registry: <https://github.com/libp2p/specs/tree/master/kad-dht>.

## Copyright

This CPS is licensed under [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/legalcode).

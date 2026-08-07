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

### The gap

Cardano does not run itself. Behind the protocol sits a network of people and services that must hear from one another for the system to function: stake pool operators must learn of a critical vulnerability before it is exploited, delegators must learn their pool is retiring before it affects them, voters must learn a governance action is open while there is still time to act on it. The chain's security and governance models quietly presume this communication happens, i.e. incident response assumes operators can be reached; accountability in governance assumes constituents hear from their representatives.

Cardano has no standard way to deliver a message that must be trustworthy but does not belong in a transaction. The chain settles state; it is not a medium for the operational, advisory, and time-sensitive traffic that surrounds that state. Today such traffic runs on infrastructure outside the ecosystem's trust model — mailing lists, Discord and Telegram channels, vendor push services, and each provider's own backend.

That arrangement has a specific consequence. Traffic of this kind needs three of the four classic communication-security properties: authenticity: the recipient can verify who sent a message; integrity: it arrived as written; and availability: it reaches everyone it should, when it should. Confidentiality, the fourth, is not a requirement for the core needs as currently identified: these messages are broadcasts, meant to be read.

Existing channels each provide some of these properties; none provide the properties this traffic depends on, in the form the ecosystem needs. An e2e encrypted messenger genuinely preserves integrity and confidentiality, but its notion of identity has no connection to Cardano's: a stake pool operator receiving an urgent notice through such a channel cannot verify that it came from the protocol team it appears to come from, that it is the current version of that notice, or that other operators received it too. Because existing channels are single, privately run services, availability rests on a service being reachable and uncompromised. Messages can be dropped, delayed, or delivered selectively, whether through outage, compromise, or policy, and the recipient cannot tell the difference. The chain beneath is Byzantine-fault-tolerant; the channel used to coordinate around it is not. The weaker layer sets the effective guarantee.

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

First, **the audience is large but the participant set is not.** Wherever the ultimate recipients number in the hundreds of thousands, they are reached through wallet infrastructure providers, of which there are on the order of ten. The nodes that must actually participate in dissemination are the always-on operators — stake pools, wallet backends, dApp and governance infrastructure — a population in the low thousands today, dominated by the roughly three thousand stake pools. The evaluation in the Rationale is sized accordingly: it reports one network of four thousand nodes, matching that population, and one of twenty thousand as headroom for growth well beyond it. A participant set of this size is small enough that a topology bounded in connections per node, and derivable in full by every participant, is tractable rather than aspirational.

Second, **the participants are already registered on-chain, or can be.** Stake pool operators are registered by construction. This is what makes an on-chain trust root a natural fit rather than an imposition: the registry the protocol needs substantially exists, and the identities in it are already backed by a cost.

The stakeholders are correspondingly: stake pool operators, as the largest set of direct participants and the recipients in the most delivery-critical scenario; wallet and infrastructure providers, whose integration is what connects the protocol to end users; governance bodies, DReps, and dApp teams as publishers; and protocol developer teams, who currently lack any authenticated broadcast channel to operators at all.

### What a solution has to provide

The scenarios above imply requirements that jointly rule out both incumbent options:

- **Authenticity.** A recipient must be able to verify that a message originated with the claimed publisher, without trusting the path it arrived over.
- **Censorship resistance.** Suppressing a message must require luck rather than choice. No participant may be able to place itself where it can silence a chosen publisher or subscriber; and where an unlucky draw does isolate someone, that isolation must be improbable, must end when the topology is next drawn, and must not be repeatable at will. Prevention in every draw is not achievable — a subscriber whose every upstream peer happens to be adversarial receives nothing, however small the adversarial fraction — so the requirement is on how rare, how brief, and how unsteerable that event is. The Rationale takes up each in turn, and quantifies the first two.
- **Non-influenceable neighbour selection.** Which peers a node disseminates with must be determined by the protocol rather than negotiated between participants, and no participant may be able to steer that determination — whether by registering additional identities, by timing its own registration, or by influencing the randomness the assignment derives from. This is precisely the property a discovery layer with freely created identities fails to provide, and it is what makes the censorship requirement above achievable at all.
- **Bounded cost per node.** Participation must not require a node to hold connections, or carry message traffic, in proportion to the size of the network. The Rationale measures these as the two cost axes — *standing links per node* and *copies per honest node* — and both must stay bounded as the network grows, or only well-resourced operators will participate, which would reintroduce informally the centralisation the proposal removes.
- **Openness to arbitrary payloads.** The scenarios differ widely in content and cadence. The protocol carries topics, and does not interpret what those topics transport.

The remainder of this document specifies a protocol meeting these requirements, and the Rationale examines where the guarantees stop.

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

## Copyright
<!-- The CIP must be explicitly licensed under acceptable copyright terms. Uncomment the license you wish to use (delete the other one) and ensure it matches the License field in the header.

If AI/LLMs were used in the creation of the copyright text, the author may choose to include a disclaimer to describe their application within the proposal.
-->

This CIP is licensed under [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/legalcode).

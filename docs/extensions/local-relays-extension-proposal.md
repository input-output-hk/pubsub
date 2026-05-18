# Protocol Extension Proposal — Local Relays Table and Reciprocal Mutual-Trust Links

**Date:** April 2026
**Status:** Draft for discussion with the AUEB research team

## Purpose

This document records a second extension to the AUEB three-layer dissemination protocol developed in the Cardano PubSub workstream. It is designed to compose with the [Relay Tier Extension](relay-tier-extension-proposal.md) but is independently deployable: a deployment can run either extension alone, both together, or neither.

Where the relay-tier extension privileges a centrally curated set of relays (owner-attested in the topic registry), this extension privileges *peer-to-peer* trust: each edge node decides which other peers it trusts as relays for itself, and an additional link is added to the dissemination topology only when two peers mutually consent.

The mechanism is not a replacement for SecureCyclon's random sampling or Vicinity's clustering. Mutual-trust links are *additive* to the base topology — a third class of link alongside the Harary ring and the random links — providing high-confidence delivery within the mutual-trust subgraph while preserving the eclipse-resistance properties of the random layer.

Companion documents in this workstream:

- [Relay Tier Extension Proposal](relay-tier-extension-proposal.md) — the prior extension introducing owner-attested relays.
- [AUEB Gap Analysis](aueb-gap-analysis-final.md) — gap-analysis observations this extension is partly designed to attenuate.
- [Actor & Use Case Analysis](actor-use-case-analysis.md) — actor and use-case decomposition; UC-1 (SPO emergency alerts) is the canonical motivating use case.
- [PubSub Staged Design Synthesis](staged-design-synthesis.md) — overall staged design where this extension applies.

---

## 1. Motivation

The relay-tier extension privileges a relay set chosen by the topic owner. That model fits use cases where a single curator naturally exists (notifications via wallet backends, where the topic owner can list the wallet providers). It does not fit every use case.

### 1.1 Some use cases lack a single curator

The notification cluster has wallet-backend providers as a natural infrastructure tier; the topic owner can list them. Other use cases — most notably SPO emergency alerts (UC-1) — do not have a single curator that is sufficient on its own. Recipients are SPO node operators themselves. While certain ecosystem entities (Cardano Foundation, IOG, Intersect) could run dedicated relay nodes and be registered in the topic registry, individual SPOs also have their own peer-to-peer trust relationships that the protocol should be able to reflect alongside any centrally listed relays.

### 1.2 Operator-decided trust is a real and valuable signal

SPOs participate in informal trust networks: monitoring partnerships, geographic redundancy groups, operator collectives, mutual incident-response arrangements. Each operator typically knows a handful of specific peers they trust to deliver messages reliably. The protocol's topology should be able to incorporate this signal without requiring a global curator to discover or attest to it.

### 1.3 The combined argument

If a deployment must contend with adversarial behavior in the open gossip overlay, and if some recipients have local trust relationships independent of any global curator, then incorporating these local trust relationships into the topology — without removing the eclipse-resistance properties of the random layer — bounds adversarial impact along an additional axis: the recipient's own trust choices. Adversarial behavior outside the recipient's chosen trust set affects the random-link delivery path; adversarial behavior by a chosen peer is bounded to that recipient's own decisions and does not spill over to other nodes.

---

## 2. Proposal

**Add a local relays table to each edge node, and a reciprocal-link establishment mechanism.** The base AUEB three-layer dissemination — Vicinity-clustered Harary ring plus random links — is unchanged. Mutual-trust links are *additive*.

### 2.1 Local relays table

Each edge node maintains a local relays table. Entries can come from two sources:

- **Owner-attested.** Entries imported from the topic registry per the relay-tier extension. These are global, applicable to all subscribers of the topic.
- **Self-attested.** Entries the node's own operator has chosen to add. These are local to the node and reflect operator-decided trust.

The table format is the same regardless of source; the source is metadata on each entry. Self-attested entries follow the privacy policy in §2.6.

### 2.2 Reciprocal-link establishment

When edge node A has B in its local relays table, A may attempt to establish a mutual link with B as follows:

1. **Request.** A constructs a signed message containing A's descriptor (key, address, capability flags) and B's identity. A signs the bundle with its own identity key. The signed binding to B's identity prevents the request from being replayed to other peers.
2. **Verification.** B receives the request. B verifies the signature, confirms its own identity is the bound recipient, and checks whether A is in B's own local relays table.
3. **Acceptance.** If A is in B's table, B accepts and the two form a bidirectional link added to the dissemination overlay's link set. Both endpoints retain the link until either side tears it down.
4. **Silent drop.** If A is not in B's table, B drops the request silently. No response is required.

The mechanism establishes a link if and only if both endpoints have independently chosen to trust the other. An adversary cannot unilaterally insert links into honest-honest pairs.

### 2.3 Freshness

To prevent replay across time (an old request being accepted after a removed-and-re-added entry), the request includes either a fresh nonce supplied by B in a prior exchange (challenge-response), a recent timestamp with a tight expiry window, or a recent chain reference. The exact freshness mechanism is left to the protocol implementation; the requirement is that the request cannot be replayed indefinitely.

### 2.4 Forwarding semantics over mutual links

Once a mutual link is established, the default forwarding rule is **symmetric all-shared-topic forwarding**: each side forwards to the other every message it receives on any topic both sides subscribe to. The link is already gated by mutual consent; layering further fanout discretion on top would weaken its value.

Two alternative semantics are protocol-compatible and may be used per deployment:

- **Priority within fanout-`k`.** Mutual links don't override fanout; they give the linked peer priority when picking who to push to.
- **Topic-scoped link.** The link carries only topics where both have explicitly opted in via per-topic tables.

The protocol prescribes that mutual links exist and that consent is bilateral; it does not mandate a single forwarding rule.

### 2.5 Liveness and teardown

Mutual links require a heartbeat to maintain liveness. If either endpoint stops responding for a configurable window, the other side considers the link dropped. Explicit teardown is also supported: A can signal removal to B if A removes B from its table.

### 2.6 Privacy of the table

The table publication model is a deployment choice:

- **Public tables.** The local relays table is gossiped as part of the node's descriptor (or separate messages). Other nodes can map the trust topology. Suitable for deployments where transparency is desirable (e.g., SPO operations, where peer trust is already a public-facing notion).
- **Private tables.** The table is revealed only during the handshake. Other nodes learn whether they are in a peer's table only by attempting the handshake. More privacy-preserving but less amenable to proactive route optimization.

### 2.7 Table size bound

Each table is bounded by a configurable `K` (e.g., 16 to 32). This caps the per-node mutual-link count, the bandwidth amplification a single misbehaving node can cause, and the maintenance cost of the table.

### 2.8 Bootstrap

How A and B come to add each other in the first place is **out of protocol scope**. It is an operator-level decision driven by social, organizational, or operational relationships. The protocol provides the table mechanism for operators to express their decisions; it does not automate the discovery.

---

## 3. Topology Properties

The mutual-link layer sits on top of the AUEB three-layer protocol, producing three coexisting link classes:

| Link class | Source | Property |
|---|---|---|
| Ring | Deterministic from node ID (Harary positioning) | Structured neighbors; predictable but vulnerable to grinding (S-10) |
| Random | SecureCyclon peer sampling | Probabilistic coverage; eclipse-resistant within bounded adversary fraction |
| Mutual | Bilateral table consent | High-trust delivery within the consenting subgraph |

These classes are complementary:

- The **random layer** gives eclipse resistance against an adversary the recipient does not get to choose.
- The **mutual layer** gives delivery confidence against an adversary the recipient can choose around.

In settings where mutual-trust naturally clusters (SPO operators in redundancy groups, monitoring partnerships, geographic affinities), the resulting topology may exhibit small-world properties — high local clustering plus occasional long-distance links between operators known to each other across clusters. The structural argument for this is plausible, but it should be validated empirically rather than assumed.

---

## 4. Composition with the Relay-Tier Extension

Both extensions can be deployed independently or together:

| Configuration | Topology shape |
|---|---|
| Neither extension | Base AUEB three-layer; ring + random links only |
| Relay tier only | Base + publisher→relay direct + relay→edge fanout-`k` |
| Local relays only | Base + mutual-trust links among self-attesting peer pairs |
| Both | All of the above |

A deployment running both extensions exposes the local relays table to entries from either source. Owner-attested entries enable the publisher→relay→edge fanout path; self-attested entries enable the mutual reciprocal-link path. They serve distinct purposes and do not conflict.

The combined deployment fits use cases like UC-1 (SPO alerts), where:

- Ecosystem entities (CF, IOG, Intersect) may run dedicated relay nodes and be registered as owner-attested relays for the topic.
- SPO operators may add other trusted SPOs to their local tables, forming mutual links among operator peers.
- Both kinds of relay coexist in each SPO's local table; the protocol treats them uniformly as "peers I can receive from."

---

## 5. Research Questions for Collaboration

We would value research input on:

1. **Empirical topology properties of the mutual-link overlay.** Under realistic assumptions about operator trust networks (clustering coefficients, average degree), what dissemination latency does the combined ring + random + mutual topology achieve? Does the small-world conjecture hold, or does the mutual-link layer produce something less favorable in practice?

2. **Composition with SecureCyclon's view replacement.** Should mutual-link partners be excluded from random-sample views (because they are already known and connected), or included as a freshness signal? The choice affects the effective adversary fraction in the random layer.

3. **Composition with Vicinity's clustering.** Should Vicinity's same-topic clustering bias toward peers that share a mutual-link partner? This could strengthen the topic overlay along trusted edges without sacrificing random sampling — but it also reduces the diversity of the topic neighbourhood.

4. **Privacy of self-attested entries.** What information leakage is acceptable in deployments where the table is gossiped publicly, and what handshake-only protocol provides equivalent topology benefits without disclosure?

5. **Anti-flood at the request boundary.** Even though B silently drops requests from non-table peers, B still spends CPU on signature verification. What rate-limiting mechanism (per-IP, per-identity, proof-of-work) is appropriate to prevent CPU exhaustion via bogus requests?

6. **Reciprocity and emergent incentives.** Mutual links naturally encode reciprocity; their prevalence is itself a soft incentive for honest cooperation. Does sufficient mutual-link density reduce the need for explicit slashing or stake-based incentives in stake-bound deployments? Quantifying this would inform the broader incentive design.

---

## References

- *Relay Tier Extension Proposal* (April 2026). [relay-tier-extension-proposal.md](relay-tier-extension-proposal.md).
- *AUEB Gap Analysis* (April 2026). [aueb-gap-analysis-final.md](aueb-gap-analysis-final.md).
- *Actor & Use Case Analysis* (April 2026). [actor-use-case-analysis.md](actor-use-case-analysis.md).
- *PubSub Staged Design Synthesis* (April 2026). [staged-design-synthesis.md](staged-design-synthesis.md).
- *Cardano Pub/Sub Framework: Design and Architecture* (D2). AUEB / IOG Research, 2024. [D2-Cardano-PubSub-Framework-Design-and-Architecture.pdf](D2-Cardano-PubSub-Framework-Design-and-Architecture.pdf).

# Protocol Extension Proposal — Relay Tier on the Gossip Overlay

**Date:** April 2026
**Status:** Draft for discussion with the AUEB research team

## Purpose

This document records an extension to the AUEB three-layer dissemination protocol that we have been entertaining in the Cardano PubSub workstream and would like to discuss with the protocol's authors. It is not a settled design — the intent is to share the shape of the idea, the threat-model rationale that motivated it, and the research questions where collaboration would sharpen the proposal.

The extension is intended to compose with the existing three-layer protocol (peer sampling via SecureCyclon, navigation and dissemination via Vicinity over a Harary ring with random links). Relay links are *additive* to the existing topology, not a replacement.

Companion documents in this workstream:

- [AUEB Gap Analysis](aueb-gap-analysis-final.md) — observations from our review of D2 and the SecureCyclon / Vicinity papers.
- [Actor & Use Case Analysis](actor-use-case-analysis.md) — actor and use-case decomposition that surfaces the wallet-backend subscriber set.
- [PubSub Technical Review](pubsub-technical-review-march-with-links.md) — March 2026 review converging on the notification primitive.

---

## 1. Motivation

Three observations from the analyses above converge.

### 1.1 The three-layer protocol is structurally cooperative

Both our gap analysis and the PRISM modeling work in this workstream show that the protocol's delivery properties rest on benign or only mildly adversarial behavior at the gossip layer. Once an adversary engages in any of the following, no detection or recovery mechanism is specified:

- **Selective forwarding (S-02)** — a node correctly embedded in SecureCyclon, passing every certificate check, silently drops or delays messages it is supposed to relay; the protocol cannot tell.
- **False topic-membership advertising (S-01)** — a node claims membership in topics it does not subscribe to in order to be picked as a navigation-layer routing hub for those topics, then steers victims toward attacker-controlled neighbours.
- **Lack of Sybil resistance (S-04)** — identity creation is a microsecond-cheap key generation, so the bounded-adversary-fraction assumption SecureCyclon's analysis depends on does not hold in an open setting.
- **Targeted ring-position grinding (S-10)** — Harary ring positions are a deterministic public function of node ID, so an adversary can mine identities until one falls adjacent to a chosen victim on a chosen topic, enabling per-victim eclipse for seconds-to-minutes of compute.

PRISM modeling demonstrated that eclipsing a target node is not particularly difficult: modest adversary fractions and the identity-grinding budgets described under S-10 suffice to position adversaries adjacent to a victim on a topic's ring. The Critical and High findings concentrated around the open gossip overlay describe failure modes that an open-deployment adversary can readily target, not corner cases.

We are concerned that a protocol with these properties, deployed in the open, will face attacks (selective drop, eclipse, ring-position grinding) that the current design has no answer for.

### 1.2 Some use cases have an identifiable, essential subscriber subset

Our actor and use-case analysis and the March 2026 technical review independently found that for the notification cluster (UC-2 through UC-6 — SPOs/dReps → delegators, dApps → users, governance → community, DAOs → token holders), there is a small, identifiable set of subscribers — wallet infrastructure providers (~10 major backends) — whose participation is *essential* for the use case to deliver value. End users themselves are out of pubsub scope; they reach the system through these intermediaries.

That is, the priority use cases come with a known critical subscriber subset built in.

### 1.3 Project priority is on notifications

The CEO has signalled prioritization of notification use cases (SPO/dRep → delegators, dApps → users). For these specifically, the wallet-backend set is well-defined and load-bearing.

### 1.4 The combined argument

If a deployed protocol must contend with adversarial behavior in an open gossip overlay, and if the priority use cases have a known critical subscriber subset, then privileging that subset in the overlay topology is a candidate way to *bound* adversarial impact for the recipients that matter most — without giving up the open, decentralized overlay for other subscribers.

---

## 2. Proposal

**Add a relay tier to the topic registry and to the gossip overlay's link structure.** The base AUEB three-layer dissemination — Vicinity-clustered Harary ring plus random links — is unchanged. Relay links are *additive*.

### 2.1 Registry change

Each topic record gains an optional **relay list**: an owner-attested set of node identities (key + address) that the publisher commits to push messages to directly. The list may be empty; not every topic needs one. For the notification use cases, the relay list is the wallet-backend set. For other use cases, it may be empty or name a different set if one exists.

### 2.2 Topology change

The extension overlays three additional link classes on top of the base dissemination topology:

1. **Publisher → relay direct links.** For each topic, the publisher establishes outgoing links to every relay in the registry list and pushes each signed message directly. Delivery from publisher to the relay set is then guaranteed, contingent only on individual relay availability.
2. **Edge node → relay subscription request.** A gossip peer not in the relay list (an "edge node") may request that one or more relays add an outgoing link to it. Relays accept such requests subject to a local policy (anti-flood, fanout cap, per-IP/per-identity limits). Once accepted, the relay treats the edge node as a downstream of its relay-fanout policy.
3. **Relay fanout forwarding.** Each relay carries a fanout parameter `k`. On receiving a message — whether via the direct publisher path or via the gossip overlay — the relay forwards it to at most `k` of its registered downstream subscribers. Selection is local policy:
   - If registered downstream count `n ≤ k`, forward to all of them.
   - If `n > k`, select `k` per a forwarder-local rule (random, round-robin, latency-weighted, etc.). The protocol prescribes the semantics — fanout exists and is bounded — not the rule itself.

The base Harary ring + random Vicinity links among same-topic peers continue to operate. Relay-induced edges are added on top of, not in place of, those.

### 2.3 Bootstrapping side benefit

Relays may also serve as bootstrap contacts for the gossip overlay: a node joining a topic's overlay can use the registry-listed relays as its first SecureCyclon contacts, sidestepping the unspecified bootstrap problem (I-10 — the report does not describe how a new node obtains its initial set of contacts or how the certificate-chain requirement interacts with the join process). This is a side benefit of the same registry change rather than a separate design.

### 2.4 Notes on the relay role

Two observations on the relay role, listed here for completeness and to be refined in future iterations of this proposal:

- **Relationship to AUEB's replication-server role.** A node may eventually implement both relay duty and the AUEB-defined replication-server role; the relationship is a deployment choice and a question of capability flags, not a fixed protocol identity. The relay role as defined here is narrower than the replication-server role — push-only, no required history retention or catch-up serving — but the two are not mutually exclusive.
- **Optional participation in SecureCyclon and Vicinity.** Relays are reachable from publishers via the on-chain registry and do not need to participate in SecureCyclon or Vicinity to fulfill the relay duty. Optional participation is allowed and may be desirable in deployments where edge-node operators prefer relays as trust sources for their peer-sampling views, or where relays already serve as bootstrap contacts (§2.3).

---

## 3. Intended Properties

The two-step delivery argument the extension supports:

1. **Publisher → relay set is guaranteed** (modulo individual relay availability), because the path is direct push and the relay set is owner-attested on chain.
2. **Relay set → honest gossip peers is best-effort but raised**, because every honest peer registered with at least one honest relay receives the message via that direct relay→downstream link in addition to whatever the gossip layer delivers. Adversarial subgraphs in the gossip overlay are not bypassed but are *complemented* by topology elements the adversary does not control.

For wallet-backed end users — the dominant recipient population for the notification use cases — step (1) is sufficient: their delivery runs publisher → relay (= wallet backend) → user through the wallet's own application channel, and the gossip overlay is irrelevant to them. For non-wallet-backed gossip peers, step (2) raises delivery probability above the pure-gossip baseline.

---

## 4. Scale Behaviour

The relative impact of the extension is a function of network size relative to aggregate relay fanout `R · k` (relays times fanout):

- **Small topics** (subscriber count `≤ R · k`): relays can forward to all or most subscribers directly. The gossip overlay becomes largely redundant; the extension dominates delivery, and reliability approaches that of direct push.
- **Mid-size topics**: relay paths cover a meaningful fraction; the gossip overlay covers the rest, with relay-injected messages also helping seed the overlay.
- **Large topics** (subscriber count `≫ R · k`): only a small fraction of subscribers receive via the relay path; bulk dissemination falls back on the base three-layer protocol with its existing properties and gaps.

The priority notification use cases sit in the small-topic regime once end users are correctly accounted as out-of-pubsub: the recipient set under the protocol is the ~10 major wallet backends plus a tail of independent gossip peers, not the millions of end users (reached via the wallet's own channel). The extension is well-suited to that regime — it offers a better balance between delivery guarantees and decentralization than either pure direct push or pure gossip alone, with a small footprint of additional mechanism. For use cases that genuinely scale to many independent subscribers, the extension shifts from "dominant delivery path" to "useful seeding aid"; it does not aim to replace the base protocol at large scale.

---

## 5. Research Questions for Collaboration

We would value research input on the following:

1. **Incentives for relays.** The wallet-backend case has intrinsic incentive (relays need the messages to serve their users), but the extension's general form does not. What incentive designs make the relay role sustainable when intrinsic incentive is absent? Is the security-deposit pattern proposed for the replication-server role transferable, or does this role require a different model?

2. **Economic punishment for misbehaviour.** Relays that censor, drop, or selectively forward represent a more concentrated single point of failure than ordinary gossip peers do. What detection and slashing mechanisms are feasible given the on-chain registry foothold? In particular, is selective forwarding by a relay detectable in any constructive way (cross-relay reconciliation, statistical audit, complaint protocols), or does it inherit S-02's structural undetectability — the property that a correctly-embedded node can drop messages without leaving any evidence the protocol can act on?

3. **Edge-node registration anti-flood.** What identity-binding or rate-limiting mechanism for edge-node registration with relays is consistent with SecureCyclon's certificate model? The general Sybil resistance question (S-04 — no identity cost is specified for general gossip participants, so an adversary can generate unlimited identities and violate the bounded-adversary-fraction assumption) reappears at the registration boundary specifically.

4. **Interaction with SecureCyclon and Vicinity invariants.** Relay-injected links are not produced by SecureCyclon's certificate-chained sampling. What is the impact on SecureCyclon's view convergence and on Vicinity's clustering of having a privileged link class that bypasses the sampling layer? Are there configurations where the relay tier degrades the base protocol's empirical properties rather than improving the system?

---

## References

- *AUEB Gap Analysis* (April 2026). [aueb-gap-analysis-final.md](aueb-gap-analysis-final.md).
- *Actor & Use Case Analysis* (April 2026). [actor-use-case-analysis.md](actor-use-case-analysis.md).
- *PubSub Technical Review* (March 2026). [pubsub-technical-review-march-with-links.md](pubsub-technical-review-march-with-links.md).
- *Cardano Pub/Sub Framework: Design and Architecture* (D2). AUEB / IOG Research, 2024. [D2-Cardano-PubSub-Framework-Design-and-Architecture.pdf](D2-Cardano-PubSub-Framework-Design-and-Architecture.pdf).

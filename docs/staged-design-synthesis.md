# PubSub Staged Design — Synthesis Proposal

**Date:** April 2026
**Status:** Draft proposal for review

## Purpose

This document proposes a staged design for the Cardano PubSub protocol. It synthesizes two earlier positions — a use-case-driven simple design and a generalized three-layer protocol — into a single architecture that ships incrementally: each stage delivers value standalone, and each later stage adds a layer rather than replacing the previous one.

This document complements rather than supersedes:

- [AUEB Gap Analysis](aueb-gap-analysis-final.md) — referenced throughout for which gaps activate at which stage.
- [Actor & Use Case Analysis](actor-use-case-analysis.md) — UC-N references in this doc point to that document.
- [Relay Tier Extension Proposal](relay-tier-extension-proposal.md) — extension to the AUEB three-layer protocol that introduces owner-attested relays (publisher→relay direct links, edge-node→relay subscription, relay fanout forwarding). The mechanics summarized in this design's Stage 2 are specified rigorously there.
- [Local Relays Extension Proposal](local-relays-extension-proposal.md) — second extension that adds operator-attested local relay tables and reciprocal mutual-trust links. Composes with the relay-tier extension. The canonical motivating use case is UC-1 (SPO alerts).

---

## TL;DR

A single node-style application implements a pubsub protocol whose **topology** evolves across stages and varies by use case.

The wallet-backend notification cluster (UC-2..UC-6) follows a two-stage progression:

- **Stage 1.** Publishers push directly to a known set of trusted **relays** (wallet backends). No gossip layer, no peer sampling, no topic discovery. Relays fan out to end users via their existing app channels (out of protocol scope).
- **Stage 2.** Edge nodes join opt-in, register interest with relays, and form a decentralized overlay using SecureCyclon + Vicinity-style dissemination among themselves. Relays inject messages into the overlay; the primary delivery path to wallet backends remains unchanged.

UC-1 (SPO emergency alerts) does not fit the wallet-backend pattern. Its recipient set is the SPO node operator set itself, with no wallet-backend intermediary that a topic owner could list as a relay set covering all subscribers. UC-1 deploys the full AUEB three-layer protocol with both compositional extensions ([§5](#5-uc-1-spo-alerts--distinct-topology-track)): the relay-tier extension (with ecosystem entities such as CF, IOG, and Intersect as candidate owner-attested relays) plus the local-relays extension (SPO operators add other trusted SPOs to their own tables; mutual-trust pairs form bidirectional links). The stake-bound, on-chain-identified SPO setting plus the mutual-consent link semantics reduce several AUEB gap concerns substantially.

**Deferred.** Replication-server role for offline catch-up. Incentives for non-wallet relays and edge nodes.

Cross-cutting primitives (on-chain topic registry, hash-chained signed messages, identity model) are common to all tracks and stages and ship first.

**Why staging matters for security.** Most Critical/High findings in the AUEB gap analysis target specific layers — the gossip overlay or the persistence layer. By **deferring those layers until use cases require them**, the design defers the corresponding findings: the wallet-backend Stage 1 deploys neither layer, so their attack surface does not apply; Stage 2 introduces the gossip overlay as an *additive* path that does not threaten the primary publisher→relay→wallet-backend delivery; persistence stays deferred across both stages. UC-1 deploys the full overlay from day one but in a setting where stake-bound identity and mutual-link consent reduce the gap surface materially. See [§7](#7-gap-inheritance-across-stages) for the per-track gap-inheritance map.

**Why staging matters for delivery.** The same separation lets research and engineering progress in parallel. Engineering ships Stage 1 — a finite, well-scoped piece of work using primitives that are already understood — while research continues to address the gap-analysis findings that Stage 2 will activate (Sybil resistance, certificate-chain extension to the Vicinity layers, ring-position grinding, inter-layer timing, and so on). Engineering does not sit idle waiting for the full security analysis to converge, and research is not pressured to cut corners to unblock a release.

---

## 1. Actor Model

The protocol distinguishes four actor classes. Stage 1 uses two; later stages add the others.

| Actor | Role | Stage introduced |
|---|---|---|
| **Publisher** | Authorized origin of messages on a topic. Listed in the on-chain registry per topic. | Stage 1 |
| **Relay** | Trusted infrastructure that receives messages from publishers and propagates them. In Stage 1 these are wallet backends; in later stages the relay set may broaden subject to incentive design. | Stage 1 |
| **Edge node** | Open-decentralization participant. Registers interest with one or more relays to receive messages, and gossips with other peers to propagate further. | Stage 2 |
| **End user** | Out of pubsub-protocol scope. Receives messages via the relay's existing application channel (push notification from wallet, etc.). | Out of protocol scope |

**Notes on the actor model:**

- The distinction between relays and edge nodes is a **trust boundary**, not a capability boundary. Relays are listed (or owner-attested) in the registry; edge nodes self-register interest with relays. The protocol assumes adversarial behavior at the edge-node layer and trusted behavior at the relay layer (with the understanding that "trusted" here means "small-N, identifiable, business-incentivized," not "Byzantine-resistant").

End users are explicitly out of pubsub scope. The protocol's job ends at the wallet backend; fanout to users is a wallet-application concern, with consent and filtering handled at that layer.

---

## 2. Cross-Cutting Primitives

These ship in Stage 1 and are common to all subsequent stages.

### 2.1 Topic Registry (On-Chain)

The topic registry is implemented as an on-chain smart contract (Quint-modelled, Plutus/Aiken implementation pending). Its responsibilities:

- **Topic existence and ownership.** Each topic has one or more owners (key set, possibly multisig). Owners can update the topic's parameters, including publisher and relay lists.
- **Authorized publishers.** The set of public keys whose signed messages are considered valid for a topic. May be publisher-list-based (Stage 1 default) or open with rate-limit policy (deferred to anti-spam design).
- **Relay list per topic.** Owner-attested set of relays that publishers commit to push through. In Stage 1 this is the explicit set of wallet backends a publisher pushes to. In Stage 2 this set may grow to include non-wallet trusted relays, gated by registration policy (open question).

The registry is coordination infrastructure, not identity infrastructure. The known [S-13 gap](aueb-gap-analysis-final.md#s-13--the-topic-registry-provides-coordination-not-identity-trust) — the registry cannot verify that the entity registering a topic is who it claims to be — applies here too. A trust layer above the registry (curated metadata, similar to Cardano native token metadata curation) is required for any deployment where topic identity matters. This is acknowledged as an out-of-protocol dependency.

### 2.2 Identity

The protocol uses plain public keys throughout. The on-chain registry holds keys; it does not record what those keys represent or attest to any binding between them and other credentials. Mapping a registered key to a real-world entity is the job of a separate **trust layer** that lives off-chain — similar in shape to the curated metadata registry that maps Cardano native-token policy IDs to verified issuer information — and is out of protocol scope. Keeping the protocol agnostic about identity binding avoids per-topic verification logic in a network that hosts all topics uniformly.

- **Publishers** register a public key (or multisig key set) per topic in the topic registry.
- **Edge nodes** choose what descriptor (in SecureCyclon / Vicinity terminology) to use in the protocol. The descriptor carries the operator-generated key and contact information, and the operator controls it.
- **Relays** are identified by their descriptors — registered in the topic record for owner-attested relays, or in an edge node's local relays table for self-attested entries. Descriptor rotation requires the relevant table update, not a redeploy of upstream publishers.

**Future direction: decentralized trust-table sharing.** A later stage could let topics share the off-chain trust-table mapping over the network itself — for example, via Vicinity exchanges in the dissemination layer or a dedicated record-exchange channel. The exchanged records would still require a topic-specific **sidecar plugin** to validate the attested identities. For SPO alerts (UC-1), such a plugin could read the exchanged records and verify them against the on-chain stake distribution and SPO credentials. Keeping the validation logic in a per-topic sidecar plugin preserves the core-protocol simplicity while letting topics extend their trust tables in a more decentralized manner than relying on a single curated server. A topic could start with a centrally-operated trust table (e.g., for UC-1 maintained by IOG / Intersect / an SPO collective) and evolve to a decentralized arrangement over time.

DID-based identity is a possible upgrade path if richer identity semantics (key rotation, delegation, portable reputation) become valuable (per [Architecture Building Blocks Tier 2](architecture-building-blocks.md#tier-2--nice-to-have-strengthens-base-adds-resilience)). Not required for Stage 1.

### 2.3 Message Envelope

Every message carries:

| Field | Meaning |
|---|---|
| `topicId` | Topic identifier (resolves against the registry) |
| `publisherId` | Publisher key identifier (must match an authorized publisher for `topicId`) |
| `parentHash` | Hash of the previous message from the same `publisherId` on the same `topicId`. Empty for the publisher's first message on the topic. |
| `sequence` | Monotonically increasing per `(topicId, publisherId)`. Bound to the signature. |
| `timestamp` | Publisher-assigned timestamp. Advisory only; not used for security-critical decisions (see [§7](#7-gap-inheritance-across-stages) on S-15). |
| `payload` | Application-specific message body. |
| `signature` | Publisher signature over `(topicId, publisherId, parentHash, sequence, timestamp, payload)`. |

The envelope is signed with a single long-term publisher key per topic (plain CT-log style; no key rotation inside the chain). Three properties follow:

1. **`sequence` enables ordering, catch-up, and gap detection.** Sequence numbers are the lookup key for AUEB-style replication-server catch-up queries — a query keyed on `(topicId, publisherId, sequence)` per D2 Ch.4 — so a subscriber can request a specific missed message by number. Gap detection is straightforward integer comparison once a subscriber knows the publisher's expected current sequence.

2. **`parentHash` enables chain-extension verification.** Each message commits to its predecessor's hash, so a subscriber can verify that an incoming message extends the chain it already holds. A parent-hash mismatch flags an inconsistency (gap or fork) from a single incoming message, without the subscriber needing to also hold the diverging branch. When both branches of a fork are observed, two messages with the same `(publisherId, parentHash)` and different content yield an explicit equivocation proof.

3. **Signature scope binds the envelope to topic and sequence.** Because the signature covers `topicId`, `parentHash`, and `sequence`, cross-topic replay ([I-17](aueb-gap-analysis-final.md#part-ii--implementation-level-observations)) and replication-server reordering ([I-14](aueb-gap-analysis-final.md#part-ii--implementation-level-observations)) are closed by construction — a signed message cannot be repurposed for a different topic or moved to a different sequence.

Late joiners reconstruct prior messages via the deferred replication mechanism; chain integrity is verified locally by parent-hash linkage and signature checks.

### 2.4 Relay List

Relays for a topic are listed in the registry by the topic owner. In Stage 1 this is the wallet-backend set negotiated off-chain and registered on-chain. In Stage 2 the list may include additional trusted relays. The registry entry's semantics is "the publisher commits to relaying messages through these relays" — a contract, not a self-attestation.

Wallet backends do not self-register. (See [§9 open questions](#9-open-questions) on whether self-registered "I subscribe" entries have value.)

---

## 3. Stage 1 — Direct Delivery to Relays

### 3.1 Topology

```
                 ┌──→ Relay R₁ ──→ wallet users
Publisher P ─────┼──→ Relay R₂ ──→ wallet users
                 └──→ Relay R₃ ──→ wallet users
```

For each topic, the publisher reads the registry's relay list and pushes each signed message to every listed relay. Relays verify the signature and chain reference, then hand the message to their application layer (wallet backend logic) for fanout to users.

There is no gossip, no peer sampling, no topic discovery, no replication. The topology is a fanout from a small known publisher to a small known relay set.

### 3.2 What Stage 1 Provides

- **Authentication.** Every message is verifiable against a registry-listed publisher key. Identity trust is delegated to the topic owner's curation.
- **Integrity.** Chain hashes detect omitted, reordered, or substituted messages.
- **Consistency.** Equivocation by a publisher (two messages with the same parent) is provable.
- **Best-effort delivery.** Each relay receives every message the publisher emits. Fanout from relay to end users is the wallet's responsibility.
- **Operational resilience.** Relays are independent; one failing does not block delivery to the others.

### 3.3 What Stage 1 Does Not Provide

- **No catch-up.** A relay offline when message N is pushed will not receive N unless the publisher retains and re-pushes. Stage 1 assumes either (a) wallet backends maintain high uptime relative to message frequency, (b) publishers retain a short outbound buffer and retry on transient connection failure, or (c) the deferred replication stage handles longer outages. No protocol-level catch-up.
- **No decentralized propagation.** Wallet backends are listed by topic owners; non-wallet recipients have no protocol-level path to receive messages until Stage 2.
- **No anti-spam beyond signature check.** Authorized publishers can publish at any rate; rate limiting is the topic owner's responsibility (rate caps in the registry, enforced at the relay).

### 3.4 Use Cases Served by Stage 1 (Wallet-Backend Track)

Stage 1 is sufficient for the wallet-backend notification cluster identified in the [Actor & Use Case Analysis](actor-use-case-analysis.md):

- **UC-2** (SPOs → delegators), **UC-3** (Governance → community), **UC-4** (dReps → delegators), **UC-5** (DApps → users), **UC-6** (DAOs → token holders). All collapse into the publisher → wallet-backend → user pattern. Wallet backends already know which users care about which SPOs / dReps / dApps and filter accordingly at the application layer.

UC-1 (protocol developer teams → SPOs: emergency alerts) does not fit this pattern — its recipients are SPO node operators with no wallet-backend intermediary. UC-1 is covered separately in [§5](#5-uc-1-spo-alerts--distinct-topology-track).

Use cases **out of scope for the protocol entirely**:

- **UC-7** (DeFi intents) — exclusive value, rational suppression breaks gossip ([S-18](aueb-gap-analysis-final.md#s-18--the-protocol-is-not-equipped-for-messages-with-intrinsic-private-value)). Direct-to-solver model is appropriate.
- **UC-8** (high-frequency agent coordination) — exceeds notification scale; needs a different transport.
- **UC-9 through UC-13** — small-N coordination problems (Sundae, Hydra, multisig) better solved as direct messaging or RFQ flows than as pubsub.

### 3.5 Implementation Footprint

| Component | Status |
|---|---|
| Topic registry contract | Quint-modelled; Plutus/Aiken implementation needed |
| Publisher signing + envelope | Specification needed (per §2.3) |
| Relay push protocol | Specification needed: connection model, retry, backpressure |
| Relay-side verification library | Signature check + chain validation |
| Registry-derived peer config | CLI / library to read registry and connect |

Stage 1 is a finite, well-scoped engineering effort. No research-grade primitives are required.

---

## 4. Stage 2 — Decentralized Gossip Overlay

### 4.1 Topology

Stage 2 introduces edge nodes as an opt-in second class of recipients alongside the listed relays.

```
                 ┌──→ Relay R₁ ──→ wallet users
Publisher P ─────┼──→ Relay R₂ ──→ wallet users
                 └──→ Relay R₃ ──→ wallet users
                            │
                            ├──→ Edge node E₁ ──┐
                            ├──→ Edge node E₂ ──┼─ Vicinity dissemination
                            ├──→ Edge node E₃ ──┤   among edge nodes
                            └──→ Edge node Eₖ ──┘  (SecureCyclon for sampling)
```

The publisher → relay path is unchanged from Stage 1. Relays gain a new responsibility: pushing to edge nodes that have registered interest. Edge nodes form an overlay among themselves using SecureCyclon (peer sampling) and Vicinity (topic-clustered dissemination), as specified in the AUEB three-layer protocol.

**Critical invariant: the wallet-backend delivery path does not depend on the gossip overlay.** Even if the gossip overlay is fully eclipsed, Sybiled, or partitioned, wallet backends continue to receive messages directly from publishers. The gossip overlay is **additive**, serving recipients who want decentralized delivery without requiring trust in any specific wallet backend.

### 4.2 Relay → Edge-Node Push

Edge nodes register interest with one or more relays; relays push received messages to a subset of their registered edge nodes per a local fanout-`k` policy. The three link classes (publisher→relay direct, edge-node→relay subscription request, relay fanout forwarding) and registration semantics are specified in [Relay Tier Extension Proposal §2.2](relay-tier-extension-proposal.md#22-topology-change).

The protocol prescribes that fanout exists and is bounded; it does not prescribe the specific selection rule (random, round-robin, latency-weighted, etc.) — that is a relay-local capacity decision. For small registered sets the natural choice is eager (push to all); for large sets random-subset push (push to `k` of `n`, rely on the gossip overlay to propagate the rest) is the standard shape.

### 4.3 Edge-Node ↔ Edge-Node Dissemination

Edge nodes exchange messages among themselves using the AUEB three-layer dissemination as specified — peer sampling (SecureCyclon), navigation, and dissemination (Vicinity for same-topic clustering and ring formation). The navigation layer is retained.

There is one possibility worth evaluating: for topics whose registry record carries an owner-attested relay list with descriptors, an edge node joining that topic could **bootstrap directly from the listed relays** rather than waiting for navigation-layer discovery to find same-topic peers. This would shorten the time-to-first-message for new joiners on registry-attested topics without removing the navigation layer for cases where it is needed (topics without relay lists, fallback paths, multi-topic discovery). Whether and how to formalize this shortcut — and how it interacts with SecureCyclon's certificate-chain expectations and Vicinity's clustering — is an open research question.

### 4.4 Deduplication and Convergence

A edge node may receive the same message via relay push *and* via gossip from another peer. Deduplication uses the message's chain hash as the natural key — no protocol-level state beyond a recently-seen cache. Convergence properties of the gossip overlay are inherited from the AUEB analysis with the limitations the gap analysis identifies (see [§7](#7-gap-inheritance-across-stages)).

### 4.5 What Stage 2 Adds

- **Decentralized recipient path.** A user who does not want to depend on a wallet backend can run an edge node (or use a wallet that internally runs one) and receive messages directly via the overlay.
- **Resilience to relay collusion.** If all listed relays for a topic colluded to censor, edge nodes that received the message from any one relay before the collusion can still propagate it. This is a probabilistic improvement, not a guarantee. The publisher can also publish messages to edge-nodes using the dissemination layer in parallel with the relays.

### 4.6 What Stage 2 Still Does Not Provide

- **No catch-up.** Same as Stage 1.
- **No identity-grade Sybil resistance for edge nodes.** Inherited from the AUEB protocol; see [S-04](aueb-gap-analysis-final.md#s-04--no-sybil-resistance-for-gossip-layer-participants). Stage 2 mitigates impact via the additive-path property: Sybil in the overlay does not affect wallet-backend delivery.
- **No incentive for non-wallet relays.** Stage 2's relays are still wallet backends with intrinsic incentive. Adding non-wallet relays requires the deferred incentive design.

---

## 5. UC-1: SPO Alerts — Distinct Topology Track

UC-1 (protocol developer teams → SPOs: emergency alerts) does not follow the wallet-backend track described in §3 and §4. The recipient set is SPO node operators directly; there is no wallet-backend intermediary that the topic owner can list as a relay set covering all subscribers. UC-1 instead deploys the standard AUEB three-layer protocol with two compositional extensions, riding on top of the cross-cutting primitives described in §2.

### 5.1 Topology

UC-1 runs the full AUEB three-layer protocol (peer sampling, navigation, dissemination) augmented with both protocol extensions developed in this workstream:

- **Relay-tier extension** ([Relay Tier Extension Proposal](relay-tier-extension-proposal.md)). Ecosystem entities such as the Cardano Foundation, IOG, and Intersect may run dedicated relay nodes and register them as owner-attested relays for the SPO alerts topic. The resulting links are **unidirectional**: publishers push to relays directly, and relays push to subscribed edge nodes per a local fanout-`k` policy. Edge nodes (here, individual SPO nodes) register interest with the listed relays.
- **Local relays + reciprocal mutual-trust links** ([Local Relays Extension Proposal](local-relays-extension-proposal.md)). SPO operators add other trusted SPOs to their own local relays tables. When two operators each have the other in their respective tables, the reciprocal handshake establishes a **bidirectional** link: each side forwards to the other every message on any topic both subscribe to.

Both kinds of entry coexist in the same local table on each SPO node, but the link semantics they produce differ — owner-attested entries result in incoming unidirectional links from listed relays, while mutual local-attested entries result in symmetric bidirectional links between operator-trusted peer pairs. Together with the ring and random links of the standard dissemination layer, UC-1's topology carries four classes of links in parallel.

### 5.2 Why the Threat Model Is Different

The SPO recipient set has properties the general gossip overlay does not:

- **Heterogeneous participants; stake-binding only via optional plugin.** Most participants are SPO operators, but the network may also include non-SPO infrastructure providers and full-node wallet operators who do not carry stake-bound identity. The initial 3-layer protocol contains no identity-binding heuristics; identity remains plain keys (§2.2). A future trust-layer sidecar plugin (per §2.2's future direction) could verify SPO participants against on-chain stake and credentials, raising effective Sybil cost for that subset; non-SPO participants would remain outside that verification.
- **Stable peer set.** Low churn for SPOs (epoch-bounded join/leave); the on-chain SPO list is enumerable if a plugin or operator chooses to use it.
- **Intrinsic incentive.** SPOs need the alerts to operate correctly; no protocol-level incentive design is required for participation. Non-SPO participants typically have similar operational incentives.
- **Operator-decided trust.** SPO operators already have informal trust networks (monitoring partnerships, redundancy groups). The local-relays extension surfaces these directly into the topology.

Consequently, several AUEB gap-analysis findings reduce in this setting. Some reductions are always available from the extensions and deployment context; others are conditional on a stake-binding plugin and apply only to SPO participants.

| Gap | Reduction in UC-1 |
|---|---|
| S-04 (Sybil resistance) | Mutual-link consent prevents adversaries from inserting links into honest-honest pairs (always on). With the trust-layer plugin deployed, stake-binding additionally raises Sybil cost for verified SPO participants. |
| S-10 (identity grinding for ring placement) | Plugin-conditional: with stake-binding, grinding requires compute *and* stake. Without the plugin, no reduction over the AUEB baseline. |
| S-13 (registry trust) | Plugin-conditional: SPO identities can be verified against on-chain credentials by the plugin. Without it, the registry remains coordination-only and trust mapping falls to the off-chain trust layer (§2.2) as in any other use case. |
| S-17 (no incentive to gossip) | Always on: SPOs and other participants intrinsically value alerts; mutual-link reciprocity adds soft incentive. |
| S-02 (selective forwarding by a correctly-embedded node) | Always on: diverse delivery paths (owner-attested relay + mutual-link partners + random links) mean a single non-forwarding peer is bypassed by the others. Independent of identity binding. |

Non-SPO participants (infrastructure providers, full-node wallets) remain outside any stake-bound subset and rely only on the always-on reductions.

A specialized Vicinity tuned for the low-churn SPO set (bootstrapping peer samples from the on-chain SPO list, stake-weighted ring positioning) remains a possible refinement but is not required for the design to function.

### 5.3 What UC-1 Does Not Need

- No wallet-backend intermediary (recipients are operators directly).
- No replication for catch-up beyond what SPOs can pull from peers directly — operators are infrastructure-grade with their own retention practices.
- No new incentive design (intrinsic SPO motivation plus mutual-link reciprocity).

### 5.4 Shared Primitives

UC-1 uses the same cross-cutting primitives as the wallet-backend track ([§2](#2-cross-cutting-primitives)): on-chain topic registry, identity material, hash-chained signed message envelope. The divergence from the wallet-backend track is a matter of which extensions and which attestation patterns are deployed — not a different protocol. 

Specific calibration points for UC-1 (mutual-link table size, freshness mechanism for the reciprocal handshake, public vs. private table policy) are treated as open questions ([§9.8](#98-uc-1-deployment-calibration)).

---

## 6. Deferred Stages

These are not abandoned; they are out of scope for the Stage 1 / Stage 2 design and the UC-1 track described above.

### 6.1 Replication (Catch-Up)

When a relay or edge node is offline at message N, no protocol-level mechanism currently fills the gap. Replication adds a catch-up role: nodes (potentially the same wallet backends, or specialized archival nodes) maintain a queryable history per `(topicId, publisherId)` and respond to "give me everything since chain hash X" pull requests.

This activates the AUEB persistence-layer design and inherits its gap-analysis findings ([S-07](aueb-gap-analysis-final.md#s-07--byzantine-failure-notifications-enable-ejection-by-false-accusation), [S-08](aueb-gap-analysis-final.md#s-08--proof-of-storage-is-acknowledged-as-prohibitively-expensive--no-alternative-designed), [S-14](aueb-gap-analysis-final.md#s-14--catch-up-requires-information-the-subscriber-may-not-have), I-15, I-19). The protocol-level catch-up query mechanism still needs design (chain-hash anchored, not timestamp anchored, to close S-14).

The incentives for replication servers needs to be refined.

### 6.2 Incentives

For the use cases addressed above, the design relies on heuristics — operator self-interest, mutual-link consent, and existing trust networks among operators — to ensure delivery to key actors and reduce the risk of eclipse and partition attacks. Use cases where self-interest is weaker or pre-existing trust connections do not arise naturally will require explicit incentive mechanisms before the protocol can be deployed securely. See [Incentive Model](incentive-model.md) for the persistence-layer incentive analysis already developed in this workstream.

---

## 7. Gap Inheritance Across Stages

This map shows which findings from the [AUEB Gap Analysis](aueb-gap-analysis-final.md) activate, are bypassed, or have reduced severity at each stage and track.

### 7.1 Stage 1 (Wallet-Backend Track) — Direct Delivery to Relays

**Bypassed (the layer or feature does not exist):**

| Finding | Reason |
|---|---|
| S-01, S-02, S-03, S-05, S-06, S-09, S-10 | Attack the navigation/dissemination overlay or SecureCyclon — Stage 1 has none |
| S-04 | No open gossip-layer participants; relays are an owner-listed set |
| S-11, S-12 | No multi-layer gossip; foundational-protocol guarantees not relied on |
| S-17 | No gossip layer to participate in |
| S-19 | Architecturally sidestepped — primary delivery is direct push, not gossip; the rational-subscriber-prefers-polling tension does not exist |
| S-07, S-08 | Replication role not deployed |
| S-14, S-16, I-19 | No catch-up workflow |
| I-02, I-04, I-06, I-10, I-12 | Properties of overlay structure; no overlay |

**Inherited (apply unchanged):**

| Finding | Stage 1 implication |
|---|---|
| S-13 (registry coordination ≠ identity trust) | Topic-name impersonation is a real risk; mitigation via curated metadata layer above the registry, out of protocol scope |
| S-15 (timestamp clock spec) | Timestamps in the message envelope are advisory only; chain ordering uses sequence + parent hash, not time. Substantially defanged but worth specifying explicitly |
| S-18 (private-value messages break gossip) | Stage 1 doesn't gossip, but the protocol scope still excludes exclusive-value use cases (UC-7, UC-8 explicitly out) |
| I-01 (key rotation for owners) | Critical for any topic with high-value publication authority; mitigation via multisig owner sets |
| I-03 (dedup mechanism) | Relays need a dedup cache; specifying it is implementation-level work |
| I-07, I-08 (registry confirmation latency, rollback) | Standard chain-integration handling; n-block confirmation delay |
| I-09 (encryption) | Out of scope; Stage 1 topics are public by design |
| I-13 (NAT) | Relays are infrastructure-grade with public addresses; not a Stage 1 issue |
| I-16 (open-topic spam) | Stage 1 uses publisher-listed topics; only relevant if open topics are introduced |

**Closed by construction (the design eliminates the finding):**

| Finding | Mechanism |
|---|---|
| I-05 (sequence-number gaps indistinguishable from lost events) | Chain-hash linkage detects gaps deterministically |
| I-14 (gossip path arrives unordered, signature doesn't bind sequence) | Envelope binds `sequence` + `parentHash` into the signature |
| I-17 (cross-topic replay) | Envelope binds `topicId` into the signature |

### 7.2 Stage 2 (Wallet-Backend Track) — Adding the Gossip Overlay

Stage 2 reactivates the AUEB protocol's gap surface for the **overlay path only**:

**Activated, full severity:**

| Finding | Notes |
|---|---|
| S-01, S-02, S-03, S-05, S-06 | Standard SecureCyclon/Vicinity attack surface against the gossip overlay |
| S-04 (Sybil resistance) | Critical at the relay→edge-node registration boundary AND on the overlay itself. Mitigation requires identity-binding work, deferred |
| S-10 (identity grinding for ring placement) | Relevant once dissemination uses the Harary ring or any deterministic-position structure |
| S-11 (heuristic foundations) | Stage 2 inherits the empirical-vs-formal-verification gap of SecureCyclon and Vicinity |
| S-12 (inter-layer timing) | Two layers (sampling + dissemination); timing analysis required before deployment |
| S-17 (no incentive for disinterested-topic gossip) | Real for any edge node that participates in multiple topics it does not consume |

**Activated but with reduced impact (the additive-path property):**

| Finding | Reduction |
|---|---|
| S-01, S-02, S-03, S-06 (above) | These attacks degrade overlay delivery; they do **not** affect wallet-backend delivery. The original AUEB protocol's Critical/High severity is retained for users whose only path is the overlay, but is reduced to Medium for wallet-backed users (the dominant population) |
| S-19 (persistence vs gossip tension) | Sidestepped: in this design, gossip is additive rather than the primary delivery path, so the rational-subscriber-prefers-polling failure mode does not threaten the dominant flow |

**Implementation-level findings activated:** I-02, I-03, I-04, I-06, I-10, I-12 — all overlay-structure properties.

### 7.3 UC-1 (SPO Alerts) Track

UC-1 deploys gossip dissemination from day one ([§5](#5-uc-1-spo-alerts--distinct-topology-track)), so the wallet-backend bypass-set in §7.1 does not apply. The relay-tier and local-relays extensions plus the SPO operator-trust context reduce several findings — some always, some only when the optional stake-binding trust-layer plugin (§2.2) is deployed for SPO participants:

| Finding | UC-1 reduction |
|---|---|
| S-04 (Sybil) | Always on: mutual-link consent prevents adversaries from inserting links into honest-honest pairs. Plugin-conditional: stake-binding additionally raises Sybil cost for verified SPO participants. |
| S-10 (identity grinding for ring placement) | Plugin-conditional: combined compute + stake cost when stake-binding is deployed. Without the plugin, no reduction over the AUEB baseline. |
| S-13 (registry trust) | Plugin-conditional: SPO identities can be verified against on-chain credentials by the plugin. Without it, the trust layer is off-chain as for any other use case. |
| S-17 (no incentive to gossip) | Always on: SPOs and other participants intrinsically value alerts; mutual-link reciprocity adds soft incentive. |
| S-02 (selective forwarding) | Always on: diverse delivery paths (owner-attested relay + mutual-link partners + random links) bypass any single non-forwarding peer. |

Non-SPO participants (infrastructure providers, full-node wallets) rely only on the always-on reductions. The remaining gap surface (S-01, S-03, S-05, S-06, S-11, S-12) still applies to UC-1's gossip layer; the effective adversary fraction in stake-bound deployments is smaller than the general AUEB analyses assume. UC-1 deployment calibration questions are tracked in [§8.8](#88-uc-1-deployment-calibration).

### 7.4 Deferred Stages

Adding replication activates S-07, S-08, S-14, I-15, I-19. Adding non-wallet relays or independent edge nodes activates the broader incentive design problem (no intrinsic reason for them to relay) and re-opens the Sybil resistance question for whichever new actor class joins. These are explicitly out of scope for the staged Stage 1 / Stage 2 design and the UC-1 track.

---

## 8. Open Questions

Items that need resolution before, during, or after the design becomes a specification.

### 8.1 Relay Push Policy at Scale

Stage 2 specifies eager and random-subset push as the two valid relay-local policies. The crossover point at which random-subset becomes preferable, and the fanout factor `k` for random-subset, are deployment parameters. Both depend on real network and load characteristics; neither can be pinned down before Stage 1 deployment data exists.

### 8.2 Edge-Node Registration with Relays (Stage 2)

How an edge node registers interest with a relay is unspecified. Open sub-questions:

- Is registration self-attested, relay-attested, algorithmic, other?
- What identity binding does the relay require (e.g. Cardano stake credential, SPO key, generic Ed25519)?
- What anti-spem policy applies?

These interact with the deferred Sybil-resistance work and need not be settled in Stage 1.

### 8.3 User-Level Subscription / Consent Model

In Stage 1 the protocol assumes the wallet handles which messages a user wants. The boundary between protocol and wallet UX is intentional, but the implications for wallet-app design (per-topic permissions, opt-in defaults for SPO/dRep/dApp messages) deserve explicit documentation, possibly as a companion document to this one.

### 8.4 Late-Joiner Support Strategy Before Replication Ships

A relay rebooting after a longer outage has no protocol-level catch-up in Stage 1 or Stage 2. Two interim strategies, before replication ships:

- Publisher-side outbound buffer + retry on connection re-establishment.
- Relay-to-relay catch-up (peer wallet backends serve each other history).

Either is workable for Stage 1 frequencies. The replication design supersedes both.

### 8.5 Per-Entity vs Per-Category Topics for SPO/dRep

The design currently favors per-entity topics (each SPO/dRep registers their own topic with their on-chain identity), based on the self-sovereignty argument: per-entity registration uses existing on-chain credentials and avoids needing a curation list of "allowed publishers" for a shared category topic.

The cost is registry size and the number of streams a wallet backend subscribes to (potentially thousands). Mitigations may exist at the API layer (topic-family abstractions), but are not yet specified.

### 8.6 Relay Self-Attested vs Owner-Attested Registry Entries

Whether wallet backends should self-register topic support in the registry (and what such an entry would mean contractually) is open. Owner-attested relay lists are sufficient for Stage 1 and Stage 2; self-attestation is a feature only if it enables a discoverability flow (users querying which wallet backends carry topic Y) and that flow is not yet in the use case set.

Note that this point is about an application level support and not a relay role registration.

### 8.7 Hash Chain Recovery on Publisher Key Loss

The plain hash chain uses a single long-term publisher key. Key rotation is owner-driven (registry update), but a key-loss event leaves the chain bricked unless an explicit recovery anchor is added. Recovery options range from "new genesis after key rotation" (clean break, breaks chain continuity) to "owner-signed bridge" (preserves continuity, requires owner involvement). Acceptable answers depend on the topic's recovery requirements.

### 8.8 UC-1 Deployment Calibration

UC-1 ([§5](#5-uc-1-spo-alerts--distinct-topology-track)) uses the standard AUEB three-layer protocol plus the relay-tier and local-relays extensions. The protocol shape is fixed by those choices; deployment calibration remains open:

- **Mutual-link table size `K`.** What table size best fits SPO operational structure (typical operator group size, redundancy partner counts)? The Local Relays Extension Proposal suggests K = 16 to 32 as plausible; SPO operator surveys would tighten this.
- **Freshness mechanism for the reciprocal handshake.** Challenge-response, recent-timestamp, or chain-reference. Each has different implementation cost and replay-window properties.
- **Public vs. private local relays table.** Public tables ease topology mapping and discovery; private tables protect operator-trust patterns. SPOs choose deployment policy.
- **Identity binding for SPO node keys.** Mechanics to reuse the SPO on-chain identity in order to improve security.
- **Specialized Vicinity tuning.** Whether the standard Vicinity variant suffices given the stake-bound threat-model reductions, or whether a low-churn-aware variant (bootstrap from the on-chain SPO list, stake-weighted ring positioning) is worth the effort.
- **Operator opt-in deployment model.** UC-1 ideally runs as a sidecar to cardano-node, not inside it. Sidecar reading topology files, plugin, or fully independent install.

---

## References

- *AUEB Gap Analysis* (April 2026). [aueb-gap-analysis-final.md](aueb-gap-analysis-final.md).
- *Actor & Use Case Analysis* (April 2026). [actor-use-case-analysis.md](actor-use-case-analysis.md).
- *Incentive Model* (April 2026). [incentive-model.md](incentive-model.md).
- *Cardano Pub/Sub Framework: Design and Architecture* (D2). AUEB / IOG Research, 2024. [D2-Cardano-PubSub-Framework-Design-and-Architecture.pdf](D2-Cardano-PubSub-Framework-Design-and-Architecture.pdf).
- *PubSub Technical Review* (March 2026). [pubsub-technical-review-march-with-links.md](pubsub-technical-review-march-with-links.md).

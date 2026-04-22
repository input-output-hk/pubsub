# AUEB Report — Gap Analysis and potential problems

**Date:** April 2026
**Source document:** D2 — Cardano Pub/Sub Framework Design and Architecture (September 2024)
**Supporting papers:** SecureCyclon: Dependable Peer Sampling (ICDCS 2023); VICINITY: A Pinch of Randomness Brings out the Structure (Middleware 2013)

---

## Overview

This document consolidates observations from two independently produced reviews of the AUEB report and the underlying protocols it relies on. One reviewer was the author of this commit and the other one is Claude. Both reviewers read the source documents independently and recorded observations without seeing each other's work. The observations were then discussed and merged into this document.

Observations are organized into two tiers:

1. **Structural** — issues that affect the correctness, security, or viability of the design. These begin with active attack vectors and then move to design gaps that do not constitute immediate attacks but are structurally significant.
2. **Implementation-level** — lower-level gaps and missing details to be addressed in later design phases.

---

## Part I — Structural Observations

### Attack Vectors

#### S-01 · Navigation Eclipse via False Topic Membership Advertising

The navigation layer uses Vicinity to build per-node views of which nodes are close to which topics. There is no mechanism to verify that a node actually subscribes to the topics it advertises. An adversary can claim membership in strategically chosen topics to maximize its appearance in other nodes' finger tables. By advertising membership in the most subscribed or most central topics, the adversary positions itself as a routing hub at the navigation layer — legitimate nodes' finger links will naturally point to it, giving it control over topic discovery routing for those topics. This attack operates entirely at the Vicinity layer and bypasses SecureCyclon, which provides no protection against false topic membership claims.

**Effect.** A successful attack has two compounding consequences: (1) denial of discovery — nodes subscribing to or recovering connectivity for the target topic fail to find legitimate peers, or do so only after long delay; (2) redirection — the adversary returns descriptors of other adversary-controlled nodes, placing the victim in an attacker-chosen neighborhood. Once the adversary selects a victim's topic neighbors, downstream attacks at the dissemination layer (S-02, S-03, S-06) become viable against that victim. Both new subscribers and existing nodes recovering from churn are affected.

---

#### S-02 · SecureCyclon Security Scope Is Misrepresented: Topology ≠ Delivery

SecureCyclon defends against attacks on *who connects to whom* — hub attacks and eclipse attacks on the overlay topology. It does not defend against a correctly-embedded node that drops, delays, or selectively forwards messages. A node that passes all SecureCyclon certificate checks can suppress every message it receives for a given topic with no detection mechanism. The report implicitly treats overlay security as delivery security; these are distinct properties, and only the first is addressed.

---

#### S-03 · SecureCyclon Certificate Chains Do Not Extend to the Vicinity Layers

SecureCyclon prevents descriptor fabrication through a certificate chain verified during peer sampling gossip exchanges. The navigation and dissemination layers use Vicinity, which applies no certificate-chain verification. A malicious node correctly embedded in SecureCyclon can, during Vicinity gossip, inject fabricated descriptors pointing to other malicious nodes — without a valid SecureCyclon certificate, because Vicinity does not check for one. This re-introduces the hub attack at the Vicinity layer, bypassing the protection SecureCyclon provides at the peer sampling layer.

A subtler attack is also unaddressed: a correctly embedded malicious node can selectively forward only descriptors of other malicious nodes that carry valid certificates, steering legitimate nodes toward a malicious neighborhood without fabricating anything. SecureCyclon prevents descriptor forgery; it does not prevent selective forwarding of legitimate descriptors.

---

#### S-04 · No Sybil Resistance for Gossip Layer Participants

SecureCyclon's security analysis assumes a bounded fraction of malicious nodes. In a permissionless network, this holds only if creating a new node identity is costly. Standard asymmetric key pairs (e.g., Ed25519) can be generated in microseconds, so an adversary can trivially create thousands of distinct identities and violate the assumed adversary fraction with no computational barrier.

The report requires on-chain registration and a security deposit for replication servers, providing Sybil resistance for that specific role. No identity cost or Sybil resistance mechanism is specified for general gossip layer participants.

---

#### S-05 · Timestamp Manipulation Attack on SecureCyclon's Selection Function

SecureCyclon descriptors carry a creator-assigned timestamp used to establish freshness; newer descriptors are preferred in the selection process. A malicious node sets an artificially high timestamp on its own descriptor, making it appear fresher than all legitimate descriptors in the network. Legitimate nodes preferring fresher descriptors will favor the malicious one in their views — without any certificate violation, since the malicious node signed its own descriptor with its own key. The inflated timestamp persists through all subsequent transfers, as transfers add signatures but do not re-timestamp the original descriptor.

This attack is compounded by the synchrony assumption underlying SecureCyclon's security analysis: detection rates are derived from synchronized gossip cycles, but real deployments have heterogeneous hardware and variable network latency, reducing detection effectiveness under asynchronous operation.

**Note on the selection rule.** The timestamp-based freshness preference is not incidental — it serves two roles in SecureCyclon: (1) a *liveness signal*, since the certificate chain proves legitimate propagation but says nothing about whether the referenced node is still online, so without a freshness criterion views accumulate dead nodes; and (2) *replay resistance*, since a selection rule indifferent to age would let an adversary re-inject very old but validly-signed descriptors to poison views with stale nodes or nodes with rotated keys. Switching away from time-based replacement (e.g., to random selection or active liveness probing) is possible but requires deeper analysis of the impact on both properties before it can be recommended.

---

#### S-06 · Vicinity Has No Byzantine Resistance — Navigation and Dissemination Layers Are Undefended

The Vicinity paper states explicitly that Byzantine behavior is beyond its scope. Both the navigation and dissemination layers in the AUEB report are built on Vicinity without addressing this gap.

These are attacks on overlay structure formation, not on message forwarding. A Byzantine node in the navigation layer can provide false routing pointers during the overlay construction phase, directing joining nodes to the wrong topic cluster and slowing or preventing convergence to the correct topic overlay. A Byzantine node in the dissemination layer can disrupt ring formation by presenting misleading neighbor selections during Vicinity gossip. Neither failure mode has a detection or recovery mechanism.

---

#### S-07 · Byzantine Failure Notifications Enable Ejection-by-False-Accusation

The maintenance protocol assigns each replication server to monitor the health of the servers storing the same events. When server p detects server q as unavailable, p broadcasts the failure to trigger data recovery across the DHT. A Byzantine p can send false failure notifications about a healthy q, triggering unnecessary data migration and causing other servers to treat q as unavailable.

No quorum, confirmation, or cross-verification mechanism for failure reports is described. The ejection criteria is also underspecified: who applies penalties, what evidence is required, and what prevents a coalition of Byzantine nodes from framing a healthy node are all open questions. While replication server security deposits could in principle be used to penalize false accusations, distinguishing intentional false accusations from accidental ones caused by transient network issues is non-trivial. A well-defined accusation and verification process must be designed before this mechanism can be safely implemented.

---

#### S-08 · Proof of Storage Is Acknowledged as Prohibitively Expensive — No Alternative Designed

Section 4.1.2 states that Proof of Replication / Proof of Retrieval protocols "may be prohibitively expensive to execute directly on-chain, in a smart contract context." This is a critical admission: the entire incentivization and penalization mechanism depends on being able to verify storage compliance. Without a feasible on-chain verification mechanism, penalties cannot be enforced, and the security deposit model provides no real guarantees. The report defers this as "a separate line of research" without providing any interim design.

---

#### S-09 · Navigation Churn Attack via Topic Creation and Deletion

Every gossip node must maintain knowledge of all topics to compute finger distances in the navigation layer. Inserting or deleting a topic shifts the circular topic ring, requiring all nodes to recompute affected finger links and establish new connections. An adversary can register and delete topics repeatedly, forcing continuous navigation layer churn across all participating nodes. The cost to the attacker is linear in the number of topics created; the cost to the network is proportional to the number of participating nodes × topics affected. On-chain registration fees provide partial rate-limiting but not a defense.

---

#### S-10 · Identity Grinding for Targeted Placement in the Harary Ring

Node positions in the Harary dissemination ring are determined by sorted 256-bit node IDs: a node's ring neighbors are the nodes whose IDs are numerically adjacent to its own. This placement is a deterministic, public function of the node's identity key. S-04 describes the global Sybil threat of crossing an adversary-fraction threshold across the whole network. Targeted placement is a distinct threat with a much smaller budget: an adversary that wants to attack a specific topic mines Ed25519 keys until one falls adjacent to a target subscriber's ID on that topic's ring. Identity generation takes microseconds, so this requires seconds to minutes per target. The result is a small number of identities positioned as ring neighbors of chosen subscribers on a chosen topic, enabling targeted eclipse, message suppression, or selective forwarding without violating any network-wide Sybil bound.

This class of attack has been explored in the PRISM model developed in this workstream. Mitigations — identity binding to on-chain material (e.g., stake pool keys) or topic-specific unpredictable salts in the ring position function — are left for later design work.

---

### Design Gaps

#### S-11 · SecureCyclon and Vicinity Security Guarantees Are Heuristic and Simulation-Based

SecureCyclon is presented in its source paper as a heuristic approach evaluated through PeerSim simulations. The paper's own results show that under high adversarial conditions (40% malicious nodes), a significant fraction of legitimate nodes still end up with a disproportionate number of links to malicious nodes — eclipse resistance degrades with adversary fraction rather than being guaranteed. The Vicinity paper has no formal proof of convergence, no bound on convergence time as a function of network parameters, and no security analysis under adversarial conditions. All properties are empirically demonstrated through simulation on networks of up to 65K nodes.

The AUEB report builds both its navigation and dissemination layers on Vicinity, and its peer sampling layer on SecureCyclon, without surfacing these limitations. Any formal specification — such as the Quint and PRISM models being developed — rests on foundational protocols whose properties are justified by heuristic argument and simulation alone. This is a material gap for advancing through SRL levels, where analytical verification of key parameters is required at SRL 3 and formal specifications at SRL 4.

---

#### S-12 · The Three Gossip Layers Have No Specified Relative Timing

SecureCyclon, Vicinity (navigation), and Vicinity (dissemination) each gossip independently on their own periods. The relative timing of these three layers is not specified. If the peer sampling layer gossips much faster than the dissemination layer, it may continuously replace ring neighbors before they stabilize. If the dissemination layer gossips faster than the navigation layer, nodes may receive topic traffic before they have found the correct topic cluster. No analysis of inter-layer timing dependencies is provided, leaving the three-layer protocol without a coherent system-level convergence guarantee.

---

#### S-13 · The Topic Registry Provides Coordination, Not Identity Trust

The registry gives every participant a consistent, shared view of which topics exist and which public keys are authorized to publish. However, it cannot verify that the entity registering a topic is who it claims to be. Any actor can register a topic with any name, including names designed to impersonate a legitimate authority — for example, registering a topic named `governance/emergency` with a fraudulent publisher key.

This is structurally identical to the fake token problem on Cardano: anyone can mint tokens with any display name; the policy ID is the ground truth, but human-readable names collide freely. The ecosystem's response was a curated external registry mapping policy IDs to verified metadata. PubSub has the same structure and requires the same remedy: a trust layer above the registry binding topic ownership to verifiable identities. None of this is designed or acknowledged in the report.

---

#### S-14 · Catch-Up Requires Information the Subscriber May Not Have

The catch-up workflow assumes the subscriber can provide the timestamp of when it went offline. This fails in common distributed systems scenarios:

- A subscriber that experienced a network partition has no reliable record of when it stopped receiving events.
- A subscriber that crashed and restarted has lost local state entirely, including the offline timestamp.
- A subscriber that was partially online — receiving some but not all gossip traffic — has no clean "went offline" event at all.

The timestamp-based query is only correct for intentional, clean shutdowns. For crash, partition, or partial connectivity, the recovery path is undefined.

---

#### S-15 · Timestamps Are Used in Multiple Contexts Without a Clock Specification

The report uses timestamps in at least three distinct contexts: the offline timestamp provided by the subscriber for catch-up queries; the timestamp stored in the topic log alongside the last sequence number per publisher; and the 5-second timeouts for failure detection pings. No clock synchronization protocol is specified, and no designation of whose clock is authoritative is provided. A publisher with a skewed clock could assign timestamps that fall outside a subscriber's expected recovery range, causing valid events to be silently excluded from catch-up results. This compounds the timestamp-related issues already identified at the protocol layer: creator-assigned timestamps in SecureCyclon descriptors enable freshness manipulation (→ S-05), and Vicinity's gossip exchanges similarly carry no timestamp verification. Across all three layers, timestamps are used as an assumed shared ground truth without any mechanism to establish or enforce it.

---

#### S-16 · Cross-Publisher Ordering Is Absent From the Retrieval Model

The catch-up mechanism retrieves events per publisher: for each publisher active on a topic, a subscriber requests all missed sequence numbers in order. This provides internally consistent ordering within a single publisher's stream but gives no mechanism to interleave events from multiple publishers chronologically.

*Example:* If publisher A sends A1, publisher B sends B1, then A sends A2, a recovering subscriber retrieves {A1, A2} from A's stream and {B1} from B's stream with no protocol-level information about how these interleave.

This should be stated explicitly as a known limitation: there is no well-defined notion of "the Nth event on topic T" — only "the Nth event from publisher P on topic T." The retrieval model is per-publisher by design, with no cross-publisher ordering semantics.

---

#### S-17 · Nodes Have No Incentive to Participate in Gossip for Topics They Do Not Subscribe To

The navigation layer requires every node to maintain finger links to topics it does not subscribe to and respond to routing queries from other nodes. These are real operational costs — bandwidth, connection management, computational overhead — with no direct return for a node whose only interest is its own subscribed topic's messages. The report implicitly assumes SPOs will operate nodes for their own use and that the overhead is negligible. As message volumes or topic counts grow, the gap between the cost of participation and the benefit to the forwarder widens.

---

#### S-18 · The Protocol Is Not Equipped for Messages With Intrinsic Private Value

Gossip protocols assume cooperative forwarding. When a message's value is non-exclusive — equally useful to all recipients — there is no incentive to suppress it. The notification use cases (governance alerts, SPO announcements) largely satisfy this. However, when a message has exclusive or competitive value — only the first actor to receive and act on it benefits, or the value is destroyed by sharing — the rational strategy upon receipt is suppression rather than forwarding.

The protocol makes no structural distinction between notification messages with non-exclusive value and messages with exclusive value. This should be documented as a designed-in scope constraint: the gossip protocol is appropriate for notifications whose value is non-exclusive and unsuitable for messages where recipient value depends on exclusivity or competitive advantage.

---

#### S-19 · The Persistence Layer Structurally Undermines the Gossip Layer for Notification Use Cases

The gossip layer provides real-time delivery. The replication layer provides reliable catch-up retrieval. For notification use cases — where messages are infrequent and latency is measured in minutes rather than milliseconds — a subscriber polling replication servers periodically achieves functionally equivalent results without running a gossip node. If enough subscribers make this choice, the gossip overlay thins, dissemination becomes slower and less reliable, further reducing the incentive to participate.

This also reframes the minimum viable design question: for the notification use case specifically, a design where publishers write directly to replication servers and subscribers poll may be simpler, more reliable, and sufficient — making the three-layer gossip protocol unnecessary complexity for the actual target use cases.

---

## Part II — Implementation-Level Observations

The following observations are lower-level gaps and unspecified details to be addressed in later design phases. They do not represent immediate structural risks but must be resolved before the system can be implemented.

---

**I-01 · Key rotation for topic owners.** The owners list contains public keys with no key rotation or recovery mechanism. An attacker who steals an owner key can add themselves as a permanent owner, remove legitimate owners, modify the publisher list, or delete the topic.

**I-02 · Small topic subscriber sets.** The Harary ring with connectivity t requires at least t+1 nodes. For topics with very few subscribers (1–5 nodes), the ring degrades to a trivial path or a single node with no meaningful fault-tolerance properties. Minimum subscriber counts and fallback behavior are unspecified.

**I-03 · Message deduplication mechanism unspecified.** When a node receives a message it has already seen, it discards it. The mechanism for tracking seen messages is not described: data structure, retention period, eviction policy. An unbounded seen-list is a memory exhaustion vector for long-running nodes on high-traffic topics.

**I-04 · Ring link latency.** The Harary ring orders nodes by arbitrary 256-bit IDs. Adjacent IDs are not geographically or topologically close, so ring links — the reliability backbone of the dissemination layer — may be systematically higher-latency than the random links, inverting the intended performance profile.

**I-05 · Sequence number gaps.** The catch-up scheme increments per-publisher sequence numbers until a lookup returns nothing. If a publisher crashes after partially submitting an event (signed but not yet stored by replication servers), the subscriber increments past the gap and concludes it is up-to-date. There is no mechanism to distinguish "no event was published with this sequence number" from "an event was published but was lost before reaching the DHT."

**I-06 · Online but missed — DHT query on convergence.** A newly joined node whose ring has not yet converged may miss events with no recovery path. The mitigation is straightforward but unspecified: once a node infers it has converged to its position in the topic overlay, it should proactively query the DHT to fill any gap from the convergence window.

**I-07 · On-chain confirmation latency.** Topic registry updates are confirmed on-chain only after block finality (~20 seconds with Ouroboros Praos). During this window, removed publishers can still inject messages that will be forwarded and stored. Adding a confirmation delay (waiting N blocks before acting on registry changes) mitigates most lag window issues. However, for the compromised publisher case, an actively exploited key continues injecting messages during both the confirmation window and any additional delay.

**I-08 · Registry behavior under chain rollback.** I-07 addresses the confirmation-latency window: the interval between a registry transaction appearing on-chain and being safe to act on. Distinct concern: Cardano's settlement model permits rollbacks of up to K blocks. A topic registration, publisher-list change, or `setReplicationFactor` call that is confirmed but not yet deeply finalized can be reverted. The report specifies no rollback-handling protocol for off-chain state derived from on-chain events: how nodes detect that a registry action has been undone, what happens to messages signed under a now-reverted publisher list, and whether replication servers excise events that were accepted under a reverted owner configuration. Treating settled blocks as final compresses the confirmation window at the cost of rollback hazards; the tradeoff is not analyzed.

**I-09 · Privacy — encryption not designed.** The report acknowledges all topics are public and states that "if privacy is required, encryption should be employed." No encryption mechanism, key distribution scheme, group key management, or rekeying protocol is designed.

**I-10 · Bootstrapping mechanism unspecified.** New nodes need an initial set of contacts to begin participating in SecureCyclon. The bootstrap mechanism — and how the certificate chain requirements interact with the initial join process — is not described.

**I-11 · Historical publisher validation gap.** The topic registry reflects the current publisher list, not its history. A node catching up on historical events cannot validate signatures from publishers that have since been removed. A replication server presenting events from removed publishers cannot be challenged, nor can it be verified that events from removed publishers are being correctly excluded. An on-chain history of publisher list changes, or timestamp-scoped registry queries, would be needed to validate historical events correctly.

**I-12 · SecureCyclon certificate mechanism requires an unspecified PKI.** SecureCyclon requires each node to sign its descriptor and each subsequent transfer, forming a certificate chain that proves legitimate ownership through each intermediate holder. This requires per-node asymmetric key pairs and a PKI to bootstrap and verify them. The report does not specify how nodes generate these keys, whether they are the same as on-chain identity keys, how revocation works, or how initial key material is bootstrapped.

**I-13 · NAT traversal unaddressed.** The report assumes nodes connect directly to each other via IP address and port. If light clients are expected to participate in the gossip overlay — which the report does not rule out — NAT traversal becomes a blocker: the majority of light clients operate behind NAT and cannot receive inbound connections. The transport layer requirements are never specified.

**I-14 · Two delivery paths designed independently — no integrated subscriber behavior.** The live gossip path (Chapter 3) and the catch-up DHT path (Chapter 4) are designed independently. A real subscriber must use both simultaneously: receiving live events via gossip, detecting gaps using sequence numbers, filling gaps via DHT, and reconciling two asynchronous streams. This integrated subscriber behavior — buffering, gap detection, DHT queries, stream reconciliation — is entirely absent from the report. A related issue is that on the gossip path, messages arrive in an unpredictable order and carry no sequence number in their payload. A subscriber has no intrinsic way to establish the publisher-intended order of messages without relying on replication servers as an external authority — which defeats the purpose of the live gossip path as an independent delivery mechanism. Additionally, the publisher's signature covers message content but does not bind it to the sequence number, so a replication server can reorder messages within a publisher's stream while keeping all signatures valid, with no way for the subscriber to detect this at the routing level.

**I-15 · Topic log and event store writes are not atomic.** When a publisher submits an event, two distinct writes must occur: the event stored at `hash(TOPIC · PUBLISHER · SEQUENCE_NR)` on the DHT, and an update to the topic log at `hash(TOPIC)` recording the latest sequence number. No distributed transaction protocol is described. If the topic log is updated before all replicas are stored, a recovering subscriber reads "last sequence = N" but the request for event N returns nothing — with no way to distinguish "not yet replicated" from "never published."

**I-16 · Open topic attack surface: flooding and storage exhaustion.** If the publishers list is empty, any node can publish to a topic. The report provides no rate-limiting or anti-spam mechanism. An adversary can flood an open topic with high-frequency messages, consuming forwarding bandwidth across all subscribed nodes; replication servers are obligated to store every message that passes the publisher signature check, so a sustained attack also exhausts replication server disk storage. The security deposit mechanism creates no remedy — complying with the protocol is the attack. This gap is explicitly called out as functional requirement FR5.1 but is never addressed in the design.

**I-17 · Publisher signatures not bound to topic — cross-topic replay.** A message signed by a publisher is not a priori bound to a specific topic, so a malicious node could replay a signed message into another topic where the same publisher is also listed — every signature check passes. A simple mitigation is for publishers to use distinct key material per topic. Noted here for documentation purposes.

**I-18 · Subscription patterns observable through gossip.** I-09 notes that payload encryption is not designed. Separately, subscription privacy — knowledge of which topics a given IP or node relays, catches up on, or serves ring-neighbor traffic for — is observable through traffic analysis by an adversary running gossip nodes across the overlay. This is independent of payload encryption: encrypted content does not hide the fact that a subscriber relays encrypted traffic for a specific topic. For use cases where subscription itself is sensitive — voting behavior, SPO affiliation, geographically scoped subscriptions — metadata privacy is a distinct requirement from payload privacy and is structurally hard to retrofit into gossip overlays.

**I-19 · Replication factor migration semantics unspecified.** Owners can change a topic's replication factor via `setReplicationFactor`, but the migration protocol is not defined: whether excess replicas are purged or retained, whether new replicas are created eagerly or lazily, and how the system behaves during the transition are all unspecified. If a topic owner is compromised, changes to this parameter may cause harm whose severity depends both on the semantics the protocol ultimately adopts and on the use case. The observation here is the undefined behavior itself; impact cannot be assessed further until migration semantics are specified.

---

## Summary

### Structural — Attack Vectors

| ID | Area |
|---|---|
| S-01 | Navigation eclipse via false topic membership advertising |
| S-02 | SecureCyclon scope: topology ≠ delivery security |
| S-03 | Certificate scope regression at Vicinity layers |
| S-04 | No Sybil resistance for gossip participants |
| S-05 | Timestamp manipulation on SecureCyclon selection function |
| S-06 | Vicinity Byzantine resistance absent — overlay structure formation undefended |
| S-07 | Byzantine failure notifications / ejection-by-false-accusation |
| S-08 | Proof of storage undesigned — penalties unenforceable |
| S-09 | Navigation churn attack via topic creation and deletion |
| S-10 | Identity grinding for targeted placement in the Harary ring |

### Structural — Design Gaps

| ID | Area |
|---|---|
| S-11 | Formal guarantees are heuristic and simulation-based |
| S-12 | Three gossip layers have no specified relative timing |
| S-13 | Topic registry: coordination not identity trust |
| S-14 | Catch-up prerequisites unavailable to subscriber |
| S-15 | Timestamps lack clock specification — cross-cutting across all three layers |
| S-16 | Cross-publisher ordering absent from retrieval model |
| S-17 | No forwarding incentive for disinterested nodes |
| S-18 | Protocol not equipped for messages with private value |
| S-19 | Persistence layer undermines gossip layer for notification use cases |

### Implementation-Level

| ID | Area |
|---|---|
| I-01 | Key rotation for topic owners |
| I-02 | Small topic subscriber sets |
| I-03 | Message deduplication mechanism unspecified |
| I-04 | Ring link latency unoptimized |
| I-05 | Sequence number gaps — lost events indistinguishable from gaps |
| I-06 | Online but missed — DHT query on convergence unspecified |
| I-07 | On-chain confirmation latency — compromised publisher window |
| I-08 | Registry behavior under chain rollback |
| I-09 | Privacy — encryption not designed |
| I-10 | Bootstrapping mechanism unspecified |
| I-11 | Historical publisher validation gap |
| I-12 | SecureCyclon certificate mechanism requires an unspecified PKI |
| I-13 | NAT traversal unaddressed |
| I-14 | Two delivery paths designed independently — no integrated subscriber behavior |
| I-15 | Topic log and event store writes not atomic |
| I-16 | Open topic attack surface — flooding and storage exhaustion |
| I-17 | Publisher signatures not bound to topic — cross-topic replay |
| I-18 | Subscription patterns observable through gossip |
| I-19 | Replication factor migration semantics unspecified |

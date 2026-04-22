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

## Severity Levels

Each observation in this document carries a severity tag indicating the impact of the issue if left unaddressed. Severity is orthogonal to the tier classification (Attack Vector / Design Gap / Implementation-Level): the tier describes what *kind* of issue it is; severity describes how *consequential* it is.

| Tag | Meaning |
|---|---|
| **Critical** | Invalidates a core claim or assumption of the protocol. The system cannot be safely deployed, or a stated security property does not hold, until this is resolved. No mitigation is designed in the report. |
| **High** | Realistic exploit or structurally significant gap with broad impact, but not protocol-invalidating. A plausible adversary can cause meaningful harm, or the absence of the design blocks advancement to later SRL levels. |
| **Medium** | Exploitable or gap-level issue whose impact is bounded, conditional on deployment choices, or whose mitigation is well-understood and deferrable. |
| **Low** | Edge case, explicitly stated scope constraint, minor detail to be specified in implementation, or easily addressed within existing primitives. |

Severity is assessed assuming no other changes to the protocol: the existence of an easy mitigation outside the report does not lower severity; whether the mitigation is *designed in the report* does. Use-case dependence lowers severity only when the dependence is explicit. Cascading enablers retain their own severity — an attack that enables additional attacks is not elevated on that basis, and the enabled attacks are rated on their own merits.

---

## Part I — Structural Observations

### Attack Vectors

#### S-01 · Navigation Eclipse via False Topic Membership Advertising

**Severity:** High — realistic adversary, broad reach across any targeted topic, and the redirection consequence lets the attacker select victims' peers; enables downstream exploits at the dissemination layer (S-02, S-03, S-06). No mitigation designed in the report.

The navigation layer uses Vicinity to build per-node views of which nodes are close to which topics. There is no mechanism to verify that a node actually subscribes to the topics it advertises. An adversary can claim membership in strategically chosen topics to maximize its appearance in other nodes' finger tables. By advertising membership in the most subscribed or most central topics, the adversary positions itself as a routing hub at the navigation layer — legitimate nodes' finger links will naturally point to it, giving it control over topic discovery routing for those topics. This attack operates entirely at the Vicinity layer and bypasses SecureCyclon, which provides no protection against false topic membership claims.

**Effect.** A successful attack has two compounding consequences: (1) denial of discovery — nodes subscribing to or recovering connectivity for the target topic fail to find legitimate peers, or do so only after long delay; (2) redirection — the adversary returns descriptors of other adversary-controlled nodes, placing the victim in an attacker-chosen neighborhood. Once the adversary selects a victim's topic neighbors, downstream attacks at the dissemination layer (S-02, S-03, S-06) become viable against that victim. Both new subscribers and existing nodes recovering from churn are affected.

---

#### S-02 · SecureCyclon Security Scope Is Misrepresented: Topology ≠ Delivery

**Severity:** High — a correctly-embedded adversary can silently suppress all traffic for a topic with no detection mechanism, directly undermining the delivery guarantees the protocol is implicitly taken to provide. Broad impact, applies to every use case. No detection or mitigation designed.

SecureCyclon defends against attacks on *who connects to whom* — hub attacks and eclipse attacks on the overlay topology. It does not defend against a correctly-embedded node that drops, delays, or selectively forwards messages. A node that passes all SecureCyclon certificate checks can suppress every message it receives for a given topic with no detection mechanism. The report implicitly treats overlay security as delivery security; these are distinct properties, and only the first is addressed.

---

#### S-03 · SecureCyclon Certificate Chains Do Not Extend to the Vicinity Layers

**Severity:** High — re-introduces hub attacks and selective-forwarding steering at the two layers that determine topic overlay structure, bypassing the protection SecureCyclon was introduced to provide. Broad impact across navigation and dissemination; the selective-forwarding variant is particularly hard to detect because nothing is forged. No certificate-chain extension designed.

SecureCyclon prevents descriptor fabrication through a certificate chain verified during peer sampling gossip exchanges. The navigation and dissemination layers use Vicinity, which applies no certificate-chain verification. A malicious node correctly embedded in SecureCyclon can, during Vicinity gossip, inject fabricated descriptors pointing to other malicious nodes — without a valid SecureCyclon certificate, because Vicinity does not check for one. This re-introduces the hub attack at the Vicinity layer, bypassing the protection SecureCyclon provides at the peer sampling layer.

A subtler attack is also unaddressed: a correctly embedded malicious node can selectively forward only descriptors of other malicious nodes that carry valid certificates, steering legitimate nodes toward a malicious neighborhood without fabricating anything. SecureCyclon prevents descriptor forgery; it does not prevent selective forwarding of legitimate descriptors.

---

#### S-04 · No Sybil Resistance for Gossip Layer Participants

**Severity:** Critical — invalidates a core assumption (bounded adversary fraction) of SecureCyclon's security analysis, which every certificate-chain-based guarantee rests on. Without Sybil resistance, an adversary trivially crosses the threshold and downstream properties collapse. The stake deposit mechanism applies only to replication servers, not to general gossip participants; no mitigation designed for the general case.

SecureCyclon's security analysis assumes a bounded fraction of malicious nodes. In a permissionless network, this holds only if creating a new node identity is costly. Standard asymmetric key pairs (e.g., Ed25519) can be generated in microseconds, so an adversary can trivially create thousands of distinct identities and violate the assumed adversary fraction with no computational barrier.

The report requires on-chain registration and a security deposit for replication servers, providing Sybil resistance for that specific role. No identity cost or Sybil resistance mechanism is specified for general gossip layer participants.

---

#### S-05 · Timestamp Manipulation Attack on SecureCyclon's Selection Function

**Severity:** High — trivial to execute (no cryptographic cost), broad impact (a single inflated-timestamp descriptor biases all peers' selection toward it and propagates across views), and directly undermines the view-convergence assumptions SecureCyclon's simulations depend on. The synchrony-compounding effect makes real-world severity worse than simulation suggests. No mitigation designed in the report; alternative selection rules require deeper analysis before they can be recommended.

SecureCyclon descriptors carry a creator-assigned timestamp used to establish freshness; newer descriptors are preferred in the selection process. A malicious node sets an artificially high timestamp on its own descriptor, making it appear fresher than all legitimate descriptors in the network. Legitimate nodes preferring fresher descriptors will favor the malicious one in their views — without any certificate violation, since the malicious node signed its own descriptor with its own key. The inflated timestamp persists through all subsequent transfers, as transfers add signatures but do not re-timestamp the original descriptor.

This attack is compounded by the synchrony assumption underlying SecureCyclon's security analysis: detection rates are derived from synchronized gossip cycles, but real deployments have heterogeneous hardware and variable network latency, reducing detection effectiveness under asynchronous operation.

**Note on the selection rule.** The timestamp-based freshness preference is not incidental — it serves two roles in SecureCyclon: (1) a *liveness signal*, since the certificate chain proves legitimate propagation but says nothing about whether the referenced node is still online, so without a freshness criterion views accumulate dead nodes; and (2) *replay resistance*, since a selection rule indifferent to age would let an adversary re-inject very old but validly-signed descriptors to poison views with stale nodes or nodes with rotated keys. Switching away from time-based replacement (e.g., to random selection or active liveness probing) is possible but requires deeper analysis of the impact on both properties before it can be recommended.

---

#### S-06 · Vicinity Has No Byzantine Resistance — Navigation and Dissemination Layers Are Undefended

**Severity:** Critical / High (use-case dependent) — Critical for use cases requiring Byzantine resistance (governance alerts, cross-chain coordination, DeFi-style settings with active adversaries): the foundational protocol makes no claim under adversarial conditions, so no navigation or dissemination property holds. High for cooperative-assumption use cases: no stated property is broken in the intended regime, but the structural absence leaves the layers undefended against any deviation from the cooperative assumption. No detection or recovery mechanism designed either way.

The Vicinity paper states explicitly that Byzantine behavior is beyond its scope. Both the navigation and dissemination layers in the AUEB report are built on Vicinity without addressing this gap.

These are attacks on overlay structure formation, not on message forwarding. A Byzantine node in the navigation layer can provide false routing pointers during the overlay construction phase, directing joining nodes to the wrong topic cluster and slowing or preventing convergence to the correct topic overlay. A Byzantine node in the dissemination layer can disrupt ring formation by presenting misleading neighbor selections during Vicinity gossip. Neither failure mode has a detection or recovery mechanism.

---

#### S-07 · Byzantine Failure Notifications Enable Ejection-by-False-Accusation

**Severity:** Medium — today's exploitable surface is bounded: false notifications trigger unnecessary data migration and operational disruption, but the higher-impact consequence (economic loss through honest-node slashing via coalition framing) is conditional on a future ejection/slashing mechanism that is not yet designed. Still non-trivial to mitigate — requires a real accusation/verification subsystem — and must be addressed explicitly when the ejection process is specified.

The maintenance protocol assigns each replication server to monitor the health of the servers storing the same events. When server p detects server q as unavailable, p broadcasts the failure to trigger data recovery across the DHT. A Byzantine p can send false failure notifications about a healthy q, triggering unnecessary data migration and causing other servers to treat q as unavailable.

No quorum, confirmation, or cross-verification mechanism for failure reports is described. The ejection criteria is also underspecified: who applies penalties, what evidence is required, and what prevents a coalition of Byzantine nodes from framing a healthy node are all open questions. While replication server security deposits could in principle be used to penalize false accusations, distinguishing intentional false accusations from accidental ones caused by transient network issues is non-trivial. A well-defined accusation and verification process must be designed before this mechanism can be safely implemented.

---

#### S-08 · Proof of Storage Is Acknowledged as Prohibitively Expensive — No Alternative Designed

**Severity:** Critical — the entire replication incentive and penalization design rests on enforceable storage verification. The report itself acknowledges PoR/PoRet are prohibitively expensive on-chain and defers this as "a separate line of research." Without a feasible verification mechanism, security deposits provide no real guarantee and the persistence layer has no working trust model. Self-acknowledged critical admission.

Section 4.1.2 states that Proof of Replication / Proof of Retrieval protocols "may be prohibitively expensive to execute directly on-chain, in a smart contract context." This is a critical admission: the entire incentivization and penalization mechanism depends on being able to verify storage compliance. Without a feasible on-chain verification mechanism, penalties cannot be enforced, and the security deposit model provides no real guarantees. The report defers this as "a separate line of research" without providing any interim design.

---

#### S-09 · Navigation Churn Attack via Topic Creation and Deletion

**Severity:** Medium — realistic resource-exhaustion attack with broad reach (every node bears churn cost), and the cost asymmetry favors the attacker (N × T churn for the cost of T registrations). However, on-chain registration fees provide a tunable mitigation lever already present in the system; a well-calibrated fee schedule or per-identity rate limits would materially raise attacker cost. Not rigorously designed in the report, but the primitives are there.

Every gossip node must maintain knowledge of all topics to compute finger distances in the navigation layer. Inserting or deleting a topic shifts the circular topic ring, requiring all nodes to recompute affected finger links and establish new connections. An adversary can register and delete topics repeatedly, forcing continuous navigation layer churn across all participating nodes. The cost to the attacker is linear in the number of topics created; the cost to the network is proportional to the number of participating nodes × topics affected. On-chain registration fees provide partial rate-limiting but not a defense.

---

#### S-10 · Identity Grinding for Targeted Placement in the Harary Ring

**Severity:** High — cheap and precise (seconds to minutes per target), places adversary nodes adjacent to chosen subscribers on chosen topics, and enables the full set of per-victim exploits (eclipse, message suppression, selective forwarding). Complements S-04 — even with global Sybil resistance, targeted grinding remains viable unless placement itself is randomized. No mitigation is designed in the report; candidate directions exist only as draft suggestions that require deeper analysis and may not hold up.

Node positions in the Harary dissemination ring are determined by sorted 256-bit node IDs: a node's ring neighbors are the nodes whose IDs are numerically adjacent to its own. This placement is a deterministic, public function of the node's identity key. S-04 describes the global Sybil threat of crossing an adversary-fraction threshold across the whole network. Targeted placement is a distinct threat with a much smaller budget: an adversary that wants to attack a specific topic mines Ed25519 keys until one falls adjacent to a target subscriber's ID on that topic's ring. Identity generation takes microseconds, so this requires seconds to minutes per target. The result is a small number of identities positioned as ring neighbors of chosen subscribers on a chosen topic, enabling targeted eclipse, message suppression, or selective forwarding without violating any network-wide Sybil bound.

This class of attack has been explored in the PRISM model developed in this workstream. Mitigations — identity binding to on-chain material (e.g., stake pool keys) or topic-specific unpredictable salts in the ring position function — are left for later design work.

---

### Design Gaps

#### S-11 · SecureCyclon and Vicinity Security Guarantees Are Heuristic and Simulation-Based

**Severity:** High — directly blocks advancement to SRL 3 (analytical verification) and SRL 4 (formal specifications), which are stated workstream objectives. Any formal model (Quint, PRISM) inherits the limitations of the foundational protocols it builds on. Broad impact: every property of every layer depends on primitives whose guarantees are empirical rather than analytically verified. Not Critical only because the empirical behavior at realistic scale is at least documented — the gap is between "demonstrated" and "verified," not between "works" and "doesn't."

SecureCyclon is presented in its source paper as a heuristic approach evaluated through PeerSim simulations. The paper's own results show that under high adversarial conditions (40% malicious nodes), a significant fraction of legitimate nodes still end up with a disproportionate number of links to malicious nodes — eclipse resistance degrades with adversary fraction rather than being guaranteed. The Vicinity paper has no formal proof of convergence, no bound on convergence time as a function of network parameters, and no security analysis under adversarial conditions. All properties are empirically demonstrated through simulation on networks of up to 65K nodes.

The AUEB report builds both its navigation and dissemination layers on Vicinity, and its peer sampling layer on SecureCyclon, without surfacing these limitations. Any formal specification — such as the Quint and PRISM models being developed — rests on foundational protocols whose properties are justified by heuristic argument and simulation alone. This is a material gap for advancing through SRL levels, where analytical verification of key parameters is required at SRL 3 and formal specifications at SRL 4.

---

#### S-12 · The Three Gossip Layers Have No Specified Relative Timing

**Severity:** Medium — real design gap with concrete impact (no system-level convergence guarantee, risk of pathological behavior at certain parameter settings such as ring thrashing or premature dissemination), but tractable with parameter analysis rather than new cryptographic or architectural work. Must be closed for any production deployment; the mitigation path is straightforward.

SecureCyclon, Vicinity (navigation), and Vicinity (dissemination) each gossip independently on their own periods. The relative timing of these three layers is not specified. If the peer sampling layer gossips much faster than the dissemination layer, it may continuously replace ring neighbors before they stabilize. If the dissemination layer gossips faster than the navigation layer, nodes may receive topic traffic before they have found the correct topic cluster. No analysis of inter-layer timing dependencies is provided, leaving the three-layer protocol without a coherent system-level convergence guarantee.

---

#### S-13 · The Topic Registry Provides Coordination, Not Identity Trust

**Severity:** Medium — identity trust is customarily an application-layer concern rather than a protocol responsibility, and a known mitigation pattern exists (Cardano native tokens solved the same shape of problem with a curated external registry). Impact depends on deployment choices: any serious notification use case will need an identity/trust layer on top, but the protocol itself is not expected to solve it. The report should acknowledge this as an explicit out-of-scope dependency rather than leaving it implicit.

The registry gives every participant a consistent, shared view of which topics exist and which public keys are authorized to publish. However, it cannot verify that the entity registering a topic is who it claims to be. Any actor can register a topic with any name, including names designed to impersonate a legitimate authority — for example, registering a topic named `governance/emergency` with a fraudulent publisher key.

This is structurally identical to the fake token problem on Cardano: anyone can mint tokens with any display name; the policy ID is the ground truth, but human-readable names collide freely. The ecosystem's response was a curated external registry mapping policy IDs to verified metadata. PubSub has the same structure and requires the same remedy: a trust layer above the registry binding topic ownership to verifiable identities. None of this is designed or acknowledged in the report.

---

#### S-14 · Catch-Up Requires Information the Subscriber May Not Have

**Severity:** High — crash, partition, and partial connectivity are the normal failure modes of any distributed subscriber, not edge cases. The catch-up path as specified works only for intentional clean shutdowns, which is the least common shutdown mode in practice. Every real subscriber is eventually affected. Mitigations exist (sequence-number anchoring, per-publisher last-seen state) but require redesigning the catch-up query interface, not a parameter tweak.

The catch-up workflow assumes the subscriber can provide the timestamp of when it went offline. This fails in common distributed systems scenarios:

- A subscriber that experienced a network partition has no reliable record of when it stopped receiving events.
- A subscriber that crashed and restarted has lost local state entirely, including the offline timestamp.
- A subscriber that was partially online — receiving some but not all gossip traffic — has no clean "went offline" event at all.

The timestamp-based query is only correct for intentional, clean shutdowns. For crash, partition, or partial connectivity, the recovery path is undefined.

---

#### S-15 · Timestamps Are Used in Multiple Contexts Without a Clock Specification

**Severity:** High — timestamps are load-bearing across all three gossip layers plus the catch-up path, and clock skew among honest publishers alone can cause valid events to be silently excluded from catch-up results. Silent correctness failures are particularly severe because subscribers cannot detect them. Compounds S-05. Mitigation requires specifying a clock model; no synchronization protocol or authoritative-clock designation is given in the report.

The report uses timestamps in at least three distinct contexts: the offline timestamp provided by the subscriber for catch-up queries; the timestamp stored in the topic log alongside the last sequence number per publisher; and the 5-second timeouts for failure detection pings. No clock synchronization protocol is specified, and no designation of whose clock is authoritative is provided. A publisher with a skewed clock could assign timestamps that fall outside a subscriber's expected recovery range, causing valid events to be silently excluded from catch-up results. This compounds the timestamp-related issues already identified at the protocol layer: creator-assigned timestamps in SecureCyclon descriptors enable freshness manipulation (→ S-05), and Vicinity's gossip exchanges similarly carry no timestamp verification. Across all three layers, timestamps are used as an assumed shared ground truth without any mechanism to establish or enforce it.

---

#### S-16 · Cross-Publisher Ordering Is Absent From the Retrieval Model

**Severity:** Low — this is a design constraint rather than a defect. The retrieval model is per-publisher by design; the gap is documentation (making the limitation explicit), not architecture. Use cases requiring cross-publisher ordering would be fundamentally ill-suited to this protocol regardless.

The catch-up mechanism retrieves events per publisher: for each publisher active on a topic, a subscriber requests all missed sequence numbers in order. This provides internally consistent ordering within a single publisher's stream but gives no mechanism to interleave events from multiple publishers chronologically.

*Example:* If publisher A sends A1, publisher B sends B1, then A sends A2, a recovering subscriber retrieves {A1, A2} from A's stream and {B1} from B's stream with no protocol-level information about how these interleave.

This should be stated explicitly as a known limitation: there is no well-defined notion of "the Nth event on topic T" — only "the Nth event from publisher P on topic T." The retrieval model is per-publisher by design, with no cross-publisher ordering semantics.

---

#### S-17 · Nodes Have No Incentive to Participate in Gossip for Topics They Do Not Subscribe To

**Severity:** High — under the intended shared-layer model, where the navigation layer serves many topics across participants with heterogeneous interests, rational free-riding causes the navigation layer to thin, finger tables to degrade, and the overlay structure assumed by the protocol to stop holding. Compounds S-19. No incentive mechanism is designed for general gossip participation (contrast with the replication-server deposit model). Severity reduces to Medium or Low in narrower deployment models (single-topic deployments, or settings where all participants subscribe to all hosted topics), but those are not the stated deployment target.

The navigation layer requires every node to maintain finger links to topics it does not subscribe to and respond to routing queries from other nodes. These are real operational costs — bandwidth, connection management, computational overhead — with no direct return for a node whose only interest is its own subscribed topic's messages. The report implicitly assumes SPOs will operate nodes for their own use and that the overhead is negligible. As message volumes or topic counts grow, the gap between the cost of participation and the benefit to the forwarder widens.

---

#### S-18 · The Protocol Is Not Equipped for Messages With Intrinsic Private Value

**Severity:** Low / Critical (use-case dependent) — Low when the protocol's scope is explicitly restricted to non-exclusive-value messages (the notification cluster, UC-1 through UC-6): the constraint is a documentation fix, not a design change. Critical for any deployment intended to serve messages with exclusive or competitive value (DeFi intents, competitive trading signals): rational recipients will suppress rather than forward, breaking dissemination at the economic layer. No mitigation is available inside this protocol — exclusive-value messages fundamentally require a different coordination model (auction, sealed-bid, encrypted mempool, etc.).

Gossip protocols assume cooperative forwarding. When a message's value is non-exclusive — equally useful to all recipients — there is no incentive to suppress it. The notification use cases (governance alerts, SPO announcements) largely satisfy this. However, when a message has exclusive or competitive value — only the first actor to receive and act on it benefits, or the value is destroyed by sharing — the rational strategy upon receipt is suppression rather than forwarding.

The protocol makes no structural distinction between notification messages with non-exclusive value and messages with exclusive value. This should be documented as a designed-in scope constraint: the gossip protocol is appropriate for notifications whose value is non-exclusive and unsuitable for messages where recipient value depends on exclusivity or competitive advantage.

---

#### S-19 · The Persistence Layer Structurally Undermines the Gossip Layer for Notification Use Cases

**Severity:** High — for the notification cluster (UC-1 through UC-6, the bulk of the stated use cases), the three-layer gossip protocol competes with a much simpler polling alternative built on the replication layer. Rational subscribers will choose polling, thinning the gossip overlay and compounding S-17. This questions whether the protocol's core architectural choice is the right fit for its stated target. Mitigation is either scope restriction (use the protocol for use cases that genuinely need real-time gossip) or architectural redesign (drop gossip for notifications) — neither is a small tweak.

The gossip layer provides real-time delivery. The replication layer provides reliable catch-up retrieval. For notification use cases — where messages are infrequent and latency is measured in minutes rather than milliseconds — a subscriber polling replication servers periodically achieves functionally equivalent results without running a gossip node. If enough subscribers make this choice, the gossip overlay thins, dissemination becomes slower and less reliable, further reducing the incentive to participate.

This also reframes the minimum viable design question: for the notification use case specifically, a design where publishers write directly to replication servers and subscribers poll may be simpler, more reliable, and sufficient — making the three-layer gossip protocol unnecessary complexity for the actual target use cases.

---

## Part II — Implementation-Level Observations

The following observations are lower-level gaps and unspecified details to be addressed in later design phases. They do not represent immediate structural risks but must be resolved before the system can be implemented.

---

**I-01 · Key rotation for topic owners.** The owners list contains public keys with no key rotation or recovery mechanism. An attacker who steals an owner key can add themselves as a permanent owner, remove legitimate owners, modify the publisher list, or delete the topic. *[Severity: High — a single stolen owner key permanently alters the topic (attacker becomes permanent owner, legitimate owners removable, topic deletable) with no recovery path. For authority-sensitive topics such as governance or emergency alerts, impact extends to every trusting subscriber. Standard mitigation patterns (multisig, rotation ceremonies, deposit-based recovery) exist but none is designed in the report.]*

**I-02 · Small topic subscriber sets.** The Harary ring with connectivity t requires at least t+1 nodes. For topics with very few subscribers (1–5 nodes), the ring degrades to a trivial path or a single node with no meaningful fault-tolerance properties. Minimum subscriber counts and fallback behavior are unspecified. *[Severity: Medium — behavior is undefined in the degenerate low-subscriber regime, which is common for several notification use cases (e.g., SPO-to-delegator). The protocol still functions, but its fault-tolerance claims do not hold. Mitigation is well-understood (define minimum subscriber counts and a fallback to direct replication-server polling below threshold) but unspecified.]*

**I-03 · Message deduplication mechanism unspecified.** When a node receives a message it has already seen, it discards it. The mechanism for tracking seen messages is not described: data structure, retention period, eviction policy. An unbounded seen-list is a memory exhaustion vector for long-running nodes on high-traffic topics. *[Severity: Medium — resource-exhaustion concern for any long-lived, high-traffic deployment. Mitigation space is standard (bloom filters, time-windowed caches, sequence-number-based dedup) and well-understood, but needs specifying before implementation. Can be weaponized by a flood attacker — see I-16.]*

**I-04 · Ring link latency.** The Harary ring orders nodes by arbitrary 256-bit IDs. Adjacent IDs are not geographically or topologically close, so ring links — the reliability backbone of the dissemination layer — may be systematically higher-latency than the random links, inverting the intended performance profile. *[Severity: Medium — material architectural concern affecting every deployment; the reliability backbone may be worst-case routed. Not exploitable as an attack and delivery still functions, just slower than intended. Mitigation is standard (latency-aware ring construction, locality-sensitive ID assignment, or locality-aware placement functions).]*

**I-05 · Sequence number gaps.** The catch-up scheme increments per-publisher sequence numbers until a lookup returns nothing. If a publisher crashes after partially submitting an event (signed but not yet stored by replication servers), the subscriber increments past the gap and concludes it is up-to-date. There is no mechanism to distinguish "no event was published with this sequence number" from "an event was published but was lost before reaching the DHT." *[Severity: High — silent correctness failure: a subscriber believes it is up-to-date while a published event is permanently missing from its view, and cannot detect the condition. Applies to every deployment using the stated catch-up design. Mitigation requires a real design change (publisher-signed gap attestations, on-chain sequence-number anchoring, or two-phase commit between event storage and topic-log update — related to I-15), not a parameter tweak.]*

**I-06 · Online but missed — DHT query on convergence.** A newly joined node whose ring has not yet converged may miss events with no recovery path. The mitigation is straightforward but unspecified: once a node infers it has converged to its position in the topic overlay, it should proactively query the DHT to fill any gap from the convergence window. *[Severity: Low / Medium (use-case dependent) — Low for use cases where the subscriber's interest is only from join time forward (e.g., live operational alerts): the convergence window is just "before join," not a defect. Medium for use cases requiring historical completeness (e.g., governance vote notifications, subscription-state alerts): events published during convergence are silently missed. Mitigation is straightforward and inside existing primitives; just unspecified.]*

**I-07 · On-chain confirmation latency.** Topic registry updates are confirmed on-chain only after block finality (~20 seconds with Ouroboros Praos). During this window, removed publishers can still inject messages that will be forwarded and stored. Adding a confirmation delay (waiting N blocks before acting on registry changes) mitigates most lag window issues. However, for the compromised publisher case, an actively exploited key continues injecting messages during both the confirmation window and any additional delay. *[Severity: Medium — known tradeoff inherent to chain-backed registries; the mitigation path (N-block confirmation delay) is standard parameter calibration. The active-key-compromise subcase compounds I-01 rather than standing alone.]*

**I-08 · Registry behavior under chain rollback.** I-07 addresses the confirmation-latency window: the interval between a registry transaction appearing on-chain and being safe to act on. Distinct concern: Cardano's settlement model permits rollbacks of up to K blocks. A topic registration, publisher-list change, or `setReplicationFactor` call that is confirmed but not yet deeply finalized can be reverted. The report specifies no rollback-handling protocol for off-chain state derived from on-chain events: how nodes detect that a registry action has been undone, what happens to messages signed under a now-reverted publisher list, and whether replication servers excise events that were accepted under a reverted owner configuration. Treating settled blocks as final compresses the confirmation window at the cost of rollback hazards; the tradeoff is not analyzed. *[Severity: Medium — real correctness gap, but mitigation is well-understood in principle: wait a safe number of blocks before acting on registry state so rollbacks are minimized in practice, and specify excision procedures for the rare deeper rollbacks. Standard practice for Cardano integrations. Strictly harder than I-07 because it requires retroactive handling, but the design space is known.]*

**I-09 · Privacy — encryption not designed.** The report acknowledges all topics are public and states that "if privacy is required, encryption should be employed." No encryption mechanism, key distribution scheme, group key management, or rekeying protocol is designed. *[Severity: Low / High (use-case dependent) — Low for the public-notification cluster (governance alerts, SPO announcements, DApp public notifications): topics are public by design and the deferral is reasonable. High for any deployment requiring confidentiality: group encryption with key distribution, member onboarding, rotation, and forward secrecy is a substantial cryptographic subsystem. Standard patterns exist (MLS, Signal group messaging, proxy re-encryption) but integrating one into this protocol is non-trivial.]*

**I-10 · Bootstrapping mechanism unspecified.** New nodes need an initial set of contacts to begin participating in SecureCyclon. The bootstrap mechanism — and how the certificate chain requirements interact with the initial join process — is not described. *[Severity: Medium — on the critical path to any deployment: without a defined bootstrap process, no node can join. Interaction with SecureCyclon's certificate chain is non-trivial (first-node chain seeding, trusted bootstrap contacts). Standard solutions exist (seed nodes, DNS bootstrap, on-chain-registered bootstrap set) but none specified.]*

**I-11 · Historical publisher validation gap.** The topic registry reflects the current publisher list, not its history. A node catching up on historical events cannot validate signatures from publishers that have since been removed. A replication server presenting events from removed publishers cannot be challenged, nor can it be verified that events from removed publishers are being correctly excluded. An on-chain history of publisher list changes, or timestamp-scoped registry queries, would be needed to validate historical events correctly. *[Severity: Medium — real validation gap affecting catch-up from before a publisher-list change; affects the integrity of historical event streams. Publisher removal events are presumably infrequent but often coincide with the exact sensitive case (compromise response, malicious-publisher ejection) where historical validation matters most. Mitigation is standard chain-integration work (on-chain publisher-list history or timestamp-scoped queries), no new design research required.]*

**I-12 · SecureCyclon certificate mechanism requires an unspecified PKI.** SecureCyclon requires each node to sign its descriptor and each subsequent transfer, forming a certificate chain that proves legitimate ownership through each intermediate holder. This requires per-node asymmetric key pairs and a PKI to bootstrap and verify them. The report does not specify how nodes generate these keys, whether they are the same as on-chain identity keys, how revocation works, or how initial key material is bootstrapped. *[Severity: High — PKI is load-bearing for multiple properties: certificate-chain validity (the core of SecureCyclon's security argument), key revocation after compromise, and bootstrap trust. Without a specified PKI, the certificate-chain mechanism is structurally incomplete. Mitigation patterns exist (standard PKI or on-chain identity binding via SPO/stake keys) but none is designed in the report. Related to S-04, which captures the core-assumption impact at Critical.]*

**I-13 · NAT traversal unaddressed.** The report assumes nodes connect directly to each other via IP address and port. If light clients are expected to participate in the gossip overlay — which the report does not rule out — NAT traversal becomes a blocker: the majority of light clients operate behind NAT and cannot receive inbound connections. The transport layer requirements are never specified. *[Severity: Low / High (use-case dependent) — Low for SPO-only or infrastructure-operator deployments: public IPs are the norm, NAT traversal is a non-issue, and the gap reduces to specifying the transport layer explicitly. High for any deployment including mobile or browser light clients (relevant to UC-2, UC-3, UC-5, UC-6 where end users are in the actor set): NAT traversal is a whole additional transport subsystem (STUN/TURN, WebRTC, relay infrastructure) plus changes to the gossip overlay to handle nodes that cannot accept inbound connections. The scope determines which case applies; deployment intent for light-client participation is not stated in the report.]*

**I-14 · Two delivery paths designed independently — no integrated subscriber behavior.** The live gossip path (Chapter 3) and the catch-up DHT path (Chapter 4) are designed independently. A real subscriber must use both simultaneously: receiving live events via gossip, detecting gaps using sequence numbers, filling gaps via DHT, and reconciling two asynchronous streams. This integrated subscriber behavior — buffering, gap detection, DHT queries, stream reconciliation — is entirely absent from the report. A related issue is that on the gossip path, messages arrive in an unpredictable order and carry no sequence number in their payload. A subscriber has no intrinsic way to establish the publisher-intended order of messages without relying on replication servers as an external authority — which defeats the purpose of the live gossip path as an independent delivery mechanism. Additionally, the publisher's signature covers message content but does not bind it to the sequence number, so a replication server can reorder messages within a publisher's stream while keeping all signatures valid, with no way for the subscriber to detect this at the routing level. *[Severity: High — bundles three gaps: (a) no integrated subscriber behavior across live + catch-up paths; (b) gossip path arrives unordered and without sequence numbers in payload, forcing reliance on replication servers as an order authority and defeating the independence of the gossip path; (c) signatures do not bind sequence numbers, allowing undetectable reordering by replication servers — a silent integrity failure even in the non-adversarial case. Mitigation directions exist (sign `seq || content`, include sequence numbers in gossip payload, design the integrated subscriber state machine) but all require protocol changes and affect every deployment.]*

**I-15 · Topic log and event store writes are not atomic.** When a publisher submits an event, two distinct writes must occur: the event stored at `hash(TOPIC · PUBLISHER · SEQUENCE_NR)` on the DHT, and an update to the topic log at `hash(TOPIC)` recording the latest sequence number. No distributed transaction protocol is described. If the topic log is updated before all replicas are stored, a recovering subscriber reads "last sequence = N" but the request for event N returns nothing — with no way to distinguish "not yet replicated" from "never published." *[Severity: Medium / High (use-case dependent) — Medium for low-frequency notification use cases (governance alerts, emergency announcements): the indistinguishability window is brief and typically self-resolves via retry. High for high-frequency deployments: the race window persists under continuous write pressure, and subscribers face persistent three-way ambiguity ("not yet replicated" vs "never published" vs "published but lost") that compounds with I-05. Mitigation patterns exist (two-phase commit, write-ordering barriers, explicit pending markers) but require protocol-level design work in either case.]*

**I-16 · Open topic attack surface: flooding and storage exhaustion.** If the publishers list is empty, any node can publish to a topic. The report provides no rate-limiting or anti-spam mechanism. An adversary can flood an open topic with high-frequency messages, consuming forwarding bandwidth across all subscribed nodes; replication servers are obligated to store every message that passes the publisher signature check, so a sustained attack also exhausts replication server disk storage. The security deposit mechanism creates no remedy — complying with the protocol is the attack. This gap is explicitly called out as functional requirement FR5.1 but is never addressed in the design. *[Severity: Low / High (use-case dependent) — Low for deployments that never use open topics (every topic carries a defined publisher list): the attack surface does not exist. High for any deployment supporting open topics: trivially executable, exhausts shared network resources, and the deposit mechanism offers no defense since compliance *is* the attack. Mitigation space is known (rate limits, per-topic quotas, proof-of-work, stake requirements for open-topic publishing) but nothing is specified despite FR5.1.]*

**I-17 · Publisher signatures not bound to topic — cross-topic replay.** A message signed by a publisher is not a priori bound to a specific topic, so a malicious node could replay a signed message into another topic where the same publisher is also listed — every signature check passes. A simple mitigation is for publishers to use distinct key material per topic. Noted here for documentation purposes. *[Severity: Low — mitigation is trivial (distinct keys per topic, or domain-separated signatures over `TOPIC || seq || content`). Requires the same publisher to be listed in multiple topics, which is avoidable by operational discipline. Recorded as a documentation item rather than a design concern.]*

**I-18 · Subscription patterns observable through gossip.** I-09 notes that payload encryption is not designed. Separately, subscription privacy — knowledge of which topics a given IP or node relays, catches up on, or serves ring-neighbor traffic for — is observable through traffic analysis by an adversary running gossip nodes across the overlay. This is independent of payload encryption: encrypted content does not hide the fact that a subscriber relays encrypted traffic for a specific topic. For use cases where subscription itself is sensitive — voting behavior, SPO affiliation, geographically scoped subscriptions — metadata privacy is a distinct requirement from payload privacy and is structurally hard to retrofit into gossip overlays. *[Severity: Low / High (use-case dependent) — Low for use cases where subscription is already public or non-sensitive. High for use cases where subscription itself is sensitive: metadata privacy is architecturally hard to retrofit into gossip overlays. Standard mitigations (mix networks, cover traffic, onion routing) add substantial complexity and cost.]*

**I-19 · Replication factor migration semantics unspecified.** Owners can change a topic's replication factor via `setReplicationFactor`, but the migration protocol is not defined: whether excess replicas are purged or retained, whether new replicas are created eagerly or lazily, and how the system behaves during the transition are all unspecified. If a topic owner is compromised, changes to this parameter may cause harm whose severity depends both on the semantics the protocol ultimately adopts and on the use case. The observation here is the undefined behavior itself; impact cannot be assessed further until migration semantics are specified. *[Severity: Low — the observation is deliberately scoped to "undefined behavior in need of specification." No silent correctness failure, no resource exhaustion, no exploitable surface without further design choices. Once migration semantics are specified, any new concerns would surface as new observations.]*

---

## Summary

### Structural — Attack Vectors

| ID | Severity | Area |
|---|---|---|
| S-01 | High | Navigation eclipse via false topic membership advertising |
| S-02 | High | SecureCyclon scope: topology ≠ delivery security |
| S-03 | High | Certificate scope regression at Vicinity layers |
| S-04 | Critical | No Sybil resistance for gossip participants |
| S-05 | High | Timestamp manipulation on SecureCyclon selection function |
| S-06 | Critical / High (use-case dependent) | Vicinity Byzantine resistance absent — overlay structure formation undefended |
| S-07 | Medium | Byzantine failure notifications / ejection-by-false-accusation |
| S-08 | Critical | Proof of storage undesigned — penalties unenforceable |
| S-09 | Medium | Navigation churn attack via topic creation and deletion |
| S-10 | High | Identity grinding for targeted placement in the Harary ring |

### Structural — Design Gaps

| ID | Severity | Area |
|---|---|---|
| S-11 | High | Formal guarantees are heuristic and simulation-based |
| S-12 | Medium | Three gossip layers have no specified relative timing |
| S-13 | Medium | Topic registry: coordination not identity trust |
| S-14 | High | Catch-up prerequisites unavailable to subscriber |
| S-15 | High | Timestamps lack clock specification — cross-cutting across all three layers |
| S-16 | Low | Cross-publisher ordering absent from retrieval model |
| S-17 | High | No forwarding incentive for disinterested nodes |
| S-18 | Low / Critical (use-case dependent) | Protocol not equipped for messages with private value |
| S-19 | High | Persistence layer undermines gossip layer for notification use cases |

### Implementation-Level

| ID | Severity | Area |
|---|---|---|
| I-01 | High | Key rotation for topic owners |
| I-02 | Medium | Small topic subscriber sets |
| I-03 | Medium | Message deduplication mechanism unspecified |
| I-04 | Medium | Ring link latency unoptimized |
| I-05 | High | Sequence number gaps — lost events indistinguishable from gaps |
| I-06 | Low / Medium (use-case dependent) | Online but missed — DHT query on convergence unspecified |
| I-07 | Medium | On-chain confirmation latency — compromised publisher window |
| I-08 | Medium | Registry behavior under chain rollback |
| I-09 | Low / High (use-case dependent) | Privacy — encryption not designed |
| I-10 | Medium | Bootstrapping mechanism unspecified |
| I-11 | Medium | Historical publisher validation gap |
| I-12 | High | SecureCyclon certificate mechanism requires an unspecified PKI |
| I-13 | Low / High (use-case dependent) | NAT traversal unaddressed |
| I-14 | High | Two delivery paths designed independently — no integrated subscriber behavior |
| I-15 | Medium / High (use-case dependent) | Topic log and event store writes not atomic |
| I-16 | Low / High (use-case dependent) | Open topic attack surface — flooding and storage exhaustion |
| I-17 | Low | Publisher signatures not bound to topic — cross-topic replay |
| I-18 | Low / High (use-case dependent) | Subscription patterns observable through gossip |
| I-19 | Low | Replication factor migration semantics unspecified |

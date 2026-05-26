# PubSub Technical Report 1: Three-Layer Stack Findings and a Path Forward

## 1. Summary

The workstream has been evaluating the inherited three-layer pubsub design: SecureCyclon for peer sampling, Vicinity for topic navigation, a hybrid dissemination layer on top. Formal analysis has falsified the uniformity assumption that the upper layers depend on, and surfaced further bias and attack surface in the upper layers themselves. As a result, the three-layer stack is no longer being pursued as the prototype direction.

The team has aligned on a **list-based architecture** as the first-step replacement. Peer sampling and navigation collapse into an on-chain subscription list (one entry per subscribed node, each carrying the node's topic-interest set), the dissemination layer is kept unchanged, and the module interfaces are held fixed so the implementation can later be swapped for a *multi-peer-sampling protocol* delivered by research.

---

## 2. Where we started

The inherited design has three stacked layers, specified in the original research paper *[Cardano Pub/Sub Framework: Design and Architecture](D2-Cardano-PubSub-Framework-Design-and-Architecture.pdf)*:

- **SecureCyclon.** Gossip-based peer sampling. Each node holds a small random view of the network. Views are refreshed by descriptor exchanges, with built-in defences against hub formation and frequency-based eclipse.
- **Vicinity.** Orders same-topic peers into a ring by node ID (the public key) and maintains finger links to nearby topics.
- **Hybrid dissemination.** A Harary `H(N,2)` ring backbone plus random links drawn from the Cyclon view.

Every higher-layer security argument in this design substitutes *"sample uniformly from a node's view"* for *"sample uniformly from the network"*. That substitution is load-bearing: if Cyclon does not produce uniform-looking views, the bounds derived on top of it are unsound. The whole stack rests on it ([logbook, pivot to concrete output](../logbook.md#2026-05-05--weekly-pivot-to-concrete-output)).

---

## 3. What the formal analysis found

### 3.1 The base layer does not produce uniform overlay graphs

The formal-methods workstream framed Cyclon's claimed uniformity as three properties of increasing strength and tested each empirically. The properties, their statements, and the mechanism by which the bias arises are documented in [cyclon_properties_report.md](../formal_spec/peer_sampling/cyclon/cyclon_properties_report.md). See in particular [D1.3 falsification](../formal_spec/peer_sampling/cyclon/cyclon_properties_report.md#3-d13--overlay-graph-uniformity-falsified-under-deterministic-initiation-restored-under-poisson-initiation). Summary:

| Property | Statement | Status |
|---|---|---|
| **D1.1** Marginal uniformity | Every node has the same probability of being in any other node's view | Holds |
| **D1.2** Per-view uniformity | Each individual view is statistically a uniform random subset of the network | Holds |
| **D1.3** Overlay-graph uniformity | The whole graph is indistinguishable from a uniform random `c`-out digraph | **Falsified** |

D1.3 is the property that licenses *sample-from-view ≡ sample-from-network* in eclipse and partition arguments. Under the deterministic per-cycle initiation rule Cyclon specifies, D1.3 is falsified at every tested network size. Total-variation distance from uniform grows monotonically with N and exceeds 0.48 at N=10⁴. The deviation is structural, not a finite-size artifact: deterministic self-injection combined with in-degree-proportional partnering concentrates the in-degree distribution *tighter than uniform*.

Two implications:

- **The bias is invisible to any single node.** Each view looks uniform on its own (D1.2 holds). The bias lives in the joint structure across views, which is what the upper layers depend on.
- **The known fix has a cost.** Switching to Poisson per-cycle initiation restores D1.3 at the marginal level, but breaks SecureCyclon's frequency-check defence against hub attacks. Honest nodes would trigger the check roughly two cycles in three (see [Trade-off with SecureCyclon](../formal_spec/peer_sampling/cyclon/cyclon_properties_report.md#trade-off-with-securecyclon)). The patches do not compose cleanly.

### 3.2 The upper layers add their own bias and are independently vulnerable

The research design assumes identity creation is costly (the Sybil-resistance assumption that underpins SecureCyclon's adversary-fraction bound). It does not specify a mechanism for that cost, and it does not address *position-targeted grinding* even if each identity is costly. The upper-layer attack surfaces below all depend on that gap.

- **Vicinity ring positions can be ground.** Vicinity orders same-topic peers in a ring by self-selected node IDs (public keys). With identity cost in place, an adversary can still generate many costly identities and select the ones whose IDs land in a target's neighbourhood ([m2_eclipse_report.md](../formal_spec/hybrid_dissemination/partitioning/golden_tier/m2_eclipse_report.md)). The analysis treats the deterministic link layer as adversarially unreliable for this reason. Closing this surface requires position derivation that an adversary cannot bias, for example a verifiable random function over a per-epoch nonce. The inherited design does not specify any such mechanism.
- **Per-topic views lose uniformity even when the global property holds.** Organising dissemination per topic via the navigation layer means per-topic views do not inherit Cyclon's uniformity. The available mitigation is a separate Cyclon instance per topic, which is costly ([logbook, Cyclone Property 3 session](../logbook.md#2026-05-12--pubsub-working-session-cyclone-property-3-spo-onboarding)). It also remains an open question whether, even if the *input* to Vicinity is uniform, the *output* is uniform, and how much an adversarial fraction can bias that output.
- **Misaligned incentives at the navigation layer.** Vicinity expects same-topic peers to forward each other's messages. A subscriber to one topic is asked to relay messages of other topics they may actively oppose. The "we love cats" subscriber is asked to forward "we hate cats" traffic. An adversary can pretend to subscribe to every topic to maximise its observation and forwarding footprint.
- **Two adversarial nodes can partition any subscriber, independent of network size.** With open subscription, self-selected IDs, and no specified anti-grinding mechanism, two adversarial neighbours isolate any specific subscriber with probability `e^{-RF}` (around 13.5% for two random links per node). Adding subscribers does not help; only raising the random-link fanout does ([adversarial_partition_report.md](../formal_spec/hybrid_dissemination/partitioning/adversarial_partition_report.md), which explicitly notes that *"the architecture does not specify any mechanism to prevent ID grinding"*).

### 3.3 Two silent attack vectors at the peer-sampling layer

Two attacks are difficult to detect or punish because both are invisible against legitimate behaviour:

- **Descriptor drop.** A malicious peer absorbs a node descriptor instead of forwarding it, silently eclipsing the originator by one descriptor at a time. The attack is bounded in the standard analysis: outgoing-link count makes a fully malicious neighbourhood unlikely, and the originator re-shares in the next round ([logbook, descriptor-drop discussion](../logbook.md#2026-05-12--pubsub-working-session-cyclone-property-3-spo-onboarding)). Bounded is not free. At adversarial concentrations approaching the SecureCyclon assumption boundary, descriptor drop becomes a slow eclipse with no protocol-level detector.
- **Biased view response.** When node A asks peer B for `k` peers to populate A's view, A must trust that B sampled without bias. SecureCyclon's frequency check defends against a peer *initiating exchanges too often*, but does not check *which peers* are returned. An adversary controlling B can return biased samples, and the receiving node has no local check. A biased view still looks uniform from inside (D1.2 is content-level, not source-level). Formal analysis of this attack — two internal deviations, neither of which produces any externally observable anomaly — confirms that all nine of SecureCyclon's mechanisms miss it entirely: none of D1–D9 fire ([cyclon_silent_bias_attack.md](../formal_spec/peer_sampling/cyclon/cyclon_silent_bias_attack.md)). SecureCyclon's own paper acknowledges this attack class but explicitly scopes it out of the adversary model (§II.C). Empirically, at a 10% adversary fraction the attack achieves a 3.9× amplification (honest views become ~40% adversary-pointing); at 15% the figure reaches 93% saturation. The attack reaches equilibrium within ~60 cycles and persists indefinitely with no natural decay.

Both attacks exploit the same assumption: that other peers are honest in how they sample. That assumption is the surface gossip-based sampling actually offers. Removing it requires either heavy protocol additions (signed-descriptor chains, equivocation proofs, sketched in [extensions/](extensions/)) or removing gossip-based sampling itself.

### 3.4 The layers are structurally at odds

SecureCyclon's stated philosophy is *non-bias*: a network with X% malicious nodes yields X% malicious links, preventing over-representation. Vicinity does the opposite. It deliberately introduces structure (topic-clustered, ID-ordered ring) so the dissemination layer can derive coverage and latency guarantees. The dissemination layer mixes those deterministic links with random ones drawn from Cyclon, importing whatever bias Cyclon carries.

The upper layers cite the uniform-sampling assumption that the base layer was supposed to provide, while themselves adding the topology and the grinding surface that make eclipse and partition attacks possible. Fixing one layer's property tends to break another (the Poisson example above). Each layer's security has to be argued more or less standalone. Inheritance up the stack is not free.

---

## 4. Alternatives considered

Three directions were on the table.

**A. Repair and extend the three-layer stack.** Resolve the Cyclon hub-defence ↔ uniformity trade-off, close Vicinity's grinding surface, detect or bound descriptor-drop and biased-view attacks, and re-derive eclipse and partition bounds without the uniform-view assumption. None of these has a finished specification.

**B. Multiple parallel SecureCyclon instances, one per topic.** Recovers per-topic uniformity if SecureCyclon's underlying issues are repaired, but scales poorly with the number of topics a node subscribes to and inherits descriptor-drop and biased-view attacks unchanged.

**C. Collapse peer sampling and navigation behind an on-chain subscription list.** Each entry carries the node's topic-interest set; per-topic queries are filters over the list. Sampling becomes a local computation. The dissemination layer is unchanged.

---

## 5. Decision and rationale

The team has aligned on **option C, the list-based architecture**.

Rationale, in order of weight:

1. **Removes the falsified load-bearing assumption.** Peer sampling becomes a local computation over an on-chain list; the upper layers no longer cite a uniformity property the base layer does not deliver.
2. **Closes the sampling-layer form of two silent attacks.** Descriptor drop and biased view response disappear when sampling is a local computation over the list. A weaker form persists at the contact-information layer, but target existence is guaranteed by the list, so endpoint resolution can be retried against any other reachable peer. Note: a VRF-based subset-selection mechanism could neutralise the biased-pick component of the view-bias attack in a gossip-based design, but the silent-drop component has no known deterministic defence — making the gossip approach a partial fix at best ([cyclon_silent_bias_attack.md §6](../formal_spec/peer_sampling/cyclon/cyclon_silent_bias_attack.md#6-mitigation-idea-sketch)).
3. **Shippable on the existing timeline.** The on-chain topic registry already exists in Quint with an Aiken implementation in flight, and the subscription list reuses the same contract patterns.
4. **Pragmatic Byzantine posture.** A small bootstrap set is treated as trusted infrastructure. The assumption is explicit, narrow, and revisable.
5. **Not a one-way door.** The module interfaces are held fixed, so a future swap to the multi-peer-sampling protocol is a module-level change.

Acknowledged costs:

- **On-chain fees and footprint.** Subscribe, unsubscribe, and topic-interest updates are Cardano transactions; volume scales with churn, contract state with active subscribers.
- **List-view manipulation.** A node trusting a single chain follower can be lied to about list state. Mitigation: multi-source verification, light-client sync, or a local follower.
- **Local cheating.** Operators with full per-topic visibility can deviate from prescribed sampling undetectably. The multi-peer-sampling research handoff is what removes this.
- **Privacy.** Subscriber keys and topic interests are durable on-chain and publicly aggregable. Fine for operator-class participants, material for private subscribers.

---

## 6. Research handoff

The decision splits the work cleanly. Engineering proceeds on the list-based architecture. Research is asked to define a protocol that lets the list be retired without re-architecting consumers.

**The multi-peer-sampling problem (target of the research handoff).**

> Given a network in which each node is associated with a subset of topics, design a protocol that allows each node to **sample uniformly at random from the set of subscribers of a given topic**, without requiring the node to hold the full subscription list.

A protocol that delivers this primitive can replace the list-based sampling implementation without changes to the dissemination layer. With it in place, the local-cheating surface (full per-topic visibility) closes, and the on-chain footprint shifts from "the whole list" to "the protocol's coordination data".

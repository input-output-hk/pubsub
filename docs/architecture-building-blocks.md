# PubSub Architecture — Building Blocks & Scoping

Based on Ezequiel's review (common denominator: authenticated notification
primitive), the AUEB research (D1/D2), and security analysis of the
three-layer protocol.

The idea: define a minimal base that works, then treat additional
capabilities as composable building blocks that can be added
independently.

---

## Base Use Case

**Authenticated notification delivery:** identifiable parties (governance
bodies, SPOs, DApp teams) publish verifiable events to interested
subscribers (wallet backends, SPO nodes), with best-effort delivery and
no mandatory long-term persistence.

**Participant profile:** primarily SPOs and infrastructure providers.
Low churn (join/leave at epoch boundaries). Low adversarial incentive
to suppress notifications. Hundreds to low thousands of nodes.

---

## Properties & Building Blocks

### Tier 1 — Fundamental (must have for base use case)

| Property | What it means | Building block | Status | Notes |
|----------|--------------|----------------|--------|-------|
| Topic management | Create/delete topics, manage roles (owner/admin/publisher) | On-chain Topic Registry smart contract | Quint spec complete, needs Plutus/Aiken implementation | D2 Ch.2, formal spec covers this fully |
| Publisher authentication | Messages are signed, subscribers verify sender identity | Static-key signing + on-chain publisher lists (prototype); DID-based identity as Tier 2 upgrade | Design exists, needs implementation | Deliverable D2. Static keys are sufficient for the base use case — subscribers only need to verify the signer is in the on-chain publisher list. DIDs add key rotation, delegation, and multi-device support but are not required for the prototype. |
| Message dissemination | Published messages reach all online subscribers | Gossip protocol (random links at minimum) | AUEB design exists, needs prototype | Core of D1 deliverable |
| Peer discovery | New nodes find same-topic peers | SecureCyclon (peer sampling) + Vicinity (topic-aware clustering) | SecureCyclon + Vicinity designed | D2 Ch.3.3. SecureCyclon is required even with RandCast: it prevents hub formation, link manipulation, and eclipse attacks at the peer sampling layer. Without it, an adversary can poison a node's random view, compromising Vicinity from the start. Vicinity remains necessary regardless of dissemination choice — SecureCyclon provides random peers network-wide, Vicinity clusters peers by topic proximity. Upper-layer descriptor signing (Tier 2) complements but does not replace SecureCyclon. |
| Crash fault tolerance | Network stays connected when nodes fail/leave | Redundant links (random or structured) | Harary graph designed, RandCast alternative under analysis | PRISM model partially covers this |
| Basic Sybil resistance | Joining the network has a cost | On-chain identity (staking credential requirement) | Natural fit with SPO model, not formally designed | Implicit in the SPO participant model |

### Tier 2 — Nice to Have (strengthens base, adds resilience)

| Property | What it means | Building block | Dependency | Notes |
|----------|--------------|----------------|------------|-------|
| Byzantine fault tolerance | Network delivers messages even with actively malicious nodes | Descriptor signing at all layers + unpredictable ring position (VRF) | Tier 1 dissemination | Current design has gaps (Ezequiel's Vicinity concern, positional Sybil). Deliverable D1 scope asks for this. |
| Deterministic delivery guarantee | Mathematical guarantee that messages reach all honest nodes under bounded failures | Harary graph (RingCast) | Tier 1 peer discovery + ring convergence protocol | Only valuable if BFT ring positioning is solved. Without it, RandCast may be simpler and sufficient. |
| Censorship resistance | No single node or small coalition can suppress a specific message | Redundant dissemination paths + Byzantine-aware protocol | Tier 2 BFT | Deliverable D1 asks to prove this. Requires formal threat model. |
| Store-and-forward persistence | Subscribers who were offline can catch up on missed messages | Lightweight buffer on peers or dedicated storage nodes | Tier 1 dissemination | D2 Ch.4 designs a full DHT; base use case may only need hours of buffering |
| Cross-layer descriptor integrity | Vicinity gossip exchanges verify descriptor authenticity | Signed descriptors verified at Vicinity and dissemination layers | Tier 1 peer discovery (SecureCyclon) | Closes the gap Ezequiel identified. SecureCyclon protects peer sampling integrity (hub/eclipse resistance); this block adds descriptor verification at Vicinity and dissemination to prevent forged descriptors at upper layers. Both are needed — SecureCyclon is necessary but not sufficient, and upper-layer signing alone cannot compensate for a compromised sampling layer. |
| Anti-spam / rate limiting | Prevent topic flooding | Per-topic rate limits, stake-weighted quotas | Tier 1 topic management | FR5.1 requirement, completely undesigned |
| Navigation efficiency | O(log T) routing to any topic from any entry point | Vicinity finger links across topics | Tier 1 peer sampling | D2 Ch.3.2. Useful at scale (10k+ topics), overkill for initial deployment |

### Tier 3 — Beyond Scope (separate protocols or future work)

| Property | Why beyond scope | Alternative approach |
|----------|-----------------|---------------------|
| Competitive intent distribution | Rational suppression breaks gossip (Ezequiel §2.1) | Direct-to-solver via on-chain registry |
| High-throughput agent coordination | Exceeds gossip capacity, no incentive to share (Ezequiel §2.5) | Direct WebSocket, centralized relays |
| End-to-end encryption / private topics | Orthogonal to dissemination, application-layer concern | MLS (RFC 9420) on top of base protocol |
| Many-to-many group messaging | Different communication pattern than pub/sub broadcast | Dedicated messaging protocol |
| On-chain vote routing | Adds trust assumptions over current on-chain voting (Ezequiel §2.2) | Keep votes on-chain (CIP-1694) |
| Automated validator response | Unacceptable security risk (Ezequiel §2.3) | Information delivery only, local operator policy |
| Full DHT persistence (weeks, high replication) | Over-engineered for notification use case (Ezequiel §5.4) | Lightweight store-and-forward buffer |

---

## Mapping to Deliverables

| Deliverable | Tier 1 blocks needed | Tier 2 blocks (stretch) |
|-------------|---------------------|------------------------|
| **D1: Byzantine resilient prototype + cost analysis** | Dissemination (RandCast), peer discovery (SecureCyclon + Vicinity), crash FT, basic Sybil resistance | BFT (upper-layer descriptor signing, VRF positioning), censorship resistance proof |
| **D2: Identity + on-chain topic management** | Topic registry (Quint spec → Plutus/Aiken), publisher authentication (static-key signing) | DID-based identity upgrade, cross-layer descriptor integrity |
| **D3: Fee & incentive analysis** | Basic Sybil resistance (stake requirement) | Anti-spam/rate limiting, store-and-forward incentives |

---

## SRL Status & Gaps

**Current assessment: SRL 2 — substantially complete, targeting SRL 3.**

### SRL 2 evidence (concept formulated, basic principles coded, synthetic experiments)

| Exit criterion | Status | Evidence |
|---|---|---|
| Concept formulated | Done | Authenticated notification primitive (Ezequiel's review, this doc) |
| Basic principles coded | Done | Quint spec — topic registry with 15 invariants + temporal liveness (TLC verified). PRISM DTMC — RingCast N=6 with parameterised fanout and failure (coverage, latency, overhead). |
| Experiments with synthetic data | Done | TLC model checking over synthetic state spaces. PRISM probabilistic model checking. Denis's analytical partitioning result (2 adversaries eclipse any subscriber with P ≈ e^{-RF}, independent of N). |
| M&S refining performance predictions | Done | PRISM model quantifies coverage probability, expected rounds, message overhead under crash failures |
| Application identified + feasibility | Done | SPO notification use case, building blocks tier analysis |
| R&D approach formulated | Done | Epic breakdown with tier/deliverable mapping |
| Documented description of feasibility & benefit | Partial | This doc + Ezequiel's review + incentive model cover the substance; needs consolidation into a single exit report |
| Published results | Partial | AUEB D1/D2 published; Denis's partitioning result and PRISM results not yet written up as a report |

### SRL 2 → 3 gaps

To exit SRL 2 and enter SRL 3, the remaining work is documentation
consolidation rather than missing experiments: write up the existing
Quint, PRISM, and partitioning analysis results as a cohesive report
addressing feasibility, benefit, and preliminary performance predictions.

### SRL 3 targets (analytical/experimental proof-of-concept)

| Exit criterion | Mapping |
|---|---|
| Critical functions/components identified | Tier 1 building blocks in this doc |
| Component coding completed | Topic registry contract (Aiken), publisher auth, dissemination prototype (SecureCyclon + Vicinity + RandCast) |
| Component verification completed | Quint spec verification (on-chain); extend PRISM or build simulator (dissemination) |
| Analytical proof-of-concept documented | Threat model & security analysis resolving signing strategy, ID assignment, RandCast vs RingCast decision |
| Laboratory test environment established | Prototype testbed for gossip protocol |
| Key performance metrics established | Coverage, latency, message overhead, eclipse resistance bounds |

### SRL 3 characteristic deliverables

- Full requirements specification (on-chain: Quint spec exists; dissemination: needed)
- Outline design (this doc + ADRs)
- Initial proofs of concept (Epics #1–3)
- Published applications paper (consolidating formal + analytical results)

---

## Open Questions

1. Does the base use case (SPO notifications) actually need Byzantine
   fault tolerance, or is crash FT + stake-based Sybil resistance
   sufficient in practice?
2. Is RandCast sufficient for the prototype, with RingCast as an
   optional upgrade if the threat model justifies it?
3. What is the minimum viable persistence model? Full DHT or just
   peer-side buffering?
4. Should the formal verification effort focus on the dissemination
   protocol (extending the PRISM work) or the smart contract (extending
   the Quint work), or both?
5. **Descriptor signing strategy** (Ezequiel's question): SecureCyclon
   and upper-layer descriptor signing are complementary, not
   alternatives. SecureCyclon prevents hub formation and eclipse attacks
   at the peer sampling layer — without it, an adversary can poison a
   node's view before Vicinity ever sees it. Upper-layer signing at
   Vicinity and dissemination prevents forged descriptors being accepted
   at those layers — without it, the gap Ezequiel identified remains
   open. Both are needed. Epic #5 should formalise the verification
   requirements at each layer boundary.

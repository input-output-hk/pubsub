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
| Publisher authentication | Messages are signed, subscribers verify sender identity | DID-based signing + on-chain publisher lists | Design exists, needs implementation | Deliverable D2 |
| Message dissemination | Published messages reach all online subscribers | Gossip protocol (random links at minimum) | AUEB design exists, needs prototype | Core of D1 deliverable |
| Peer discovery | New nodes find same-topic peers | Peer sampling + topic-aware filtering | SecureCyclon + Vicinity designed | D2 Ch.3.3 |
| Crash fault tolerance | Network stays connected when nodes fail/leave | Redundant links (random or structured) | Harary graph designed, RandCast alternative under analysis | PRISM model partially covers this |
| Basic Sybil resistance | Joining the network has a cost | On-chain identity (staking credential requirement) | Natural fit with SPO model, not formally designed | Implicit in the SPO participant model |

### Tier 2 — Nice to Have (strengthens base, adds resilience)

| Property | What it means | Building block | Dependency | Notes |
|----------|--------------|----------------|------------|-------|
| Byzantine fault tolerance | Network delivers messages even with actively malicious nodes | Descriptor signing at all layers + unpredictable ring position (VRF) | Tier 1 dissemination | Current design has gaps (Ezequiel's Vicinity concern, positional Sybil). Deliverable D1 scope asks for this. |
| Deterministic delivery guarantee | Mathematical guarantee that messages reach all honest nodes under bounded failures | Harary graph (RingCast) | Tier 1 peer discovery + ring convergence protocol | Only valuable if BFT ring positioning is solved. Without it, RandCast may be simpler and sufficient. |
| Censorship resistance | No single node or small coalition can suppress a specific message | Redundant dissemination paths + Byzantine-aware protocol | Tier 2 BFT | Deliverable D1 asks to prove this. Requires formal threat model. |
| Store-and-forward persistence | Subscribers who were offline can catch up on missed messages | Lightweight buffer on peers or dedicated storage nodes | Tier 1 dissemination | D2 Ch.4 designs a full DHT; base use case may only need hours of buffering |
| Cross-layer descriptor integrity | Vicinity gossip exchanges verify descriptor authenticity | Signed descriptors verified at every layer, not just SecureCyclon | Tier 1 peer discovery | Closes the gap Ezequiel identified. Relatively straightforward to add. |
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

## Architecture Decision: RandCast vs RingCast

This is the key design choice that determines complexity:

| | RandCast (random links only) | RingCast (Harary + random links) |
|---|---|---|
| **Delivery guarantee** | Probabilistic (high but not 100%) | Deterministic under t-1 crash failures |
| **Join/leave complexity** | Trivial — get random peers, done | Convergence period to find ring position |
| **Positional Sybil risk** | N/A — no ring to game | Requires unpredictable position assignment |
| **New joiner experience** | Immediately fully integrated | Degraded until ring converges |
| **Implementation complexity** | Low | Medium-high (ring maintenance, convergence) |
| **BFT story** | Simpler — fewer attack surfaces | Stronger guarantee but more gaps to close |
| **When to choose** | SPO-scale, low churn, notification use case | High-failure environments, strict delivery SLAs |

**Recommendation for discussion:** Start with RandCast for the prototype
(Tier 1), prove dissemination properties formally, then evaluate whether
adding the Harary structure (Tier 2) is justified by the threat model.

---

## Mapping to Deliverables

| Deliverable | Tier 1 blocks needed | Tier 2 blocks (stretch) |
|-------------|---------------------|------------------------|
| **D1: Byzantine resilient prototype + cost analysis** | Dissemination (RandCast), peer discovery, crash FT, basic Sybil resistance | BFT (descriptor signing, VRF positioning), censorship resistance proof |
| **D2: DID + on-chain topic management** | Topic registry (Quint spec → Plutus/Aiken), publisher authentication (DID signing) | Cross-layer descriptor integrity |
| **D3: Fee & incentive analysis** | Basic Sybil resistance (stake requirement) | Anti-spam/rate limiting, store-and-forward incentives |

---

## Suggested Epic Breakdown

1. **Topic Registry Contract** — Implement Plutus/Aiken contract from
   Quint spec. Includes on-chain tests. (Tier 1, Deliverable D2)

2. **DID Integration** — Publisher signing and subscriber verification
   using DIDs. (Tier 1, Deliverable D2)

3. **Dissemination Prototype (RandCast)** — Random-link gossip within a
   topic. Peer sampling via Cyclon. No ring structure initially.
   (Tier 1, Deliverable D1)

4. **Formal Dissemination Analysis** — Extend PRISM model or build
   simulator. Compare RandCast vs RingCast. Quantify coverage, latency,
   message overhead under crash and Byzantine failure models.
   (Tier 1–2, Deliverable D1)

5. **Threat Model & Security Analysis** — Define adversary capabilities,
   analyse descriptor integrity across layers, node ID assignment,
   positional Sybil resistance. Decide if Harary structure is needed.
   (Tier 2, Deliverable D1)

6. **Fee & Incentive Model** — Cost analysis for SPO participation,
   stake-weighted rate limiting design, storage incentives if persistence
   is added. (Tier 1–2, Deliverable D3)

7. **Byzantine Hardening** (if justified by #5) — Add descriptor signing
   at Vicinity layers, VRF-based ring positioning, upgrade to RingCast
   if warranted. (Tier 2, Deliverable D1)

8. **Store-and-Forward** (if needed) — Lightweight catch-up buffer for
   offline subscribers. Scoped version of D2 Ch.4 DHT.
   (Tier 2, Deliverable D1/D3)

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
5. Is SecureCyclon needed over plain Cyclon if we add descriptor
   signing at all layers? (Ezequiel's question)

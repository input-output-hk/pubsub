# Research Foundation

!!! info "Audience: Engineers, Architects, Researchers"

This page documents the foundational research that informs the Cardano PubSub architecture, specifically the **"Cardano Pub/Sub Framework: Design and Architecture"** paper produced by IOG Research in collaboration with Athens University of Economics and Business.

## Paper Overview

| Property | Value |
|----------|-------|
| **Title** | Cardano Pub/Sub Framework: Design and Architecture |
| **Authors** | Alexandros Antonov, Evangelos Kolyvas, Spyros Voulgaris |
| **Institution** | Department of Informatics, Athens University of Economics and Business |
| **Deliverable** | D2 — September 2024 |
| **Commissioned by** | IOG (Input Output Global) |

---

## Core Design Principles

The research establishes several key principles for a Cardano-native Pub/Sub system:

1. **Decentralized Architecture** — No central coordinators; nodes self-organize using gossip protocols
2. **On-chain Administration** — Topic registry managed via smart contract for transparency and global knowledge
3. **SPO-Powered Persistence** — Stake Pool Operators provide storage infrastructure with economic incentives
4. **Efficiency + Reliability** — Hybrid approach combining fast probabilistic dissemination with guaranteed delivery

---

## Three-Layer Dissemination Protocol

The paper proposes a three-layer protocol stack for message dissemination, with each layer serving a distinct purpose:

```
┌─────────────────────────────────────────────────────────────────┐
│              Layer 3: Dissemination Layer                       │
│         Hybrid Dissemination (Harary Graph + Random Links)      │
│         → Fast, reliable message delivery within a topic        │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│              Layer 2: Navigation Layer                          │
│                    Vicinity Protocol                            │
│         → Efficient routing to target topic's subscribers       │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│              Layer 1: Peer Sampling Layer                       │
│                    SecureCyclon Protocol                        │
│         → Maintains connected overlay, provides random samples  │
└─────────────────────────────────────────────────────────────────┘
```

### Layer 1: Peer Sampling (SecureCyclon)

**Purpose:** Maintain a connected overlay across all subscribers regardless of topic, and provide continuous random peer samples.

| Property | Description |
|----------|-------------|
| **Protocol** | SecureCyclon (secure variant of Cyclon) |
| **Origin** | Developed at AUEB for IOG's "Eclipse-Resistant Network Overlays" project (2020-2022) |
| **View Size** | Small, fixed-size per node |
| **Gossip** | Periodic exchanges with random neighbors |

**Key Properties:**

- **Extreme robustness** to node failures — overlay remains connected even with high concurrent failures
- **Self-healing** — adapts to failures, maintaining random graph structure
- **Randomness** — neighbors uniformly randomly selected, continuously refreshed
- **Eclipse resistance** — prevents adversaries from isolating nodes

### Layer 2: Navigation (Vicinity)

**Purpose:** Enable nodes to efficiently discover and connect to same-topic subscribers.

| Property | Description |
|----------|-------------|
| **Protocol** | Vicinity (gossip-based overlay building) |
| **Proximity Metric** | Circular topic ordering based on on-chain Topic Registry |
| **Finger Links** | Links at distances b⁰, b¹, b² ... (typically b=2) in both directions |
| **Routing Hops** | O(log_b T) where T = total topics |

**How it works:**

1. Topics are assigned unique IDs from the on-chain registry
2. Topics are modeled as vertices in a ring (0 to T-1)
3. Each node maintains links to topics at exponentially increasing distances
4. When joining, a node can reach its target topic in logarithmic hops

### Layer 3: Dissemination (Hybrid)

**Purpose:** Fast, reliable event dissemination within a topic.

| Property | Description |
|----------|-------------|
| **Protocol** | Hybrid Dissemination |
| **Structure** | Harary Graph + Random Links |
| **Reliability** | Harary graph guarantees connectivity under failures |
| **Efficiency** | Random links provide exponential-speed propagation |

**Harary Graph (H_{t,n}):**

- Minimal-link graph guaranteeing connectivity when up to t-1 nodes/links fail
- Nodes arranged in cyclic order with links to t/2 closest neighbors in each direction
- Provides **guaranteed delivery** even under failures

**Random Links:**

- Each node maintains additional random links to same-topic peers
- Enables **fast propagation** — message reaches f^0, f^1, f^2... nodes exponentially
- Fanout (f) as low as 2 is sufficient for fast dissemination

**Combined Effect:**

- Random links spread messages quickly to most nodes
- Harary links ensure **100% delivery** to remaining nodes
- Logarithmic dissemination time with high resilience

---

## On-Chain Topic Registry

Topics are administered via a smart contract deployed on Cardano, providing global knowledge of all topics and their configurations.

### Topic Properties

| Property | Description |
|----------|-------------|
| **TopicId** | 256-bit unique ID assigned by registry (permanent, never reused) |
| **Name/Description** | Human-readable topic identifier |
| **Owners** | List of public keys with full administrative rights |
| **Publishers** | List of authorized publishers (empty = open topic) |
| **Retention Period** | How long events should be stored (in epochs) |
| **Replication Factor** | Number of nodes storing each event |

### Topic Types

| Type | Publishers List | Behavior |
|------|-----------------|----------|
| **Open Topic** | Empty | Anyone can publish |
| **Moderated Topic** | One or more keys | Only listed publishers can publish |

### Administrative API

| Method | Eligible | Description |
|--------|----------|-------------|
| `createTopic(...)` | Anyone | Initialize new topic, returns topicId |
| `deleteTopic(topicId)` | Owner | Remove topic from registry |
| `addOwner/removeOwner` | Owner | Manage ownership |
| `addAdmin/removeAdmin` | Owner | Delegate administrative rights |
| `addPublisher/removePublisher` | Owner, Admin | Control who can publish |
| `setReplicationFactor(topicId, R)` | Owner, Admin | Configure storage redundancy |
| `setRetentionPeriod(topicId, T)` | Owner, Admin | Configure event lifetime |

---

## Event Persistence Layer

The persistence layer ensures events remain available for retrieval after initial dissemination, critical for subscribers who were offline.

### Architecture: One-Hop DHT

The paper proposes a **clique-structured DHT** optimized for the expected scale of replication servers (hundreds to thousands, comparable to SPO relays).

| Property | Description |
|----------|-------------|
| **Structure** | Clique (every node knows all others) |
| **Routing** | One-hop (direct contact to responsible node) |
| **Similarity** | Amazon Dynamo / DynamoDB |
| **Membership** | On-chain registration of replication servers |

**Advantages:**

- Simplified overlay management (no complex routing infrastructure)
- Single-hop lookups (minimal latency)
- No routing overhead on storage nodes
- Fast fault detection and recovery

### Replication Servers

| Property | Description |
|----------|-------------|
| **Operators** | Primarily SPOs (established stake in ecosystem) |
| **Registration** | On-chain with IP address and public key |
| **Commitment** | Defined period (epochs) with security deposit |
| **Eligibility** | Minimum ADA threshold |
| **Join/Leave** | Only at epoch boundaries |

### Incentivization

| Mechanism | Description |
|-----------|-------------|
| **Rewards** | Periodic payments funded by publishers |
| **Penalties** | Security deposit slashed for failures |
| **Verification** | Proof of Replication / Proof of Retrieval challenges |

### Key Indexing Scheme

Events are indexed using a deterministic scheme that enables decentralized retrieval:

```
key = hash(topicId . publisherId . sequenceNr)
```

| Component | Description |
|-----------|-------------|
| **topicId** | Topic's unique identifier |
| **publisherId** | Publisher's public key |
| **sequenceNr** | Per-topic, per-publisher counter (starts at 0) |

**Benefits:**

- Events spread evenly across replication servers
- Subscribers can construct keys locally (no central registry needed)
- Lightweight per-topic log tracks latest sequence numbers per publisher

### Recovery Flow

When a subscriber recovers from being offline:

1. Query `hash(topicId)` to get list of publishers and their latest sequence numbers since disconnect time
2. Construct keys for missed events: `hash(topicId.publisherId.seqNr)` for each gap
3. Retrieve events in parallel from replication servers
4. Load balanced across R replicas per event

---

## Alignment with Our Architecture

The research paper's design aligns well with our existing architecture and fills in implementation details:

| Our Layer | Research Paper Component | Alignment |
|-----------|--------------------------|-----------|
| **P2P Networking** | Three-layer protocol (SecureCyclon + Vicinity + Hybrid) | ✅ Harary Graph already planned; adds Peer Sampling and Navigation |
| **Storage & Persistence** | One-hop DHT with SPO replication servers | ✅ Aligns with SPO-operated vision; adds concrete DHT design |
| **Identity** | — | Our addition (Identus DIDs not in research scope) |
| **Topic Administration** | On-chain Topic Registry | ✅ Provides concrete smart contract design |

### Key Adoptions

1. **SecureCyclon** for peer sampling — proven eclipse-resistant, developed for IOG
2. **Vicinity** for topic navigation — efficient O(log T) routing to any topic
3. **Hybrid Dissemination** for reliability — Harary + random links
4. **Clique DHT** for persistence — simple, fast, SPO-operated
5. **Topic Registry** smart contract — on-chain administration

### Our Extensions

The research provides the communication foundation. We extend it with:

- **Identus DID integration** — Decentralized identity for publishers/subscribers
- **MLS encryption** — End-to-end encrypted private topics
- **DeFi Intents support** — User Intent message format and agent discovery
- **L1 State Oracle** — Token-gated access control

---

## References

- Antonov, A., Kolyvas, E., Voulgaris, S. (2024). *Cardano Pub/Sub Framework: Design and Architecture*. Athens University of Economics and Business.
- Voulgaris, S., van Steen, M. (2007). *Hybrid Dissemination: Adding Determinism to Probabilistic Multicasting in Large-Scale P2P Systems*. Middleware.
- Antonov, A., Voulgaris, S. (2023). *SecureCyclon: Dependable Peer Sampling*. IEEE ICDCS.
- Voulgaris, S., van Steen, M. (2013). *Vicinity: A Pinch of Randomness Brings Out the Structure*. Middleware.

---

## Related

- [System Layers](layers.md)
- [Philosophy](philosophy.md)
- [DeFi Intents Use Case](../use-cases/defi-intents.md)

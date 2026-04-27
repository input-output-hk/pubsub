# System Layers

The Cardano PubSub Node is composed of five distinct layers. Each layer is designed with specific use cases driving its architecture.

## Layer 1: P2P Networking Layer

**Function:** Handles peer discovery, connection management, and message propagation.

### Strategic Direction

The networking layer is based on the D2 research paper (AUEB/IOG, 2024) — a three-layer dissemination protocol designed specifically for Cardano. The stack prioritizes compatibility with the **Ouroboros Network** (Cardano's native stack), ensuring that SPOs can run Cardano PubSub side-by-side with their block producers using familiar connection managers and multiplexing protocols.

### Three-Layer Dissemination Protocol

| Sub-Layer | Protocol | Purpose |
|-----------|----------|---------|
| **Peer Sampling** | SecureCyclon | Maintains connected overlay, provides random peer samples, eclipse-resistant |
| **Navigation** | Vicinity | Efficient O(log T) routing to discover same-topic subscribers |
| **Dissemination** | Hybrid (Harary + Random) | Fast propagation with guaranteed delivery |

### Key Properties

| Property | Mechanism |
|----------|-----------|
| **Speed** | Random links enable exponential message spread |
| **Reliability** | Harary Graph guarantees delivery under node failures |
| **Eclipse Resistance** | SecureCyclon prevents adversary isolation attacks |
| **Topic Discovery** | Vicinity enables logarithmic-hop routing to any topic |

### Use Case Drivers

- **DEF-01 (DeFi Intents):** Requires low latency (<500ms) propagation
- **SPO Adoption:** Ensures the software feels "native" to Cardano operators

---

## Layer 2: Storage & Persistence Layer

**Function:** Manages the temporary or durable storage of messages based on topic configuration.

### Key Components

| Component | Purpose |
|-----------|---------|
| **Hot Cache (RAM)** | Ephemeral messages with short TTLs |
| **Durable DHT (Disk/RocksDB)** | Long-term availability and retrieval |
| **Decentralized Indexing** | `hash(topicId.publisherId.sequenceNr)` allows light clients to query missing data without a central server |

### Use Case Drivers

- **DEF-01:** Uses Hot Cache (10-minute TTL) to prevent storage bloat from filled orders
- **GOV-01 (Governance):** Uses Durable DHT (14-day retention) to ensure users can read proposals even if they come online days after posting

---

## Layer 3: Identity & Verification Layer

**Function:** The "Gatekeeper" layer responsible for Authentication, Authorization, and Reputation.

### Strategic Direction

This layer is **explicitly built on Identus**. We do not use generic crypto-keys; we use DIDs. Every publisher on Cardano PubSub is identified by a DID, allowing for rich, portable reputation and credential verification.

### Key Components

| Component | Purpose |
|-----------|---------|
| **Identus Resolver** | Resolves DIDs (e.g., `did:prism`, `did:cardano`) to verify signatures and fetch public keys |
| **Verifiable Credential (VC) Verifier** | Checks if a sender holds a specific VC (e.g., "KYC'd Agent" or "Committee Member") before relaying their message |
| **L1 State Oracle** | Queries UTXO set or Stake snapshots for asset-based access control |

### Use Case Drivers

- **SOC-01 (Social):** Uses Identus DIDs to link "User Handles" across sessions
- **GOV-01 (Governance):** Verifies that a "Proposal Alert" is signed by a DID holding the "Constitutional Committee" Credential

---

## Layer 4: Security & Encryption Layer

**Function:** Provides privacy and anti-spam protections.

### Key Components

| Component | Purpose |
|-----------|---------|
| **MLS (RFC 9420) Engine** | Manages Group Key exchange and rotation for private topics |
| **Sealed Sender** | Anonymizes the source of a message during routing |

### Use Case Drivers

- **SOC-01 (Social):** Primary driver for MLS — ensures only group members can decrypt messages (E2EE)
- **AI-01 (Agents):** Uses encryption for private negotiation sessions to prevent "front-running" of strategies

---

## Layer 5: API & SDK Layer

**Function:** The interface for Wallets and dApps.

### Key Components

| Component | Purpose |
|-----------|---------|
| **gRPC / GraphQL API** | High-performance node interaction |
| **Cardano PubSub SDK (TS/Rust)** | Client libraries handling connection, signing, and encryption complexity |

### Use Case Drivers

- **All Cases:** A unified API is essential for developer adoption
- **DEF-01:** Relies on standardized Intents schemas in the SDK to ensure interoperability between different Wallets and Solvers

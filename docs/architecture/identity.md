# Identity & DID Integration

**A modular, chain-agnostic identity architecture for Cardano PubSub.**

!!! info "Layer 3: Identity & Verification"
    This document details Layer 3 of the PubSub architecture — the "gatekeeper" layer responsible for authentication, authorization, and reputation.

## Strategic Imperatives

### The Problem: Ecosystem Lock-In

Tightly coupling identity to a single blockchain creates significant risks:

| Risk | Impact |
|------|--------|
| **User Friction** | Users from other ecosystems must acquire ADA just to authenticate |
| **Architectural Rigidity** | If a better identity standard emerges, migration requires massive refactoring |
| **Liquidity Fragmentation** | Cross-chain governance requires aggregating reputation across chains |

### The Solution: Chain-Agnostic Identity

Treat the underlying blockchain as a **Verifiable Data Registry (VDR)** — a pluggable storage layer, not the identity system itself. This allows:

- Identus (did:prism) as a **premier plugin**, not the only option
- Support for EVM wallets via did:pkh (no ADA required)
- Off-chain pairwise identities via did:peer

---

## The Resolver Mesh Pattern

Instead of a monolithic resolver, PubSub uses a **Resolver Mesh**: a configurable, plugin-based system.

```
┌─────────────────────────────────────────────────────────────────┐
│                     PubSub Identity Layer                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐    resolve(did)    ┌──────────────────────┐  │
│  │  Application │ ─────────────────► │     Dispatcher       │  │
│  │    Logic     │                    │  (parses method)     │  │
│  └──────────────┘                    └──────────┬───────────┘  │
│                                                  │              │
│                    ┌─────────────────────────────┼──────────┐   │
│                    │              Registry       │          │   │
│                    │                             ▼          │   │
│         ┌─────────────────┐  ┌─────────────────┐  ┌────────────┐
│         │ IdentusDriver   │  │   PkhDriver     │  │ PeerDriver │
│         │ (did:prism)     │  │ (did:pkh)       │  │ (did:peer) │
│         └────────┬────────┘  └────────┬────────┘  └─────┬──────┘
│                  │                    │                  │      │
│                  ▼                    ▼                  ▼      │
│         ┌──────────────┐    ┌──────────────┐    ┌────────────┐ │
│         │ PRISM Node   │    │  Algorithm   │    │ Local DB   │ │
│         │ (Cardano)    │    │ (any chain)  │    │ (RocksDB)  │ │
│         └──────────────┘    └──────────────┘    └────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

### Drivers

| Driver | DID Method | Resolution Strategy |
|--------|------------|---------------------|
| **IdentusDriver** | `did:prism` | Queries PRISM Node via HTTP/gRPC; requires Cardano indexing |
| **PkhDriver** | `did:pkh` | Algorithmically constructs DID Document from blockchain address (CAIP-10) |
| **PeerDriver** | `did:peer` | Resolves from local storage; for off-chain pairwise relationships |
| **KeyDriver** | `did:key` | Self-contained; no external resolution needed |

### The DID Document as "Rosetta Stone"

Regardless of source, all DIDs resolve to a **DID Document** containing:

- **Verification Methods**: Public keys in standardized formats (JsonWebKey2020, Ed25519VerificationKey2018)
- **Service Endpoints**: URLs for communication
- **Authentication**: Which keys can authenticate

PubSub's cryptographic layer verifies signatures against DID Documents, ignoring where they came from.

---

## Supported DID Methods

### did:prism (Identus/Cardano)

The native Cardano identity method via Hyperledger Identus.

| Aspect | Details |
|--------|---------|
| **Anchoring** | DID operations batched into Merkle tree, root posted to Cardano |
| **Resolution** | Requires PRISM Node or NeoPRISM indexer |
| **Credentials** | Supports AnonCreds (zero-knowledge proofs) |
| **Best For** | Cardano-native users, privacy-preserving credentials |

### did:pkh (Cross-Chain)

Turns any blockchain address into a valid DID — **zero friction, no registration**.

| Aspect | Details |
|--------|---------|
| **Format** | `did:pkh:{chain}:{address}` (e.g., `did:pkh:eip155:1:0x123...`) |
| **Resolution** | Algorithmic — no on-chain lookup needed |
| **Best For** | Onboarding EVM users, cross-chain interoperability |

**Example:** MetaMask wallet connects → detected as `did:pkh:eip155:1:0x123...` → user can authenticate immediately without ADA.

### did:peer (Off-Chain)

For private, pairwise relationships that don't need blockchain anchoring.

| Aspect | Details |
|--------|---------|
| **Storage** | Local database (RocksDB) |
| **Resolution** | Exchanged during peering handshake |
| **Best For** | DMs, private channels, ephemeral identities |

---

## Chain-Agnostic Addressing (CAIP-10)

PubSub never stores raw addresses. All identifiers use **CAIP-10** format:

```
{namespace}:{chainId}:{accountAddress}
```

| Chain | Example |
|-------|---------|
| Cardano | `cip34:1-764824073:addr1qxy...` |
| Ethereum | `eip155:1:0x123...` |
| Bitcoin | `bip122:000000000019d6689c085ae165831e93:bc1q...` |

This ensures the database schema is ready for any chain.

---

## MLS Integration: PubSub as Delivery Service

Messaging Layer Security (MLS / RFC 9420) requires a **Delivery Service (DS)** for message ordering and broadcast. PubSub fulfills this role natively.

### Mapping MLS to PubSub Layers

| MLS Concept | PubSub Implementation |
|-------------|----------------------|
| **Broadcast Channel** | Layer 1 (Hybrid Gossip) — MLS messages wrapped in PubSub packets |
| **Ordering** | Layer 1 (Harary Graph) — ensures consistent message sequence |
| **Welcome Messages** | Topics — e.g., `pubsub/group/{groupID}/welcome` |
| **Offline History** | Layer 2 (Durable DHT) — persists epoch history for sync |

### Distributed Authentication Service

MLS requires an **Authentication Service (AS)** to bind credentials to identities. PubSub implements this using the Resolver Mesh:

1. **KeyPackage**: User publishes MLS KeyPackage signed by their DID
2. **Verification**: Recipients resolve the DID via Resolver Mesh, verify signature against DID Document
3. **Result**: Distributed, chain-agnostic authentication — no central AS

### High-Assurance Mode: Smart Contract AS

For governance channels requiring stronger guarantees:

- Smart contract maintains registry of allowed DIDs
- Clients query contract (via Ouroboros or eth_call) before MLS handshake
- Provides on-chain membership verification

---

## Wallet Adapters

Users connect via various wallets. The **IIdentityWallet** abstraction provides a unified interface:

```typescript
interface IIdentityWallet {
  getIdentifiers(): DID[];
  sign(did: DID, payload: Uint8Array): Signature;
}
```

### Supported Adapters

| Adapter | Wallets | DID Method |
|---------|---------|------------|
| **Cardano** | Lace, Nami, Eternl (CIP-30) | did:prism or did:pkh |
| **Identus** | Identus Edge Agent | did:prism |
| **EVM** | MetaMask, WalletConnect | did:pkh |

---

## Verifiable Credentials

DIDs provide authentication; **Verifiable Credentials (VCs)** provide attributes.

| Format | Ecosystem | PubSub Support |
|--------|-----------|----------------|
| **JWT-VC** | EVM/Web3 | ✅ Verify JWT signatures |
| **AnonCreds** | Identus/Hyperledger | ✅ Verify ZK proofs |

### Cross-Chain Reputation Example

1. User proves NFT ownership on Ethereum (signs VC with did:pkh)
2. PubSub verifies signature against Ethereum state
3. User gains access to Cardano-native channel based on verified credential

---

## Governance Integration (CIP-1694)

PubSub integrates with Cardano's Voltaire governance:

| Role | Identity |
|------|----------|
| **DReps** | Identified by DID credentials |
| **Constitutional Committee** | DID-signed proposals |
| **Voters** | Any supported DID method |

**Anchor Metadata**: PubSub can host CIP-100 compliant governance metadata (JSON-LD), stored via IPFS or PubSub's DHT.

---

## Prototyping Strategy

### Phase 1: Local-First (Validates Resolver + MLS)

- **Identity**: did:key only (no blockchain)
- **Transport**: In-memory message bus
- **Success**: Two clients exchange encrypted MLS messages

### Phase 2: Identus Integration

- **Identity**: Add did:prism driver
- **Infrastructure**: Local Identus stack (Cardano node + PRISM node)
- **Success**: Create DID, anchor on Cardano, resolve, sign MLS KeyPackage

### Phase 3: Full Network

- **Identity**: Add did:pkh driver
- **Transport**: PubSub network (distributed)
- **Success**: did:prism user and did:pkh user exchange encrypted messages over PubSub

---

## Technology Stack

| Component | Recommendation | Rationale |
|-----------|----------------|-----------|
| **DID Framework** | Veramo | Modular, plugin-based, TypeScript |
| **Cardano VDR** | Identus SDK | Official did:prism support |
| **EVM Support** | viem + did-provider-pkh | Lightweight, standard DID |
| **MLS** | OpenMLS (Rust/Wasm) | Robust RFC 9420 implementation |
| **Local Storage** | RxDB / PouchDB | Offline-first for DIDs and history |

---

## Open Questions

| Question | Status | Notes |
|----------|--------|-------|
| PRISM Node hosting (who runs it?) | ⬜ Not decided | Could be SPOs, dedicated service, or Identus team |
| did:pkh support priority (which chains first?) | ⬜ Not decided | Likely EVM first, then Bitcoin |
| AnonCreds vs JWT-VC preference | ⬜ Not decided | May support both equally |
| Smart Contract AS specification | ⬜ Not started | Needed for high-assurance governance |

## Related

- [System Layers](layers.md) — Layer 3 overview
- [Build vs Buy](build-vs-buy.md) — Identity adoption decision
- [Glossary: DID, Identus, MLS](../glossary.md)

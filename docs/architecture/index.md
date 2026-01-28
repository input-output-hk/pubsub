# Architecture Overview

!!! info "Audience: Engineers, Architects"

Cardano PubSub is designed as a **Decentralized Message Bus (DMB)** that fills the "Communication Gap" in the Cardano ecosystem. Unlike a blockchain ledger, which is optimized for global consensus and immutability, PubSub is optimized for **ephemeral, high-throughput, and privacy-preserving message propagation**.

## Strategic Directives

This architecture adheres to two key strategic directives:

1. **Native Ecosystem Compatibility** — The networking layer integrates natively with the Cardano Node (Ouroboros) stack, not bolted on as an afterthought.

2. **Modular DID Identity** — Decentralized Identity (DID) is a first-class citizen, with a pluggable resolver supporting multiple methods (Identus, did:pkh, did:peer) for authentication and reputation.

## Documentation Structure

| Section | Description |
|---------|-------------|
| [Research Foundation](research-foundation.md) | IOG Research paper analysis — foundational protocol design |
| [Philosophy](philosophy.md) | Core principles and design rationale |
| [System Layers](layers.md) | The five layers of the PubSub Node |
| [Identity & DIDs](identity.md) | Chain-agnostic DID integration strategy |
| [Development Strategy](development-strategy.md) | Phased kernel-out approach |
| [Build vs Buy](build-vs-buy.md) | Technology adoption decisions |

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                Layer 5: API & SDK Layer                         │
│         gRPC/GraphQL API  •  PubSub SDK (TS/Rust)              │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│            Layer 4: Security & Encryption Layer                 │
│              MLS (RFC 9420)  •  Sealed Sender                  │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│          Layer 3: Identity & Verification Layer                 │
│     DID Resolver Mesh  •  VC Verifier  •  L1 State Oracle      │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│            Layer 2: Storage & Persistence Layer                 │
│         Hot Cache (RAM)  •  Durable DHT (RocksDB)              │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│              Layer 1: P2P Networking Layer                      │
│   SecureCyclon  •  Vicinity  •  Hybrid Dissemination           │
└─────────────────────────────────────────────────────────────────┘
```

## Key Architectural Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| **Networking** | [IOG Research Protocol](research-foundation.md) | Cardano-native, SPO-compatible, proven algorithms |
| **Identity** | Modular DID (Identus, did:pkh, did:peer) | Chain-agnostic, no lock-in, portable reputation |
| **Encryption** | MLS (RFC 9420) | IETF standard, efficient group crypto |
| **Storage** | Tiered (RAM + DHT) | Different TTLs for different use cases |
| **Verification** | Custom L1 oracle | Unique to Cardano ecosystem |

## Use Cases Drive Architecture

The architecture is shaped by five core use cases (see [Use Cases](../use-cases/index.md)):

| Use Case | Architectural Stress Test | Key Decision |
|----------|---------------------------|--------------|
| [DeFi Intents](../use-cases/defi-intents.md) | Latency <500ms | Hybrid Dissemination + Hot Cache |
| [Governance](../use-cases/governance.md) | 100% delivery guarantee | Harary Graph + Durable DHT |
| [Agents](../use-cases/autonomous-agents.md) | 10k+ msg/sec throughput | Burst handling + CBOR |
| [Cross-Chain](../use-cases/cross-chain.md) | Foreign chain proofs | Verifier plugins |
| [Social](../use-cases/token-gated-social.md) | E2EE + token gating | MLS + L1 State Oracle |

## Related Documents

- [Product Requirements](../product/index.md)
- [Use Cases](../use-cases/index.md)
- [Glossary](../glossary.md)

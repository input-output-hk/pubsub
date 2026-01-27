# Architecture Overview

!!! info "Audience: Engineers, Architects"

Cardano PubSub is designed as a **Decentralized Message Bus (DMB)** that fills the "Communication Gap" in the Cardano ecosystem. Unlike a blockchain ledger, which is optimized for global consensus and immutability, PubSub is optimized for **ephemeral, high-throughput, and privacy-preserving message propagation**.

## Strategic Directives

This architecture adheres to two key strategic directives:

1. **Native Ecosystem Compatibility** — The networking layer integrates natively with the Cardano Node (Ouroboros) stack, not bolted on as an afterthought.

2. **Identus-Powered Identity** — Decentralized Identity (DID) is a first-class citizen, utilizing Identus for all actor authentication and reputation.

## Documentation Structure

| Section | Description |
|---------|-------------|
| [Philosophy](philosophy.md) | Core principles and design rationale |
| [System Layers](layers.md) | The five layers of the PubSub Node |
| [Architectural Drivers](drivers/index.md) | The 5 use cases that shape the architecture |
| [Use Case Coverage](use-case-coverage.md) | How architecture satisfies requirements |
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
│     Identus Resolver  •  VC Verifier  •  L1 State Oracle       │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│            Layer 2: Storage & Persistence Layer                 │
│         Hot Cache (RAM)  •  Durable DHT (RocksDB)              │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│              Layer 1: P2P Networking Layer                      │
│   Ouroboros Miniprotocols  •  GossipSub  •  Harary Graph       │
└─────────────────────────────────────────────────────────────────┘
```

## Key Architectural Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| **Networking** | Hybrid (Ouroboros + libp2p) | SPO familiarity + proven gossip |
| **Identity** | Identus DIDs | Ecosystem alignment, portable reputation |
| **Encryption** | MLS (RFC 9420) | IETF standard, efficient group crypto |
| **Storage** | Tiered (RAM + RocksDB) | Different TTLs for different use cases |
| **Verification** | Custom L1 oracle | Unique to Cardano ecosystem |

## Architectural Drivers

The architecture is shaped by five extreme use cases (see [Drivers](drivers/index.md)):

| Driver | Pushes | Architectural Response |
|--------|--------|------------------------|
| **DEF-01: DeFi Intents** | Latency | GossipSub + Hot Cache |
| **GOV-01: Governance** | Reliability | Harary Graph + Durable DHT |
| **AI-01: Agents** | Throughput | Burst handling + CBOR |
| **XCB-01: Cross-Chain** | Interop | Verifier plugins |
| **SOC-01: Social** | Privacy | MLS + L1 State Oracle |

## Beacon vs. PubSub Network

| Aspect | Beacon (Phase 1) | PubSub Network (Phase 2+) |
|--------|------------------|---------------------------|
| **Topology** | Centralized service | P2P SPO network |
| **Storage** | PostgreSQL | Distributed DHT |
| **Networking** | REST/WebSocket | Ouroboros + GossipSub |
| **Identity** | Identus DIDs | Identus DIDs (same) |
| **API** | REST/WebSocket | gRPC/GraphQL + REST gateway |

The key insight: **APIs and identity are consistent across both phases**, enabling seamless migration.

## Related Documents

- [Product Requirements](../product/index.md)

- [Glossary](../glossary.md)
- AUEB Paper: "Topic-Based Pub/Sub" *(internal)*

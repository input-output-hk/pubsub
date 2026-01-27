# Architecture Overview

Cardano Agora is designed as a **Decentralized Message Bus (DMB)** that fills the "Communication Gap" in the Cardano ecosystem. Unlike a blockchain ledger, which is optimized for global consensus and immutability, Agora is optimized for ephemeral, high-throughput, and privacy-preserving message propagation.

## Strategic Directives

This architecture adheres to two key strategic directives:

1. **Native Ecosystem Compatibility** — The networking layer is not limited to generic standards but is designed to integrate natively with the Cardano Node (Ouroboros) stack.

2. **Identus-Powered Identity** — Decentralized Identity (DID) is a first-class citizen, utilizing Identus (formerly Atala PRISM) for all actor authentication and reputation.

## Documentation Structure

- [Architectural Philosophy](philosophy.md) — Core principles and design rationale
- [System Layers](layers.md) — The five layers of the Agora Node
- [Use Case Coverage](use-case-coverage.md) — How architecture satisfies key use cases
- [Development Strategy](development-strategy.md) — Phased kernel-out approach
- [Build vs Buy](build-vs-buy.md) — Technology adoption decisions

## High-Level View

```
┌─────────────────────────────────────────────────────────────────┐
│                Layer 5: API & SDK Layer                         │
│         gRPC/GraphQL API  •  Agora SDK (TS/Rust)               │
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

## Related Documents

- [Product Requirements](../product/index.md)
- [Use Cases](../product/use-cases/index.md)
- AUEB Paper: "Topic-Based Pub/Sub"

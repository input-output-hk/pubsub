# Product Overview

!!! info "Audience: Executives, Product Managers, Stakeholders"

## Executive Summary

**Cardano PubSub** is the native communication layer for the Cardano ecosystem. It lets applications talk directly to your wallet — so you can vote on governance proposals, respond to DeFi alerts, and interact with dApps without ever leaving your interface.

The project follows a phased rollout:

- **Phase 1: Beacon** — A centralized service for Midnight mainnet launch (Q3 2025)
- **Phase 2: PubSub Network** — Fully decentralized, SPO-operated messaging
- **Phase 3: Full Economy** — On-chain incentives and DAO governance

## The Elevator Pitch

> **For Cardano users and developers**, Cardano PubSub is a native messaging protocol that delivers **actionable notifications directly to your wallet**. 
>
> Unlike generic Web3 messaging (XMTP, Push Protocol) or centralized platforms (Discord, Telegram), PubSub is **built into Cardano's infrastructure** — using the SPO network for delivery and Identus DIDs for identity.
>
> This means: **One-click governance voting. Instant DeFi alerts. Token-gated communities. All without leaving your wallet.**

## Strategic Context: Why Now?

### The Midnight Forcing Function

Midnight mainnet launches in **Q3 2025**. The Midnight Foundation needs a way to communicate with users (wallet notifications, governance alerts, node updates). Without PubSub, they'll be forced to use:

- ❌ Discord/Telegram (centralized, phishing-prone)
- ❌ Email (requires PII collection)
- ❌ Custom solution (fragments the ecosystem)

**Beacon is the answer** — a production-ready notification service that Midnight can use at launch, with a clear path to decentralization.

### The Competitive Window

| Competitor | Status | Our Advantage |
|------------|--------|---------------|
| XMTP | Growing on EVM | No Cardano support; EVM-centric identity |
| Push Protocol | Building own L1 | Complexity; we leverage existing SPO network |
| Waku | Mature but generic | Not compatible with Cardano's network stack |

**If we don't build this, someone else will** — and it won't be native to Cardano.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                     Application Plane                           │
│        Wallets & dApps using PubSub SDK (TS/Rust)              │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│                  Beacon (Phase 1: Centralized)                  │
│  Production-ready pub/sub with forward-compatible interfaces   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                PubSub Network (Phase 2: Decentralized)          │
│         P2P network run by Cardano SPOs                        │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│              Shared Data Availability Layer                     │
│       Message persistence anchored on Cardano/Midnight         │
└─────────────────────────────────────────────────────────────────┘
```

## Key Differentiators

| Feature | PubSub | Generic Solutions |
|---------|--------|-------------------|
| **Identity** | Identus DIDs (native to Cardano) | EVM addresses or custom |
| **Infrastructure** | SPO network (3,000+ operators) | Bootstrap new network |
| **Compatibility** | Ouroboros miniprotocols | Requires adapters |
| **Governance** | Integrated with CIP-1694 | External to chain |

## Documentation Structure

| Section | Description | Audience |
|---------|-------------|----------|
| [Vision & Problem](vision.md) | Why this exists | All |
| [Requirements](requirements/index.md) | What we're building | PMs, Engineers |
| [Use Cases](use-cases/index.md) | Business scenarios | PMs, Stakeholders |
| [Architecture Drivers](architecture.md) | Technical constraints | Engineers |
| [Market Analysis](market.md) | Competition & opportunity | Executives |
| [Roadmap](roadmap.md) | When we're delivering | All |
| [Stakeholders & Team](stakeholders.md) | Who's involved | All |
| [Risks & Asks](risks.md) | What we need | Executives |

## Key Links

| Resource | Status | Owner |
|----------|--------|-------|
| Product Manager | Active | @Reza Baram |
| Tech Lead | Hiring | Target: Feb 2025 |
| Beacon PRD | 🟡 In Progress | [View](../beacon/index.md) |
| Public Roadmap | 🟡 Draft | [View](roadmap.md) |
| Market Requirements | 🟡 In Progress | [View](market.md) |
| Go To Market Plan | ⬜ Not Started | Target: Q2 2025 |

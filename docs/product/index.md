# Product Overview

!!! info "Audience: Executives, Product Managers, Stakeholders"

## Executive Summary

**Cardano PubSub** is the native communication layer for the Cardano ecosystem. It lets applications talk directly to your wallet — so you can vote on governance proposals, respond to DeFi alerts, and interact with dApps without ever leaving your interface.

The project follows a phased rollout:

- **Phase 1: Architecture & Prototyping** — Design and validate the decentralized network
- **Phase 2: PubSub Network** — Fully decentralized, SPO-operated messaging
- **Phase 3: Full Economy** — On-chain incentives and DAO governance

## The Elevator Pitch

> **For Cardano users and developers**, Cardano PubSub is a native messaging protocol that delivers **actionable notifications directly to your wallet**. 
>
> Unlike generic Web3 messaging (XMTP, Push Protocol) or centralized platforms (Discord, Telegram), PubSub is **built into Cardano's infrastructure** — using the SPO network for delivery and Identus DIDs for identity.
>
> This means: **One-click governance voting. Instant DeFi alerts. Token-gated communities. All without leaving your wallet.**

## The Problem We're Solving

Today, critical information in the Cardano ecosystem flows through:

| Current Channel | Problem |
|-----------------|---------|
| **Discord/Telegram** | Centralized, censorable, prime target for phishing |
| **Twitter/X** | Algorithm-controlled visibility, no verification |
| **Email** | Requires PII, spam-prone, not Web3-native |
| **dApp websites** | Users must actively check; no push notifications |

**Cardano PubSub solves this** by providing a native, decentralized, and secure communication channel.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                     Application Plane                           │
│        Wallets & dApps using PubSub SDK (TS/Rust)              │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│                PubSub Network (Decentralized)                   │
│         P2P network run by Cardano SPOs                        │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│              Shared Data Availability Layer                     │
│          Message persistence anchored on Cardano               │
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
| Tech Lead | Hiring | Target: Q1 2025 |
| Product Requirements | ✅ Draft Complete | [View](index.md) |
| Architecture Design | 🟡 In Progress | [View](../architecture/index.md) |
| Market Requirements | ✅ Draft Complete | [View](market.md) |

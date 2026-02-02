# Product Overview

!!! info "Audience: Executives, Product Managers, Stakeholders"

## Executive Summary

**Cardano PubSub** is the native communication layer for the Cardano ecosystem. It enables real-time, actionable messaging between users, dApps, and services — so you can vote on governance proposals, respond to DeFi alerts, and interact with the ecosystem from any compatible interface.

## The Elevator Pitch

> **For Cardano users and developers**, Cardano PubSub is a native messaging protocol that delivers **actionable notifications to users wherever they are** — in wallets, standalone clients, dashboards, or any PubSub-compatible application.
>
> Unlike generic Web3 messaging (XMTP, Push Protocol) or centralized platforms (Discord, Telegram), PubSub is **built into Cardano's infrastructure** — using the SPO network for delivery and a modular DID-based identity layer.
>
> This means: **One-click governance voting. Instant DeFi alerts. Token-gated communities. Access from any client you choose.**

## The Problem We're Solving

Today, critical information in the Cardano ecosystem flows through:

| Current Channel | Problem |
|-----------------|---------|
| **Discord/Telegram** | Centralized, censorable, prime target for phishing |
| **Twitter/X** | Algorithm-controlled visibility, no verification |
| **Email** | Requires PII, spam-prone, not Web3-native |
| **dApp websites** | Users must actively check; no push notifications |

**Cardano PubSub solves this** by providing a native, decentralized, and secure communication channel.

## Use Cases

PubSub is designed around five core scenarios. See [Use Cases](../use-cases/index.md) for details.

| Use Case | One-Line Summary |
|----------|------------------|
| [DeFi Intents](../use-cases/defi-intents.md) | Trade without ADA — agents cover your fees |
| [Governance](../use-cases/governance.md) | One-click voting from any PubSub client |
| [Autonomous Agents](../use-cases/autonomous-agents.md) | AI agents coordinate at machine speed |
| [Cross-Chain](../use-cases/cross-chain.md) | Bridge and stake in a single action |
| [Token-Gated Social](../use-cases/token-gated-social.md) | Private communities enforced by the blockchain |

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                     Application Plane                           │
│   Wallets, dApps, Dashboards & Clients using PubSub SDK        │
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

See [Architecture](../architecture/index.md) for technical details.

## Key Differentiators

| Feature | PubSub | Generic Solutions |
|---------|--------|-------------------|
| **Identity** | Modular DID support (Identus, did:pkh, did:peer) | EVM addresses or custom |
| **Infrastructure** | SPO network (3,000+ operators) | Bootstrap new network |
| **Compatibility** | Ouroboros miniprotocols | Requires adapters |
| **Governance** | Integrated with CIP-1694 | External to chain |

## Documentation Structure

| Section | Description | Audience |
|---------|-------------|----------|
| [Vision & Problem](vision.md) | Why this exists | All |
| [Requirements](requirements/index.md) | What we're building | PMs, Engineers |
| [Market Analysis](market.md) | Competition & opportunity | Executives |
| [KPI Alignment](kpi-alignment.md) | Cardano 2030 mapping | Executives, Intersect |
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
| Use Cases | ✅ Defined | [View](../use-cases/index.md) |

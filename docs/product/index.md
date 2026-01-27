# Product Overview

## Executive Summary

Agora is a unified, cross-ecosystem communication protocol designed to be the core communication fabric for Cardano, Midnight, and PartnerChains. It addresses the critical need for a secure and standardized way to communicate with users.

The project follows a phased rollout:

- **Phase 1: Beacon** — A centralized service designed to meet the immediate mainnet launch requirements for the Midnight ecosystem
- **Phase 2+: Agora** — A fully decentralized, SPO-operated network, creating a single, interoperable communication fabric

By pursuing native Cardano network integration and leveraging Hyperledger Identus for decentralized identity, Agora will:

- Eliminate security risks from Web2 platforms
- Unify the user experience across chains
- Serve as a foundational "workhorse" for interactive, multi-chain dApps and core protocol functions

## Elevator Pitch

> For the Cardano and Midnight ecosystems, Agora is a unified communication protocol that provides a native, trustless, and cross-chain messaging fabric. Unlike generic and centralized solutions, Agora will leverage native network integration and decentralized identity to deliver a secure, efficient, and fully integrated communication layer, unlocking a new wave of interactive, multi-chain applications.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                     Application Plane                           │
│  High-quality SDKs using Hyperledger Identus DIDs              │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│                  Beacon (Centralized MVP)                       │
│  Centralized pub/sub service with forward-compatible interface │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Agora (Decentralized)                         │
│  P2P network run by Cardano SPOs, native network protocol      │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│              Shared Data Availability Layer                     │
│  Potentially anchored on Midnight for censorship resistance    │
└─────────────────────────────────────────────────────────────────┘
```

## Documentation Structure

- [Vision & Problem](vision.md) — Mission, vision, problem statement, and solution
- [Requirements](requirements/index.md) — Functional and non-functional requirements
- [Use Cases](use-cases/index.md) — Detailed use case verticals
- [Architecture](architecture.md) — Key architectural drivers
- [Market Analysis](market.md) — Market size and competitive landscape
- [Roadmap](roadmap.md) — Phased delivery plan
- [Stakeholders & Team](stakeholders.md) — Key stakeholders, team, and metrics
- [Risks & Asks](risks.md) — Risk assessment and resource requirements

## Key Links

| Resource | Status |
|----------|--------|
| Product Manager | @Reza Baram |
| Tech Lead | TBD |
| Website | TBD |
| Public Roadmap | TBD |
| Private Strategic Roadmap | In Progress |
| Market Requirements Doc | In Progress |
| Go To Market Plan | TBD |

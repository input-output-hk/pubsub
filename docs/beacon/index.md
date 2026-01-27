# Beacon: Phase 1 MVP

!!! warning "Status: In Development"
    Beacon PRD is being finalized. Target completion: **February 2025**.

## What is Beacon?

Beacon is the **centralized MVP** of Cardano PubSub, designed to meet Midnight mainnet launch requirements. It provides a stable, forward-compatible pub/sub service that will later be migrated to the fully decentralized PubSub network.

## Why Centralized First?

| Reason | Benefit |
|--------|---------|
| **Speed to Market** | Midnight mainnet cannot wait for full decentralization |
| **Interface Stability** | Wallets integrate once; we swap the backend later |
| **Risk Reduction** | Prove the value proposition before building complex infra |
| **Feedback Loop** | Real usage informs decentralized design |

## Beacon Scope

### In Scope (MVP)

- [ ] Authenticated message publishing (Identus DID signatures)
- [ ] Topic-based subscription for wallets
- [ ] Push notifications to Lace wallet
- [ ] Message persistence (configurable retention)
- [ ] Basic rate limiting and spam prevention
- [ ] REST/WebSocket API for wallet integration

### Out of Scope (Deferred to Phase 2)

- P2P network topology
- SPO node operation
- On-chain economic model
- MLS group encryption
- Cross-chain message verification

## Target Integrations

| Integration | Priority | Status |
|-------------|----------|--------|
| **Lace Wallet** | P0 | 🟡 In Discussion |
| **Midnight Foundation** | P0 | 🟡 In Discussion |
| **Governance dApps** | P1 | ⬜ Not Started |

## Architecture (Simplified)

```
┌─────────────────────────────────────────────────────────────────┐
│                      Lace / Wallets                             │
│                    (WebSocket Client)                           │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Beacon Service                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │   API GW    │  │  Pub/Sub    │  │  Identity   │             │
│  │  (REST/WS)  │  │   Engine    │  │  (Identus)  │             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
│                          │                                      │
│                   ┌──────┴──────┐                               │
│                   │  PostgreSQL │                               │
│                   │  (Messages) │                               │
│                   └─────────────┘                               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Midnight / Cardano L1                        │
│                  (Event Sources / Settlement)                   │
└─────────────────────────────────────────────────────────────────┘
```

## Delivery Timeline

**Starting: September 1, 2025**

| Week | Milestone | Owner |
|------|-----------|-------|
| 1-2 | Finalize MVP scope, streamline PRD | PM (Reza) |
| 3-6 | Prototype core flows, validate interfaces | Engineering |
| 7-8 | Performance benchmarking, architecture validation | Engineering |
| 9 | Lock MVP design, onboard engineering team | PM + Tech Lead |
| 10-12 | Production implementation for Midnight mainnet | Engineering |

## API Reference

!!! info "Coming Soon"
    API documentation will be published once interfaces are locked (Week 9).

Planned endpoints:

```
POST   /v1/publish          # Publish a message
GET    /v1/subscribe        # WebSocket subscription
GET    /v1/topics           # List available topics
GET    /v1/messages/{id}    # Retrieve specific message
DELETE /v1/messages/{id}    # Delete own message
```

## Migration Path to PubSub

Beacon is designed for **seamless migration** to the decentralized network:

1. **API Compatibility**: Beacon APIs will remain supported as a "gateway" mode
2. **Topic Structure**: Same topic taxonomy used in decentralized network
3. **Identity**: Identus DIDs work identically in both systems
4. **No Breaking Changes**: Wallets integrated with Beacon work with PubSub without code changes

See [Migration Strategy](../product/roadmap.md#migration-strategy) for details.

## Open Questions

| Question | Status | Owner |
|----------|--------|-------|
| Exact message format for Midnight notifications | 🟡 In Discussion | PM + Midnight Team |
| Push notification infrastructure (FCM/APNs vs. WebSocket-only) | ⬜ Not Decided | Engineering |
| SLA requirements for Midnight mainnet | ⬜ Not Defined | PM + Midnight Team |

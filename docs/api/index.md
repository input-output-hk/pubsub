# API Reference

!!! warning "Status: In Development"
    API specifications will be published when Beacon interfaces are locked (target: Week 9 of development).

## Overview

Cardano PubSub provides APIs at two levels:

1. **Beacon API** (Phase 1) — REST/WebSocket for wallet integration
2. **Node API** (Phase 2+) — gRPC/GraphQL for direct node interaction

## Beacon API (Coming Soon)

### Authentication

All requests require Identus DID authentication:

```http
Authorization: DID-Signature <did>:<signature>:<timestamp>
```

### Endpoints (Draft)

#### Publish Message

```http
POST /v1/publish
Content-Type: application/json

{
  "topic": "governance/proposals",
  "payload": { ... },
  "ttl": 86400
}
```

#### Subscribe to Topic

```http
GET /v1/subscribe?topics=governance/proposals,defi/alerts
Upgrade: websocket
```

#### List Topics

```http
GET /v1/topics?prefix=governance
```

### WebSocket Events

```json
{
  "type": "message",
  "topic": "governance/proposals",
  "id": "msg_abc123",
  "sender": "did:prism:xyz",
  "payload": { ... },
  "timestamp": 1706345678
}
```

## SDK Libraries (Planned)

| Language | Package | Status |
|----------|---------|--------|
| TypeScript | `@cardano-pubsub/sdk` | 🟡 Planned |
| Rust | `cardano-pubsub` | 🟡 Planned |
| Python | `cardano-pubsub-py` | ⬜ Future |

## Integration Guide

*Coming soon* — Step-by-step guide for wallet developers.

## Rate Limits

| Tier | Publish | Subscribe | Notes |
|------|---------|-----------|-------|
| Free | 10/min | 5 topics | Basic usage |
| Standard | 100/min | 50 topics | Registered dApps |
| Premium | 1000/min | Unlimited | SPO operators |

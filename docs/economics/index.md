# Economic Model

!!! warning "Status: Early Design"
    The economic model is being developed for Phase 3. This section outlines the design space and open questions.

## Overview

The Cardano PubSub economic model must solve a fundamental challenge: **incentivizing SPOs to run relay nodes** while keeping message costs low enough for mass adoption.

## Design Principles

1. **SPO Alignment** — Leverage existing Cardano stake incentives rather than creating competing tokenomics
2. **Low Friction** — Users shouldn't need to hold a special token to send basic messages
3. **Spam Resistance** — Economic costs must prevent network abuse
4. **Sustainable** — Node operators must cover infrastructure costs

## Economic Actors

| Actor | Incentive | Cost |
|-------|-----------|------|
| **Message Publishers** | Reach subscribers | Message fees (or sponsored) |
| **Subscribers** | Receive relevant notifications | Free (pull model) |
| **SPO Relay Nodes** | Fee revenue + stake rewards | Infrastructure + bandwidth |
| **Agents/Solvers** | Profit from executing intents | Message fees + compute |
| **Sponsors** | User acquisition, ecosystem growth | Subsidize user fees |

## Fee Model Options

### Option A: Native ADA Fees

| Pros | Cons |
|------|------|
| Simple, no new token | Competes with L1 fee market |
| Familiar to users | Harder to tune independently |
| No liquidity bootstrap needed | May be too expensive for high-volume |

### Option B: Dedicated PUBSUB Token

| Pros | Cons |
|------|------|
| Independent fee tuning | Token bootstrap complexity |
| Can implement staking/slashing | Additional friction for users |
| Governance over parameters | Regulatory considerations |

### Option C: Hybrid (Recommended Direction)

- **Basic messages**: Free or ADA micro-fees
- **Premium features** (guaranteed delivery, priority): PUBSUB token or higher ADA
- **SPO rewards**: Combination of ADA delegation rewards + protocol fees

## SPO Incentive Structure

```
┌─────────────────────────────────────────────────────────┐
│                   SPO Revenue Sources                    │
├─────────────────────────────────────────────────────────┤
│  1. Existing ADA staking rewards (unchanged)            │
│  2. PubSub relay fees (% of message fees)               │
│  3. Premium service fees (archival, guaranteed QoS)     │
│  4. Protocol treasury grants (bootstrap period)         │
└─────────────────────────────────────────────────────────┘
```

## Fee Schedule (Draft)

| Message Type | Estimated Cost | Rationale |
|--------------|----------------|-----------|
| Basic notification | Free or <0.001 ADA | Mass adoption |
| Governance vote | Free (sponsored) | Participation incentive |
| DeFi intent | 0.01-0.1 ADA | Value extraction justifies cost |
| Guaranteed delivery | 0.1+ ADA | Premium service |
| Large payload (>10KB) | Per-KB pricing | Resource usage |

## Open Questions

| Question | Options | Decision By |
|----------|---------|-------------|
| Token vs. no token? | ADA-only / Hybrid / New token | Phase 2 design |
| Who pays for governance messages? | Treasury / DAOs / Free | Q2 2025 |
| SPO minimum stake requirement? | None / 100K ADA / 1M ADA | Phase 2 design |
| Slashing for bad behavior? | Yes / No / Reputation only | Phase 2 design |

## Research References

- [Waku RLN (Rate Limiting Nullifiers)](https://docs.waku.org/learn/rln/) — Spam prevention via staking
- [Filecoin Storage Market](https://spec.filecoin.io/) — Economic model for decentralized storage
- [Cardano Staking Economics](https://docs.cardano.org/learn/pledging-rewards/) — Existing SPO incentive structure

## Timeline

| Milestone | Target |
|-----------|--------|
| Economic model research | Q1 2026 |
| Community feedback RFC | Q2 2026 |
| Testnet with economics | Q3 2026 |
| Mainnet economic launch | Q4 2026 (Phase 3) |

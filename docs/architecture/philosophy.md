# Architectural Philosophy

The architecture is built on core principles derived from AUEB research and executive strategic alignment.

## Core Principles

### 1. Decoupling

We separate the **Signaling Layer (Agora)** from the **Settlement Layer (Cardano L1)**.

This allows Agora to operate at speeds (milliseconds) and costs (near-zero) that the L1 cannot match, while relying on the L1 for finality.

```
┌──────────────────┐     ┌──────────────────┐
│   Agora (DMB)    │     │   Cardano L1     │
│                  │     │                  │
│  • Milliseconds  │     │  • Seconds       │
│  • Near-zero $   │────▶│  • Tx fees       │
│  • Ephemeral     │     │  • Immutable     │
│  • High volume   │     │  • Consensus     │
└──────────────────┘     └──────────────────┘
    Signaling              Settlement
```

### 2. Native Integration

The system must **not be an alien component** to Stake Pool Operators (SPOs).

It must leverage existing Cardano networking primitives and identity standards (Identus) to ensure frictionless adoption.

!!! tip "Why This Matters"
    SPOs already run complex infrastructure. Agora should feel like a natural extension of their existing stack, not a foreign system requiring new expertise.

### 3. Hybrid Dissemination

To satisfy both **high reliability** (Governance) and **low latency** (DeFi), the network utilizes a hybrid topology:

| Topology | Purpose | Use Case |
|----------|---------|----------|
| **Harary Graph** | Structured backbone for guaranteed delivery | Governance proposals |
| **GossipSub** | Randomized flood-fill for speed | DeFi intents |

### 4. Tiered Storage

Recognizing that "DeFi Intents" expire in minutes while "Governance Proposals" last weeks, the architecture rejects a "one-size-fits-all" database in favor of **configurable retention policies per topic**.

| Data Type | TTL | Storage Tier |
|-----------|-----|--------------|
| DeFi Intents | ~10 minutes | Hot Cache (RAM) |
| Solver Bids | ~5 minutes | Hot Cache (RAM) |
| Governance Proposals | 14+ days | Durable DHT (Disk) |
| Social Messages | Configurable | Durable DHT (Disk) |

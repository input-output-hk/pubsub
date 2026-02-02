# PROPOSAL
## Proposal ID: CBU018 — Decentralised Pub/Sub (Cardano PubSub)

---

# 1. The Proposal (What)

## 1.1 Description

### The Problem: Coordination is the Missing Layer

Blockchains have solved settlement. On-chain verification works. What remains primitive — across every network — is off-chain coordination: how actors discover, negotiate, and prepare transactions before they hit the chain.

This gap has cost the industry over $1 billion. The Ronin Bridge hack went undetected for six days because no alert system existed. Terra's collapse saw ad-hoc Twitter threads attempting to coordinate chain halts. Solana restarts require validators to paste ledger heights into Discord chat. Networks that orchestrate thousands of nodes for consensus still rely on Discord pings for emergencies.

Cardano feels this acutely:

- **No authenticated broadcast system** for protocol-level alerts or SPO coordination
- **No standard way** for dApps to push notifications to users
- **No infrastructure for intent-based systems** where users broadcast what they want and agents compete to fulfill it
- **Each application builds its own messaging stack**, fragmenting the ecosystem and inheriting centralization risks

### The Solution: Cardano PubSub

**Cardano PubSub** is a decentralized message bus operated by Cardano's 3,000+ SPOs. It provides permissionless publish/subscribe infrastructure for any application that needs reliable, censorship-resistant messaging.

PubSub is designed around five core use cases:

| Use Case | Problem It Solves |
|----------|-------------------|
| **DeFi Intents** | Users broadcast trading intents; agents discover and fulfill them, enabling Babel Fees and "invisible gas" UX |
| **Governance** | Verified proposals with one-click voting actions, reducing participation friction under CIP-1694 |
| **Network Operations** | Authenticated emergency alerts with delivery guarantees, replacing fragmented Discord/Telegram channels |
| **Cross-Chain** | Foundation for bridge protocols and multi-chain coordination |
| **Agent Coordination** | High-throughput coordination for automated systems (keepers, solvers, searchers) |

### Why Cardano Can Lead Here

Cardano has structural advantages that competitors lack:

- **3,000+ SPOs** already run distributed infrastructure — no need to bootstrap a new network
- **Mithril network** proves the model: 250 SPOs already participate in coordinated threshold signatures
- **libp2p stack** is deployed internally (Hydra, Mithril) and needs only to be exposed to applications
- **CIP-0137** has already proposed using Cardano's network layer for decentralized messaging

**Why now?** The competitive window is 12-18 months before XMTP and Anoma reach production mainnet. Nested Transactions (CIP-118) shipping in 2026 enables Babel Fees — but without a message bus, there's no delivery mechanism. IOG Research has validated the protocol design through collaboration with Athens University. The infrastructure exists; we need to expose it.

---

## 1.2 Deliverables

| Sequence | Item Description | High Level Estimates |
|----------|------------------|---------------------|
| **Q3 2026** | Architecture finalization; P2P prototype with modular DID integration; IOG Research protocol validation | 2 engineers for 3 months (ramp-up) |
| **Q4 2026** | SDK draft (JavaScript, Rust) for early integrators; SPO testnet deployment; Intent schema v1 specification | 4 engineers for 3 months |
| **Q1 2027** | Production mainnet launch; Wallet integration support (Lace, Eternl); Agent framework reference implementation | 4 engineers for 3 months |
| **Q2 2027** | Developer documentation & tutorials; 10+ dApp integrations; Monitoring & observability tools | 4 engineers for 3 months |

---

# 2. Proposed Value Delivered (Why)

### Strategic Context

PubSub is foundational infrastructure. Like networking in traditional software, messaging is invisible when it works — but its absence constrains everything built on top.

The proposal addresses three gaps:

1. **Emergency coordination** — Authenticated alerts replacing Discord, with $1B+ in documented losses from the current gap (Ronin, Terra, Solana outages)
2. **Application messaging** — Standard infrastructure so dApps don't each build their own fragmented solutions
3. **Intent delivery** — Enabling Babel Fees and next-generation DeFi by giving intents a delivery mechanism

## 2.1 KPIs

| Core Cardano 2030 KPIs (Adoption) | Alignment | KPI Alignment Narrative |
|-----------------------------------|-----------|-------------------------|
| **TVL** | Yes, enable | Enables Babel Fees, removing friction for users to lock value in DeFi protocols. Users without ADA can now participate. Lower barriers → higher TVL. |
| **Monthly Transactions** | Yes, directly | Every fulfilled intent generates 1+ on-chain transactions. DeFi agent activity drives high transaction volume. Target impact: +1M tx/month from intent fulfillment. |
| **Monthly Active Users (MAU)** | Yes, enable | ADA-free intents dramatically lower barrier for new users. Users who couldn't participate (no ADA holdings) can now transact via Babel Fee agents. |

## 2.2 Additional KPIs

| Additional Cardano 2030 KPIs | Alignment | KPI Alignment Narrative |
|------------------------------|-----------|-------------------------|
| **Reliability: Monthly Uptime (6 epochs)** | N/A | PubSub is off-chain infrastructure; does not directly impact L1 uptime. |
| **Operational Resilience: Voting Power Distribution** | N/A | Not directly related to stake distribution. |
| **Operational Resilience: Alternative Full Node Clients** | N/A | PubSub is separate from node client diversity. |
| **Revenue / Adoption: Annual Protocol Revenue** | Yes, enable | More transactions from intent fulfillment → more transaction fees → higher protocol revenue. |
| **Governance: DRep Participation Rate** | Yes, enable | PubSub enables verified governance notifications with one-click voting actions. Projected +20% voter turnout improvement. |
| **Scalability: Throughput Capacity per day** | Yes, enable | Intent batching by agents can optimize transaction throughput. Agents aggregate multiple intents into efficient transaction bundles. |

The governance angle matters: CIP-1694 introduces on-chain governance, but participation requires tracking proposals across scattered channels. PubSub enables direct, authenticated notifications with embedded voting actions — reducing friction between awareness and action.

## 2.3 Pillars

| Cardano 2030 Pillars | Alignment | Pillar Alignment Description |
|----------------------|-----------|------------------------------|
| **Pillar 1: Infrastructure & Research Excellence** — *Keep Cardano secure, fast, and interoperable so it can host more economic activity.* | Yes, directly | PubSub is core communication infrastructure for the Cardano network. Protocol design validated by IOG Research (Athens University collaboration). SPO-operated network leverages existing infrastructure. |
| **Pillar 2: Adoption & Utility** — *Driving widespread, non-speculative utility by focusing on high-value verticals, superior UX, and enterprise-grade security.* | Yes, directly | Enables ADA-free transactions via Babel Fees, dramatically lowering barrier to entry. Powers DeFi (intents), Governance (verified voting), and Network Operations (emergency coordination) use cases. Superior UX through "invisible gas" experience. |
| **Pillar 3: Governance** — *Cardano governance must be hard to capture, easy to use, and paced.* | Yes, enable | Enables verified, cryptographically-signed governance notifications. One-click voting from any PubSub-compatible client. Reduces phishing risk that plagues current Discord/Telegram governance communications. |
| **Pillar 4: Community & Ecosystem Growth** — *Driving global engagement through market-centric approach, cultivating skilled developer base, and demonstrating ecosystem value.* | Yes, enable | Provides developer SDKs (JavaScript, Rust, Python) and REST/WebSocket APIs. Creates new economic category of "agent operators" in ecosystem. Enables new dApp categories (intent-based DeFi, cross-chain bridges). |
| **Pillar 5: Ecosystem Sustainability & Resilience** — *Ensuring the long-term financial health and operational integrity of the network infrastructure.* | Yes, enable | SPO-operated relay network creates new revenue stream for stake pool operators (relay fees). Diversifies SPO income beyond block production, strengthening network decentralization. |

---

# 3. Roadmap

**Miro Board:** [Link TBD] (pwd: 1ntersect26)

### Visual Timeline

```
Q3 2026              Q4 2026              Q1 2027              Q2 2027
    │                    │                    │                    │
    ▼                    ▼                    ▼                    ▼
┌────────────┐      ┌────────────┐      ┌────────────┐      ┌────────────┐
│ FOUNDATION │      │   BUILD    │      │  LAUNCH    │      │   GROW     │
│ Architecture│  ──▶│  SDK &     │  ──▶│  Mainnet   │  ──▶│  Ecosystem │
│ & Prototype │      │  Testnet   │      │            │      │            │
└────────────┘      └────────────┘      └────────────┘      └────────────┘
      │                   │                   │                   │
      ▼                   ▼                   ▼                   ▼
• Architecture        • JS/Rust SDK       • Production        • 10+ dApp
  finalization        • SPO testnet         mainnet             integrations
• P2P prototype       • Early             • Lace/Eternl       • Developer
• DID module            integrators         wallet support      documentation
• IOG Research        • Intent            • Agent             • Monitoring &
  validation            schema v1           framework           observability
```

---

# 4. Resources & Dependencies

## Related Projects
| Project | Owner | Dependency Type |
|---------|-------|-----------------|
| DeFi Intents | Michael Smolenski | Parent initiative — PubSub is the message bus |
| Nested Transactions (CIP-118) | Alexey Kuleshevich (Ledger Team) | Hard dependency — enables Babel Fees |
| BitcoinVMX Bridge | Torben Poguntke | Integration — cross-chain intent use case |

## Documentation
- **Product Docs:** https://input-output-hk.github.io/pubsub/
- **Research Foundation:** IOG Research Paper (Athens University collaboration)

---

# 5. Spreadsheet Fields Summary

| Field | Value |
|-------|-------|
| **Proposal ID** | CBU018 |
| **Proposal Item** | Decentralised Pub/Sub |
| **Type** | New Product |
| **High Level Groupings** | Scale and harden the protocol |
| **Owner** | Reza Baram |
| **KPIs** | Builder NPS, TVL (enable), Monthly Transactions (direct), MAU (enable) |
| **Pillars** | Pillar 1 (directly), Pillar 2 (directly), Pillar 3-5 (enable) |
| **Personas** | Builder (core infra + tooling), Crypto Novice, Crypto Savvy, Developer (Dapps), Finance Professional, Operator SPO, Influencer |
| **AI Augmented** | Partially (enables agent coordination) |
| **Value to Midnight** | Potential (messaging infrastructure could extend to Midnight) |
| **T-Shirt Size** | M |
| **Funding (USD)** | ~$800,000 - $1,000,000 (4 devs × 9-12 months + architecture) |
| **Story Theme** | Communication efficiency |

---

*Prepared: 2026-02-02*
*PM: Reza Baram*
*Status: DRAFT*

**Assumptions:**
- Team: Hire or vendor (no existing team)
- Ramp: 2 engineers Q3 2026 → 4 engineers Q4 2026 onwards
- Duration: 12 months (Q3 2026 - Q2 2027)
- Total: ~3.5 FTE-years engineering + PM

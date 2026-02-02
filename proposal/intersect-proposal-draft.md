# PROPOSAL
## Proposal ID: Cardano PubSub - Decentralized Message Bus

---

# 1. The Proposal (What)

## 1.1 Description

### The User Perspective

*As a Cardano wallet user*, I want to trade tokens without holding ADA for fees, so that I can participate in DeFi without the friction of managing gas tokens.

*As a DeFi agent operator*, I want to discover and fulfill user intents in real-time, so that I can earn fees by providing liquidity and transaction services.

*As a governance participant*, I want to receive verified proposals and vote directly from my wallet, so that I don't miss critical decisions or fall for phishing scams.

### The Problem

The Cardano ecosystem lacks a native, decentralized communication layer. Today:

| Current State | Impact |
|---------------|--------|
| DeFi alerts via Discord/Telegram | Users miss liquidation warnings; lose funds |
| Governance announcements on Twitter | Low visibility; low voter turnout |
| No standard intent format | Each DEX builds isolated order flow; no shared liquidity |
| No ADA-free transactions | Users without ADA can't participate in DeFi |

**The DeFi Intents initiative** (led by Michael Smolenski) requires a **Decentralized Message Bus** to enable gasless trading via Babel Fees and Nested Transactions. Without PubSub, intents have no delivery mechanism.

### The Solution

**Cardano PubSub** provides the missing communication primitive:

1. **Permissionless Network** — Anyone can publish/subscribe without approval
2. **ADA-Free Broadcasting** — Users broadcast intents without paying fees
3. **Censorship Resistant** — Operated by 3,000+ Cardano SPOs
4. **Standard Message Format** — Unified intent schema for all DeFi agents

### Why Now

- **Nested Transactions (CIP-118)** landing in 2026 — enables Babel Fees
- **DeFi Intents initiative** active — PubSub is the designated message bus
- **Competitor ecosystems** (Ethereum via ERC-4337, Solana via Jito) shipping intent infrastructure
- **SPO infrastructure** already exists — no new network bootstrap required

---

## 1.2 Deliverables

| Sequence | Item Description | High Level Estimates |
|----------|------------------|---------------------|
| **Q3 2026** | Architecture finalization & P2P prototype with modular DID integration | *[TBD - need team size]* |
| **Q4 2026** | SDK draft (JS, Rust) for early integrators; SPO testnet deployment | *[TBD - need team size]* |
| **Q1 2027** | Production mainnet launch; wallet integration support (Lace, Eternl) | *[TBD - need team size]* |
| **Q2 2027** | Developer documentation; 10+ dApp integrations; monitoring tools | *[TBD - need team size]* |

---

# 2. Proposed Value Delivered (Why)

PubSub is **critical infrastructure** that enables the entire DeFi Intents initiative. Without it, users cannot broadcast intents, agents cannot discover them, and Babel Fees cannot function.

## 2.1 KPIs

| Core Cardano 2030 KPIs (Adoption) | Alignment | KPI Alignment Narrative |
|-----------------------------------|-----------|-------------------------|
| **TVL** | Yes, enable | Enables Babel Fees, reducing friction for users to lock value in DeFi protocols. Lower barriers → more TVL. |
| **Monthly Transactions** | Yes, directly | Every intent fulfilled becomes 1+ transactions. DeFi agents generate high transaction volume. Target: +1M tx/month from intent fulfillment. |
| **Monthly Active Users (MAU)** | Yes, enable | ADA-free intents lower barrier for new users. Users who couldn't participate (no ADA) can now transact via Babel Fees. |

## 2.2 Additional KPIs

| Additional Cardano 2030 KPIs | Alignment | KPI Alignment Narrative |
|------------------------------|-----------|-------------------------|
| **Reliability: Monthly Uptime (6 epochs)** | N/A | PubSub is infrastructure; doesn't directly impact chain uptime. |
| **Operational Resilience: Voting Power Distribution** | N/A | Not directly related. |
| **Operational Resilience: Alternative Full Node Clients** | N/A | Not directly related. |
| **Revenue / Adoption: Annual Protocol Revenue** | Yes, enable | More transactions → more fees. Intent fulfillment drives protocol revenue. |
| **Governance: DRep Participation Rate** | Yes, enable | PubSub enables verified governance notifications with one-click voting. Projected +20% turnout. |
| **Scalability: Throughput Capacity per day** | Yes, enable | Intent batching via agents can optimize transaction throughput. |

## 2.3 Pillars

| Cardano 2030 Pillars | Alignment | Pillar Alignment Description |
|----------------------|-----------|------------------------------|
| **Pillar 1: Infrastructure & Research Excellence** | Yes, directly | PubSub is core communication infrastructure for the Cardano network. Based on IOG Research paper (Athens University collaboration). |
| **Pillar 2: Adoption & Utility** | Yes, directly | Enables ADA-free transactions (Babel Fees), dramatically lowering barrier to entry for new users. Powers DeFi, governance, and social use cases. |
| **Pillar 3: Governance** | Yes, enable | Enables verified governance notifications and one-click voting from any PubSub client. |
| **Pillar 4: Community & Ecosystem Growth** | Yes, enable | Provides developer SDKs (JS, Rust, Python) and APIs. Creates new category of "agent operators" in ecosystem. |
| **Pillar 5: Ecosystem Sustainability & Resilience** | Yes, enable | SPO-operated network creates new revenue stream for stake pool operators. Decentralized architecture ensures resilience. |

---

# 3. Roadmap

**Miro Board:** [To be added] (pwd: 1ntersect26)

### Visual Timeline

```
Q3 2026          Q4 2026          Q1 2027          Q2 2027
   │                │                │                │
   ▼                ▼                ▼                ▼
┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐
│ Arch &   │   │ SDK &    │   │ Mainnet  │   │ Ecosystem│
│ Prototype│──▶│ Testnet  │──▶│ Launch   │──▶│ Growth   │
└──────────┘   └──────────┘   └──────────┘   └──────────┘
     │              │              │              │
     ▼              ▼              ▼              ▼
• Architecture   • JS/Rust SDK  • Production   • 10+ dApp
  finalization   • SPO testnet    network        integrations
• P2P prototype  • Early        • Lace/Eternl  • Developer
• DID module       integrators    support        docs
• IOG Research   • Intent       • Agent        • Monitoring
  validation       schema v1      framework      tools
```

---

# Resources

- **Documentation:** http://192.168.1.155:8000 (internal)
- **Related Projects:**
  - DeFi Intents (Michael Smolenski)
  - Nested Transactions / CIP-118 (Alexey Kuleshevich, Ledger Team)
  - BitcoinVMX Bridge (Torben Poguntke)
- **Research Foundation:** IOG Research Paper (Athens University)

---

*Draft prepared: 2026-01-29*
*PM: Reza Baram*
*Status: AWAITING TEAM CAPACITY INFO*

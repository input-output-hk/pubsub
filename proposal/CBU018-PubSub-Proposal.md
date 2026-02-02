# PROPOSAL
## Proposal ID: CBU018 — Decentralised Pub/Sub (Cardano PubSub)

---

# 1. The Proposal (What)

## 1.1 Description

Cardano users today face a critical friction point: to participate in DeFi, they must first acquire ADA for transaction fees. A new user wanting to swap tokens, provide liquidity, or interact with a dApp cannot do so without this prerequisite—a barrier that competing ecosystems like Ethereum (via ERC-4337 account abstraction) and Solana (via Jito) are actively eliminating.

The Cardano ecosystem lacks a native, decentralized communication layer to solve this. Currently, DeFi alerts flow through Discord and Telegram where users miss liquidation warnings; governance proposals are announced on Twitter where algorithm-controlled visibility drives chronically low voter turnout; and each DEX builds isolated order flow with no shared liquidity or standard message format.

**Cardano PubSub** is the missing infrastructure primitive that enables the DeFi Intents initiative. It provides the **Decentralized Message Bus** that allows users to broadcast trading intents without paying ADA fees, while specialized agents discover and fulfill these intents—covering transaction costs via Babel Fees and earning fees in return.

The system works as follows: A user wants to swap USDC for ADA but holds no ADA for fees. They broadcast a signed intent to the PubSub network specifying their desired trade. DeFi agents monitoring the network discover this intent, compete to fulfill it, and submit the transaction on-chain—covering the ADA fee and taking a small spread. The user receives their ADA without ever holding gas tokens. This "invisible gas" experience is table stakes for mainstream DeFi adoption.

PubSub delivers four core capabilities: (1) a **permissionless network** where anyone can publish or subscribe without gatekeepers; (2) **ADA-free broadcasting** so users can participate without holding native tokens; (3) **censorship resistance** through operation by 3,000+ Cardano SPOs; and (4) a **standard message format** enabling interoperability across wallets, agents, and dApps.

**Why now?** Nested Transactions (CIP-118) shipping in 2026 enables Babel Fees at the protocol level—but without a message bus, intents have no delivery mechanism. The DeFi Intents initiative is active and has designated PubSub as the communication layer. IOG Research has validated the protocol design through collaboration with Athens University. And critically, the SPO infrastructure already exists—we don't need to bootstrap a new network.

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

PubSub is **critical enabling infrastructure** for the DeFi Intents initiative. Without it, users cannot broadcast intents, agents cannot discover them, and Babel Fees cannot function. This directly impacts Cardano's ability to compete with Ethereum and Solana on DeFi UX.

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

## 2.3 Pillars

| Cardano 2030 Pillars | Alignment | Pillar Alignment Description |
|----------------------|-----------|------------------------------|
| **Pillar 1: Infrastructure & Research Excellence** — *Keep Cardano secure, fast, and interoperable so it can host more economic activity.* | Yes, directly | PubSub is core communication infrastructure for the Cardano network. Protocol design based on IOG Research paper (Athens University collaboration). SPO-operated network leverages existing infrastructure. |
| **Pillar 2: Adoption & Utility** — *Driving widespread, non-speculative utility by focusing on high-value verticals, superior UX, and enterprise-grade security.* | Yes, directly | Enables ADA-free transactions via Babel Fees, dramatically lowering barrier to entry. Powers DeFi (intents, swaps), Governance (verified voting), and Social (token-gated communities) use cases. Superior UX through "invisible gas" experience. |
| **Pillar 3: Governance** — *Cardano governance must be hard to capture, easy to use, and paced.* | Yes, enable | Enables verified, cryptographically-signed governance notifications. One-click voting from any PubSub-compatible client. Reduces phishing risk that plagues current Discord/Telegram governance communications. |
| **Pillar 4: Community & Ecosystem Growth** — *Driving global engagement through market-centric approach, cultivating skilled developer base, and demonstrating ecosystem value.* | Yes, enable | Provides developer SDKs (JavaScript, Rust, Python) and REST/WebSocket APIs. Creates new economic category of "agent operators" in ecosystem. Enables new dApp categories (intent-based DeFi, token-gated social). |
| **Pillar 5: Ecosystem Sustainability & Resilience** — *Ensuring the long-term financial health and operational integrity of the network infrastructure.* | Yes, enable | SPO-operated relay network creates new revenue stream for stake pool operators (relay fees). Decentralized architecture ensures no single point of failure. Aligns SPO incentives with network utility beyond block production. |

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
- **Product Docs:** http://192.168.1.155:8000 (internal)
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
| **AI Augmented** | Partially (enables autonomous AI agents) |
| **Value to Midnight** | Potential (messaging infrastructure could extend to Midnight) |
| **T-Shirt Size** | M |
| **Funding (USD)** | ~$800,000 - $1,000,000 (4 devs × 9-12 months + architecture) |
| **Story Theme** | Communication efficiency |

---

---

*Prepared: 2026-01-29*
*PM: Reza Baram*
*Status: READY FOR REVIEW*

**Assumptions:**
- Team: Hire or vendor (no existing team)
- Ramp: 2 engineers Q3 2026 → 4 engineers Q4 2026 onwards
- Duration: 12 months (Q3 2026 - Q2 2027)
- Total: ~3.5 FTE-years engineering + PM

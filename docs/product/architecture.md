# Key Architectural Drivers

This section identifies the primary functional drivers for the Cardano PubSub architecture, derived from the use case analysis.

## Key Use Cases Overview

| Use Case | User Story | Cardano PubSub's Role | Critical Architectural Drivers |
|----------|------------|--------------|-------------------------------|
| **DeFi Intents & Solvers** | A user broadcasts an intent to swap assets (e.g., BTC for ADA) without paying ADA fees. Solvers compete to fulfill it. | Real-time Order Book: Acts as the ephemeral propagation layer for partial transactions (SubTx) and solver bids. | **Low Latency (<1s):** Solvers must see intents immediately to compete. **Ephemeral Storage:** Intents only need to persist for minutes (TTL). **Topic Filtering:** Solvers must efficiently subscribe to specific pairs (e.g., `intents/btc-ada`). |
| **Actionable DAO Governance** | A user receives a governance proposal notification in their wallet and casts a vote ("Yes/No") directly via a signed message. | Secure Notification Bus: Delivers authenticated proposal metadata and collects signed vote messages. | **Delivery Guarantees:** 100% reliability required; missing a vote notification is unacceptable. **Long-Term Persistence:** Proposal data must remain accessible for the voting period (e.g., 14 days). **Spam Resistance:** Critical to prevent drowning out official DAO comms. |
| **Autonomous Agent Coordination** | Autonomous AI agents negotiate complex tasks (e.g., multi-hop arbitrage or DAO resource allocation) via rapid message exchange. | Machine-to-Machine (M2M) Bus: Facilitates high-volume, structured communication between non-human actors. | **High Throughput:** Network must handle bursts of negotiation "chatter" between agents. **Structured Payloads:** Native support for Protobuf/JSON schemas. **Deterministic Ordering:** Critical for agents agreeing on sequence of events. |
| **Cross-Chain "Bridge & Stake"** | A user on a Partner Chain (e.g., Midnight) receives a signal to bridge assets to Cardano and stake them in a single action. | Cross-Chain Signaling Layer: Carries proofs and intent signals across chain boundaries without centralized bridges. | **Light Client Verification:** Messages may need to carry compact cryptographic proofs (Mithril/ZK) payload support. **Partner Chain Compatibility:** Architecture must support non-Cardano addressing standards. |
| **Token-Gated Social Feeds** | Users access exclusive chat groups based on NFT/Token holdings. Messages are encrypted and stored off-chain. | Decentralized Social Graph: Manages permissions based on on-chain state and routes encrypted user content. | **Encryption & Privacy:** End-to-End Encryption (E2EE) and metadata protection (Sealed Sender). **Scalability:** Must support millions of topics (one per group/DM) and potentially large message histories. **On-Chain Permissioning:** Node logic must query L1 ledger state to validate access rights before relaying. |

## Architectural Implications Summary

### 1. Storage Tiers
The contrast between DeFi Intents (ephemeral) and Governance (durable) implies Cardano PubSub needs a **Tiered Storage Architecture**:
- Hot Cache for intents
- Durable DHT for governance

### 2. Message Prioritization
AI Agents and DeFi require low latency, whereas Social feeds may tolerate higher latency but require vastly more storage. The protocol likely needs **Quality of Service (QoS) tiers**.

### 3. Verification Module
Token-Gating and Cross-Chain use cases require the Cardano PubSub Node to have a modular **"State Verification" component** that can check Cardano L1 (or Partner Chain) state to authorize actions.

## Beacon Examples

Messages to be delivered by the Midnight Foundation to end users via Lace wallet:

!!! example "Sample Notifications"
    - "The Glacier Drop phase 2 is live. Claim your lost & found NIGHT tokens via the portal at https://midnight.gd"
    - "The new node version XYZ is out. To update your Midnight Block Producer access https://node.midnight.network"
    - "The governance council has opened voting for governance action number 26. NIGHT holders are welcome to cast their votes until Aug-31 via the portal at https://governance.midnight.network"

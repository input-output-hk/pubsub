# Glossary

Key terms and concepts used throughout the Cardano PubSub documentation.

## Product & Project Names

| Term | Definition |
|------|------------|
| **Cardano PubSub** | The decentralized messaging protocol for the Cardano ecosystem. Provides native, secure communication between wallets, dApps, and protocols. |
| **PubSub Network** | The fully decentralized, SPO-operated messaging network. |

## Architecture Terms

| Term | Definition |
|------|------------|
| **DMB (Decentralized Message Bus)** | The core abstraction — a censorship-resistant transport layer for messages between wallets, dApps, and protocols. |
| **Harary Graph** | A structured network topology that ensures every node has multiple independent paths to every other node, preventing "eclipse attacks" where a malicious actor isolates a node. |
| **GossipSub** | A pub/sub protocol (from libp2p) that propagates messages by "gossiping" to random peers. Fast but probabilistic delivery. |
| **Hot Cache** | RAM-based storage for ephemeral messages with short TTLs (e.g., DeFi intents that expire in minutes). |
| **Durable DHT** | Disk-based distributed hash table for messages requiring long-term storage (e.g., governance proposals). |
| **Tiered Storage** | Architecture pattern combining Hot Cache and Durable DHT, with retention policies per topic. |

## Identity & Security

| Term | Definition |
|------|------------|
| **DID (Decentralized Identifier)** | A globally unique identifier that doesn't require a central registry. Format: `did:method:specific-id` (e.g., `did:prism:abc123`). |
| **Identus** | Hyperledger's decentralized identity platform (formerly Atala PRISM). PubSub uses Identus DIDs for all authentication. |
| **Verifiable Credential (VC)** | A cryptographically signed statement about an identity (e.g., "This DID is a Constitutional Committee member"). |
| **MLS (Messaging Layer Security)** | IETF RFC 9420 — a protocol for end-to-end encrypted group messaging with efficient key rotation. |
| **Sealed Sender** | A privacy technique where relay nodes know the destination but not the source of a message. |
| **E2EE (End-to-End Encryption)** | Messages encrypted so only the intended recipient(s) can decrypt them — relay nodes cannot read content. |

## Transaction & Intent Terms

| Term | Definition |
|------|------------|
| **Intent** | A partial, declarative transaction expressing a desired outcome (e.g., "Swap 100 ADA for USDM") without specifying execution details. |
| **SubTx (Partial Transaction)** | An incomplete Cardano transaction that requires additional inputs/signatures to be valid. Used in the Intent system. |
| **Nested Transaction** | A complete transaction that bundles multiple SubTxs together (e.g., flash loan + swap + repay). |
| **Solver / Agent** | An actor that monitors intent streams, finds profitable opportunities, and executes transactions on behalf of users. |
| **Babel Fees** | A mechanism allowing users to pay transaction fees in tokens other than ADA. |

## Network Roles

| Term | Definition |
|------|------------|
| **SPO (Stake Pool Operator)** | Entities running Cardano block-producing nodes. SPOs also run PubSub relay nodes. |
| **Publisher** | Any entity broadcasting messages to the network (wallets, dApps, protocols). |
| **Subscriber** | Any entity receiving messages from specific topics (wallets, agents, indexers). |
| **Relayer** | Nodes that propagate messages through the network without being the source or destination. |

## Use Case Identifiers

| ID | Name | Primary Focus |
|----|------|---------------|
| **DEF-01** | DeFi Intents | Low-latency intent propagation for swaps, liquidations |
| **GOV-01** | DAO Governance | Reliable delivery of proposals and vote collection |
| **AI-01** | Autonomous Agents | High-throughput M2M coordination |
| **XCB-01** | Cross-Chain Bridge | Foreign chain proof verification |
| **SOC-01** | Token-Gated Social | E2EE messaging with on-chain access control |

## Protocols & Standards

| Term | Definition |
|------|------------|
| **Ouroboros** | Cardano's proof-of-stake consensus protocol. PubSub networking is designed to be compatible with Ouroboros miniprotocols. |
| **CIP (Cardano Improvement Proposal)** | The standard process for proposing changes to Cardano. |
| **CIP-1694** | The governance framework for Cardano's Voltaire era (DReps, Constitutional Committee). |
| **CIP-30** | Cardano wallet-dApp communication standard. |
| **Protobuf (Protocol Buffers)** | Google's binary serialization format, used for efficient message encoding. |
| **CBOR** | Concise Binary Object Representation — a compact data format used in Cardano transactions. |

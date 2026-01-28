# Vision & Problem Statement

!!! info "Audience: All stakeholders"

## Mission

To provide the Cardano ecosystem with the **open-source tools and protocols** necessary to build and participate in a unified, secure, and decentralized communication network.

## Vision

**Every Cardano identity becomes a communication endpoint.**

Users receive governance proposals, DeFi alerts, and community messages through any PubSub-compatible interface — wallets, standalone clients, dashboards, or custom applications. Verified, actionable, and private. No Discord. No phishing. No middlemen.

## The Problem

The Cardano ecosystem lacks a native, unified, and secure communication layer. Today, critical information flows through:

| Current Channel | Problem |
|-----------------|---------|
| **Discord/Telegram** | Centralized, censorable, prime target for phishing |
| **Twitter/X** | Algorithm-controlled visibility, no verification |
| **Email** | Requires PII, spam-prone, not Web3-native |
| **dApp websites** | Users must actively check; no push notifications |

### Impact on Users

| Scenario | What Happens Today | With PubSub |
|----------|-------------------|-------------|
| **DeFi liquidation warning** | Posted on Discord; user misses it; loses funds | Push notification with "Add Collateral" button |
| **Governance proposal** | Announced on Twitter; buried in feed; low turnout | Verified alert with embedded "Vote" action |
| **NFT drop** | Scammer impersonates project; users get phished | Cryptographically verified from project's DID |
| **SPO maintenance** | Delegators don't know pool is down | Direct notification to delegators via their preferred client |

### The Core Problems

| Problem | Impact |
|---------|--------|
| **Ecosystem Fragmentation** | Each project builds isolated notification solutions; no interoperability |
| **Security Risks** | Critical financial information transmitted through unverified channels |
| **Incompatible Technology** | Existing solutions (XMTP, etc.) don't integrate with Cardano's network stack and identity model |
| **Stifled Innovation** | Developers can't build interactive, real-time dApps without a messaging primitive |

## The Solution

Cardano PubSub provides a **unified solution** designed specifically for the Cardano ecosystem:

### 1. Unified Standard

A single protocol for the entire Cardano ecosystem. Build once, communicate everywhere.

### 2. Modular DID-Based Identity

Every message is signed by a Decentralized Identifier (DID). The identity layer supports multiple DID methods — Identus (did:prism) for Cardano-native users, did:pkh for cross-chain wallets, and did:peer for private channels. No more "verify your wallet" phishing vectors.

```
Message from: did:prism:constitutional-committee-xyz
Verified: ✓ Constitutional Committee Member
Action: [Vote Yes] [Vote No] [Abstain]
```

### 3. SPO-Powered Infrastructure

The PubSub network runs on the same 3,000+ SPOs that secure Cardano. No new network to bootstrap.

### 4. Actionable Messages

Messages aren't just text — they're **interactive**. Vote, swap, stake, and respond directly from any PubSub client.

### 5. Privacy by Default

End-to-end encryption (MLS) for private messages. Even relay nodes can't read your content.

### 6. Phased Rollout

| Phase | What | When |
|-------|------|------|
| **Architecture & Design** | Finalize architecture, prototype | 2025 |
| **PubSub Network** | Decentralized SPO network | 2026 |
| **Full Economy** | Token incentives, DAO governance | 2027 |

## Success Criteria

| Metric | Target | Why It Matters |
|--------|--------|----------------|
| **Governance participation** | +20% voter turnout | Validates "actionable" value prop |
| **SPO adoption** | 100+ relay nodes | Demonstrates decentralization |
| **dApp integrations** | 30+ in Year 1 | Proves developer demand |
| **User reach** | 100k+ wallets | Validates product-market fit |

# Vision & Problem Statement

!!! info "Audience: All stakeholders"

## Mission

To provide the Cardano ecosystem with the **open-source tools and protocols** necessary to build and participate in a unified, secure, and decentralized communication network.

## Vision

**Every Cardano identity becomes a communication endpoint.**

Users receive governance proposals and DeFi alerts through any PubSub-compatible interface. SPOs receive authenticated emergency broadcasts and protocol updates. Agents discover intents and coordinate execution. All verified, actionable, and decentralized. No Discord. No Twitter. No middlemen.

## The Problem

The Cardano ecosystem lacks a native, unified, and secure communication layer. Today, critical information flows through fragmented, unreliable channels:

| Current Channel | Problem |
|-----------------|---------|
| **Discord/Telegram** | Centralized, censorable, prime target for phishing and social engineering |
| **Twitter/X** | Algorithm-controlled visibility, no verification, impersonation risk |
| **Email** | Requires PII, spam-prone, not Web3-native |
| **dApp websites** | Users must actively check; no push notifications |

### The Cost of the Current Gap

This isn't theoretical. The communication gap has cost the industry billions:

| Incident | Loss | Communication Failure |
|----------|------|----------------------|
| **Ronin Bridge** | $625M | 6-day detection delay — no alert system |
| **Terra/Luna** | $40B+ | Ad-hoc Telegram "War Room" coordination |
| **Solana outages** | Hours of downtime | Validators paste state into Discord |
| **Prysm bug** | 382 ETH penalties | Patch distributed via Twitter |

### Impact on Users and Operators

| Scenario | What Happens Today | With PubSub |
|----------|-------------------|-------------|
| **DeFi liquidation warning** | Posted on Discord; user misses it; loses funds | Push notification with "Add Collateral" button |
| **Governance proposal** | Announced on Twitter; buried in feed; low turnout | Verified alert with embedded "Vote" action |
| **Critical node bug** | Tweeted by client team; operators asleep miss it | Authenticated alert to validator software |
| **SPO maintenance** | Delegators don't know pool is down | Direct notification to delegators |
| **Chain halt** | Validators coordinate via Discord chat | Signed state reports and restart instructions |

### The Core Problems

| Problem | Impact |
|---------|--------|
| **Ecosystem Fragmentation** | Each project builds isolated notification solutions; no interoperability |
| **No Authentication** | Critical alerts flow through channels where anyone can impersonate anyone |
| **No Guaranteed Delivery** | Operators may be offline, not monitoring the right channel |
| **Stifled Innovation** | Developers can't build interactive, real-time dApps without a messaging primitive |

## The Solution

Cardano PubSub provides a **unified solution** designed specifically for the Cardano ecosystem:

### 1. Unified Standard

A single protocol for the entire Cardano ecosystem. Build once, communicate everywhere.

### 2. Authenticated Messages

Every message is signed by a Decentralized Identifier (DID). Emergency alerts from protocol authorities are cryptographically verified. No more "verify your wallet" phishing vectors.

```
Message from: did:prism:iog-security-council
Verified: ✓ IOG Security Council
Severity: CRITICAL
Action: [Apply Patch] [View Details]
```

### 3. SPO-Powered Infrastructure

The PubSub network runs on the same 3,000+ SPOs that secure Cardano. No new network to bootstrap. The Mithril network already demonstrates this model — 250 SPOs participating in coordinated operations.

### 4. Actionable Messages

Messages aren't just text — they're **interactive**. Vote, swap, stake, and respond directly from any PubSub client.

### 5. Works When It Matters

PubSub operates independently of the main chain. During a chain halt — exactly when coordination is most critical — the messaging layer continues functioning.

### 6. Phased Rollout

| Phase | What | When |
|-------|------|------|
| **Architecture & Design** | Finalize architecture, prototype | 2025 |
| **PubSub Network** | Decentralized SPO network | 2026 |
| **Full Economy** | Relay incentives, ecosystem integrations | 2027 |

## Why Now

Three factors make this the right moment for PubSub:

### 1. DeFi Intents Initiative

The DeFi Intents initiative is actively building gasless trading for Cardano. **PubSub is the designated message bus** — without it, users have no way to broadcast intents to agents.

### 2. Nested Transactions (CIP-118)

Nested Transactions land in 2026, enabling Babel Fees. This unlocks ADA-free transactions where agents cover user fees. But gasless trading requires users to broadcast intents somewhere — that's PubSub.

### 3. Competitive Window

XMTP (despite $50M funding) won't have decentralized mainnet until 2026. Anoma is still in testnet. The window is 12-18 months before major competitors are production-ready. Cardano has a structural advantage — 3,000+ SPOs already running infrastructure — that competitors would need years to replicate.

---

## Success Criteria

| Metric | Target | Why It Matters |
|--------|--------|----------------|
| **Emergency response time** | <5 min vs. hours today | Validates "authenticated alerts" value prop |
| **Governance participation** | +20% voter turnout | Validates "actionable messages" value prop |
| **SPO adoption** | 100+ relay nodes | Demonstrates decentralization |
| **dApp integrations** | 30+ in Year 1 | Proves developer demand |
| **User reach** | 100k+ wallets | Validates product-market fit |

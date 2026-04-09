# The Case for Cardano PubSub

!!! info "Audience: Community, Stakeholders, Decision Makers"

---

## The Problem

Cardano has world-class settlement infrastructure — Plutus, Hydra, Mithril, on-chain governance. Yet when something urgent happens, we coordinate on Discord.

Protocol upgrades get announced on Twitter. Emergency patches spread through Telegram. SPOs check five platforms hoping they didn't miss anything.

The industry has lost over a billion dollars to communication failures. The Ronin hack went undetected for six days. Terra validators coordinated through leaked Telegram chats. Solana restarts involve pasting ledger heights into Discord. Not consensus failures — just people not getting the message in time.

---

## The Gap

Other ecosystems solved this years ago.

| Ecosystem | Messaging Layer | Adoption |
|-----------|-----------------|----------|
| **Ethereum** | XMTP, Push Protocol | 2M+ identities, 100M+ notifications delivered |
| **Solana** | Dialect | 1M+ daily active users |
| **Cardano** | ❌ None | Zero |

It's not that our technology is worse. We haven't built the connective tissue that turns capabilities into usable products.

---

## Why Now

**Babel Fees are coming.** CIP-118 ships in 2026, enabling users to pay fees in tokens other than ADA. But Babel Fees require coordination — users broadcast intents, agents fulfill them. Without a messaging layer, there's no way for them to find each other. The capability exists but nobody can use it.

**Governance needs it.** CIP-1694 gives us on-chain voting, but proposals get buried in forum posts and tweets. Turnout suffers because people don't see them in time or forget to vote. Verified notifications delivered directly to wallets — with vote buttons built in — would transform participation.

---

## What PubSub Is

PubSub is the foundational messaging layer — the infrastructure that everything else builds on.

Publish messages to a topic, subscribe to receive them. Users publish intents; agents receive them. The Constitutional Committee publishes proposals; wallets receive them. Security teams publish alerts; SPO nodes receive them.

Five core use cases:

| Use Case | What It Enables |
|----------|-----------------|
| **DeFi Intents** | Users broadcast trading intents; agents fulfill them, enabling Babel Fees |
| **Governance** | Verified proposals with one-click voting, delivered directly to wallets |
| **Network Operations** | Authenticated emergency alerts with guaranteed delivery to SPOs |
| **Cross-Chain** | Coordination layer for bridge protocols and multi-chain messaging |
| **Agent Coordination** | High-throughput communication for automated systems — liquidation bots, order solvers, price oracles |

PubSub is the foundation. The DeFi Intents team builds intent schemas on top. Governance tooling builds voting flows on top. Every application benefits from one shared, reliable, decentralized transport layer.

---

## Why Native

- **Operated by SPOs** — Decentralized from day one. No dependency on a single company that could change priorities, raise prices, or shut down.
- **Integrated with Cardano identity** — Wallets work seamlessly. No bridges to external identity systems.
- **Governed on-chain** — Topic administration is transparent and auditable, using the same mechanisms as the rest of Cardano.
- **Economically aligned** — Relay fees create a new revenue stream for SPOs, strengthening the operator network that secures Cardano.

---

## Cardano's Advantage

Other ecosystems had to bootstrap operator networks from scratch. Cardano already has one.

**3,000 SPOs** already run infrastructure businesses, compete for delegation, and have economic stake in the ecosystem. Running messaging relays fits their existing model.

**Mithril proves it works.** 250 SPOs already participate in coordinated threshold signatures. SPO-operated infrastructure beyond block production is proven.

**The research is done.** Athens University designed the protocols — SecureCyclon, Vicinity, Hybrid Dissemination. Peer-reviewed, Cardano-specific, ready for implementation.

We're not starting from scratch. We're assembling pieces that already exist.

---

## What We Get

**Frictionless onboarding.** New user has USDC, wants ADA. Today: stuck without ADA for fees. With PubSub + Babel Fees: broadcast intent, agent fulfills it, done.

**Better governance.** Constitutional Committee proposal hits your wallet with a "Vote Now" button. No checking forums. No missing deadlines. Participation goes from single digits to meaningful turnout.

**Faster emergency response.** Signed alerts propagate in seconds, not hours. Validator software can respond automatically.

---

## 2030 Alignment

PubSub maps directly to Cardano's 2030 strategic KPIs:

| KPI | Current | 2030 Target | How PubSub Contributes |
|-----|---------|-------------|------------------------|
| **TVL** | $200M | $3B | Enables intent-based DeFi, Babel Fees, cross-chain bridges — the infrastructure that grows TVL |
| **Transactions** | ~1M/month | 27M/month | Every fulfilled intent settles on-chain. Agent competition increases settlement activity |
| **Users** | 100K-300K | Growth | Removes barriers — users don't need ADA to start, don't need to understand UTXOs |

**Pillar alignment:** Core Infrastructure, DeFi Enablement, Governance Tooling

---

## Assumptions & Dependencies

**Dependencies:**

- **CIP-118 Nested Transactions** (Ledger Team) — Required for full DeFi Intents flow. Users sign partial transactions; agents complete them. Without CIP-118, Babel Fees can't work as designed.
- **DeFi Intents Initiative** — Defines intent schemas and agent layer. PubSub is the transport; they build the application logic on top.

**Assumptions:**

- SPOs will operate relays when economically incentivized (Mithril adoption suggests yes)
- Agent ecosystem emerges organically once infrastructure exists (we'll seed with reference implementations)
- Wallet integrations follow standardized messaging APIs

**What works independently:**

PubSub delivers value even if dependencies slip. Governance notifications, emergency alerts, and agent coordination don't require CIP-118. The DeFi Intents use case specifically depends on it — others don't.

---

## What It Takes

12 months. Under $1M. Small team building on existing research and operator network.

That's what unlocks Cardano's 2026 roadmap — turning capabilities into products.

---

## The Decision

Every serious ecosystem has a coordination layer. Cardano doesn't — yet.

We have the research. We have the operators. We have the identity infrastructure. We have the use cases waiting.

The settlement layer is ready. The 2026 roadmap is ready. The missing piece is the infrastructure that connects them to users.

Let's build it.

---

*[Architecture](../architecture/index.md) | [Use Cases](../use-cases/index.md) | [Proposal CBU018](https://github.com/input-output-hk/pubsub/tree/main/proposal)*

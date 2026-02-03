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

| Ecosystem | Messaging Layer | Status |
|-----------|-----------------|--------|
| **Ethereum** | XMTP, Push Protocol | 40M+ smart accounts, 100M+ notifications |
| **Solana** | Dialect | 1M+ daily active users |
| **Cardano** | ❌ None | Zero |

It's not that our technology is worse. We haven't built the connective tissue that turns capabilities into usable products.

---

## Why Now

**Babel Fees are coming.** CIP-118 ships in 2026, enabling users to pay fees in tokens other than ADA. But Babel Fees require coordination — users broadcast intents, agents fulfill them. Without a messaging layer, there's no way for them to find each other. The capability exists but nobody can use it.

**Governance needs it.** CIP-1694 gives us on-chain voting, but turnout depends on people knowing there's something to vote on. Verified notifications with one-click voting would transform participation.

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
| **Agent Coordination** | High-throughput communication for keepers, solvers, and automated systems |

PubSub doesn't define message contents or handle application logic. The DeFi Intents team builds intent schemas. Governance tooling builds voting flows. PubSub just moves messages — reliably, quickly, without centralized intermediaries.

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

**Better governance.** Proposal notifications delivered directly to wallets with vote buttons. Participation up, friction down.

**Faster emergency response.** Signed alerts propagate in seconds, not hours. Validator software can respond automatically.

---

## What It Takes

12 months. Under $1M. Small team building on existing research and operator network.

That's what unlocks Cardano's 2026 roadmap — turning capabilities into products.

---

## The Question

It's not whether Cardano needs a coordination layer. Every serious ecosystem has one.

We have the research. We have the operators. We have the identity infrastructure. We have the use cases.

Cardano's settlement layer is world-class. Time to build the coordination layer to match.

---

*[Architecture](../architecture/index.md) | [Use Cases](../use-cases/index.md) | [Proposal CBU018](https://github.com/input-output-hk/pubsub/tree/main/proposal)*

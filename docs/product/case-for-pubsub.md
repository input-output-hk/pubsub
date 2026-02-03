# The Case for Cardano PubSub

!!! info "Audience: Community, Stakeholders, Decision Makers"

---

## The Uncomfortable Truth

Here's something we don't talk about enough: Cardano has some of the most sophisticated blockchain infrastructure in the world — Plutus, Hydra, Mithril, formal verification, on-chain governance — and yet, when something urgent happens, we coordinate on Discord.

Think about that for a moment. We've built a system designed to operate without trusted intermediaries, and then we rely on a gaming chat app owned by a single company to communicate critical information. Protocol upgrades get announced on Twitter. Emergency patches spread through Telegram. SPOs find out about incidents by checking five different platforms and hoping they didn't miss anything.

This isn't a Cardano-specific failing. The Ronin Bridge hack went undetected for six days because there was no alert system — just silence. When Terra collapsed, validators coordinated through leaked Telegram "war room" chats. Solana restarts involve validators literally pasting ledger heights into Discord and waiting for someone to compile them. The Prysm bug on Ethereum was patched via Twitter; operators who were asleep woke up to slashing penalties.

The industry has collectively lost over a billion dollars to communication failures. Not smart contract bugs. Not consensus failures. Just... people not getting the message in time.

---

## What Other Ecosystems Did About It

Ethereum figured this out a few years ago. They built XMTP for wallet-to-wallet messaging. Push Protocol for notifications. CoW Protocol for trading intents. Today, they have 40 million smart accounts, $87 billion in intent-based trading volume, and over 100 million notifications delivered through native infrastructure.

Solana built Dialect. Base integrated XMTP. These aren't side projects — they're core infrastructure that makes everything else work.

Cardano has nothing equivalent. Zero.

| Ecosystem | Intent Infrastructure | Messaging Layer | Status |
|-----------|----------------------|-----------------|--------|
| **Ethereum** | ERC-4337 / CoW Protocol | XMTP, Waku, Push | 40M+ smart accounts; $87B CoW volume |
| **Solana** | Jito (97% validator adoption) | Dialect | $2.9B staked; 1M+ DAU on Dialect |
| **Base** | Smart Wallet | XMTP / Push | Leader in transfer volume |
| **Cardano** | ❌ None | ❌ None | **Zero** |

When analysts compare ecosystems, they're increasingly pointing this out. It's not that our technology is worse — in many ways it's better. It's that we haven't built the connective tissue that turns capabilities into usable products.

---

## Why This Matters Right Now

In 2026, Cardano is shipping Nested Transactions through CIP-118. This enables Babel Fees — the ability for users to pay transaction fees in tokens other than ADA. It's a big deal. A user holding only USDC could swap it for ADA without needing ADA to pay the fee. The "invisible gas" experience that makes onboarding frictionless.

But here's the problem: Babel Fees require coordination. A user needs to broadcast their intent — "I want to swap this for that" — and an agent needs to receive it, decide to fulfill it, and complete the transaction. Without a way for users and agents to find each other, Babel Fees are a capability that nobody can use.

It's like building a world-class postal service but forgetting to create addresses. The infrastructure exists, but there's no way to direct anything to anyone.

The same is true for governance. CIP-1694 gives us on-chain voting, DReps, and constitutional governance. But voter turnout depends on people knowing there's something to vote on. Right now, that means hoping they see the right tweet or check the right forum at the right time. Verified notifications delivered directly to wallets — with a button to vote right there — would transform participation. But we don't have that.

---

## What PubSub Actually Is

Cardano PubSub is a messaging layer. Nothing more, nothing less.

It lets anyone publish messages to a topic, and anyone else subscribe to receive them. A user publishes an intent; agents subscribed to that topic receive it. The Constitutional Committee publishes a proposal notification; wallets subscribed to governance receive it. A security team publishes an emergency alert; SPO nodes receive it.

PubSub doesn't define what those messages contain or what happens when they're received. It just moves them — reliably, quickly, and without centralized intermediaries.

The DeFi Intents team defines intent schemas. Governance tooling defines voting flows. Wallet teams build the UX. PubSub is the transport layer they all use.

Think of it like TCP/IP for Cardano applications. TCP/IP doesn't know anything about websites or email — it just delivers packets. HTTP and SMTP are built on top. Similarly, PubSub doesn't know anything about intents or governance — it just delivers messages. The application logic is built on top.

### What PubSub Provides

- **Publish/subscribe messaging** — Anyone can create a topic, publish to it, or subscribe
- **Reliable delivery** — Messages reach subscribers even under network failures
- **Persistence** — Offline subscribers can catch up on missed messages
- **Authentication** — Messages are signed; subscribers can verify the source
- **Decentralized operation** — Run by SPOs, not a single company

### What PubSub Does NOT Provide

- Intent schemas or DeFi logic (that's the DeFi Intents project)
- Governance voting flows (that's governance tooling)
- Wallet UX (that's wallet teams)
- Babel Fees integration (that's CIP-118 + DeFi Intents)

PubSub is infrastructure. It enables these applications but doesn't implement them.

---

## Why Native Infrastructure

Cardano's messaging infrastructure should be:

- **Operated by Cardano SPOs** — Three thousand independent operators with economic stake in Cardano's success
- **Integrated with Cardano identity** — Native Identus (did:prism) support, seamless wallet experience
- **Governed on Cardano** — Topic administration through on-chain smart contracts, transparent and auditable
- **Economically aligned with Cardano** — Relay fees for SPOs, strengthening the operator network
- **Independent** — Our roadmap, our priorities, our governance

This is what native gives us.

---

## Cardano Has Something Special

Here's what makes this interesting: Cardano has an advantage that other ecosystems didn't have when they built their coordination layers.

We have 3,000 stake pool operators.

Building a decentralized messaging network usually means bootstrapping an operator network from scratch. It's hard. You need to find people willing to run infrastructure, design incentives for them to participate, and build trust over years.

Cardano already did that. SPOs aren't just validators — they're infrastructure businesses. They compete for delegation by offering services, maintaining uptime, and building reputation. They have the technical expertise, the operational discipline, and the economic stake in the ecosystem's success.

Running a messaging relay is a natural extension of what SPOs already do. It's not asking them to do something foreign — it's giving them another way to add value and differentiate. Some will offer it as a free service to attract delegators. Others might charge relay fees. The economic model writes itself because it fits the existing incentive structure.

We've already seen this work. Mithril has 250 SPOs participating in coordinated threshold signatures. The pattern of SPOs running additional infrastructure beyond block production is proven.

### The Research Is Done

We're also not starting from a blank page technically. IOG commissioned Athens University to design the protocols for this. They produced a complete framework: SecureCyclon for peer sampling, Vicinity for topic routing, Hybrid Dissemination for reliable message delivery. It's peer-reviewed research specifically designed for Cardano's architecture.

Building native doesn't mean reinventing everything. It means implementing validated research using an operator network we already have.

### Assembling Existing Pieces

Other ecosystems had to build messaging infrastructure AND bootstrap an operator network AND create an economic model AND solve identity.

Cardano only needs to build the messaging infrastructure. The operators, the economic model, and the identity layer already exist. We're assembling pieces that are already there.

---

## What We Actually Get

If we build this, a few things become possible that aren't possible today.

A new user downloads a Cardano wallet. They have USDC from an exchange. They want to swap some for ADA. Today, they can't — they need ADA to pay the transaction fee, but they don't have any ADA because that's what they're trying to get. With PubSub and Babel Fees, they broadcast an intent, an agent picks it up, and the swap happens. The agent takes a small spread; the user gets their ADA without ever hitting that barrier. The "how do I get started" problem goes away.

A governance proposal comes up for vote. Today, it gets posted on a forum, tweeted about, maybe discussed on Discord. Turnout is low because people don't see it or forget about it. With PubSub, every wallet subscribed to governance notifications receives the proposal directly — verified, from the actual Constitutional Committee, with buttons to vote right there. Participation goes up because friction goes down.

A critical bug is found in node software. Today, the team tweets about it and hopes everyone sees it in time. Some operators are asleep; some don't check Twitter; some mistake it for a scam. With PubSub, the alert goes out signed by a registered authority, propagates through the SPO network in seconds, and validator software can even respond automatically — entering safe mode while the operator reviews. Response time drops from hours to seconds.

These aren't hypothetical benefits. They're what other ecosystems get from their coordination infrastructure. We're just catching up.

---

## The Opportunity Window

There's also a timing element. If Cardano builds now, we can have native, SPO-operated coordination infrastructure that's decentralized from day one — because we're not bootstrapping a new operator network. We're using the one we've spent years building.

---

## What It Takes

This isn't a moonshot. It's a 12-month project with well-understood requirements.

We need a small team — two engineers ramping up to four — building on the Athens University research. We need coordination with the wallet teams to integrate the SDK. We need SPO outreach to explain the opportunity and onboard early operators.

The total cost is under a million dollars. That's a rounding error compared to what Cardano has invested in the settlement layer. And it's what unlocks that investment — turning capabilities into products people can actually use.

---

## The Real Question

The question isn't whether Cardano needs a coordination layer. Every serious ecosystem has one.

We have the research. We have the operators. We have the identity infrastructure. We have the use cases waiting. What we need is the decision to build.

Cardano's settlement layer is world-class. It's time to build the coordination layer to match.

---

*For technical details, see [Architecture](../architecture/index.md) and [Use Cases](../use-cases/index.md).*

*For the formal proposal, see [Intersect Proposal CBU018](https://github.com/input-output-hk/pubsub/tree/main/proposal).*

# Actor & Use Case Analysis

**Date:** April 2026
**Status:** Draft — inferences from existing documentation; claims marked ⚠️ require validation with real actors

## Purpose

This document applies a structured framework to each candidate use case (ideally)  before any protocol design work is finished. The premise is that the right technical solution can only be chosen once the communication problem, the actors involved, and the demand reality are clearly understood.

The original five use cases in the project documentation were organized as architectural stress tests (what pushes the design the hardest?) rather than as real actor scenarios. This document reorganizes them into **actor-pair scenarios** — who is trying to communicate with whom, and why.

**Framework questions applied to each scenario:**

1. What communication problem is being solved? What are actors trying to communicate?
2. Who are the senders and recipients?
3. Where is the value created in the communication?
4. What is the size of the actor sets?
5. Would any actor be willing to pay?
6. What is the communication pattern? (broadcast/unicast, push/pull, sync/async, message expiry)
7. What delivery guarantees are actually needed?
8. What is the recipient connectivity profile? (always-online infrastructure vs. mobile/browser)
9. What do actors use today, and what specifically fails?

---

## UC-1 · Protocol Developer Teams → SPOs: Emergency Alerts & Operational Coordination

**Maps to original use case:** Network Operations

### 1. Communication problem

Protocol developer teams (IOG, Intersect, CF) need to reach all active SPOs quickly during emergencies: critical node bugs, security vulnerabilities, chain halt coordination, upgrade deadlines. Today this happens via Discord, Telegram, and Twitter — channels that are unauthenticated, fragmented across platforms, and require SPOs to be actively monitoring.

### 2. Senders and recipients

- **Senders:** ~5–10 recognized entities: node development teams, the Intersect Security Council, possibly the Constitutional Committee for governance-adjacent network decisions. A small, stable, well-known set.
- **Recipients:** ~3,000 active SPO pools. In practice, a pool may be operated by an individual or a small team. The actual number of human operators receiving messages is probably closer to ~2,000–3,000.

### 3. Value created

- **For senders:** Reaching SPOs quickly minimizes incident blast radius. A critical bug that takes 6 days to propagate (Ronin) vs. 6 minutes is a qualitative difference in network resilience.
- **For SPOs:** Their business (block production, delegator rewards) depends on running correct, up-to-date software. Missing an emergency alert is directly costly.
- **For the ecosystem:** Network reliability and security are public goods. Faster incident response protects delegators, DApps, and users who depend on chain liveness.

### 4. Actor set sizes

| Actor | Estimated Count |
|---|---|
| Authorized senders | ~5–10 entities |
| SPO pools (recipients) | ~3,000 |
| Human operators behind pools | ~2,000–3,000 |

The recipient set is small relative to most notification systems. This is not a "millions of users" scaling problem.

### 5. Willingness to pay

- **Senders:** Maybe — these are organizations with operational budgets, and the cost of not reaching SPOs reliably is high. ⚠️ *Not validated.*
- **SPOs:** Mixed — SPOs already express reluctance to run additional infrastructure without compensation, but they have a direct business interest in receiving alerts. A model where senders fund delivery (rather than asking SPOs to pay) is more realistic. ⚠️ *Not validated.*

We can predict that the number of expected messages is low. Even if senders pay, it won't generate any relevant revenue flow.

### 6. Communication pattern

- **Pattern:** One-to-many broadcast; push; asynchronous.
- **Urgency tiers:** Critical alerts (patch now, stop block production) vs. routine coordination (upgrade in 2 weeks). Different urgency levels warrant different delivery SLAs.
- **Message expiry:** Emergency messages remain relevant for hours to days. Routine upgrade announcements may be relevant for weeks.
- **No response required from recipients** (for the notification itself — response is local action taken by the SPO).

### 7. Delivery requirements

- **For critical alerts:** As close to reliable as possible. Missing a critical security alert has direct financial consequences for SPOs and the network.
- **For routine updates:** Best-effort acceptable with some persistence for catch-up (SPOs that were briefly offline).
- Ordering is not strictly required — each alert is independent.

### 8. Recipient connectivity profile

**Favorable:** SPOs are infrastructure operators running always-on servers. They have stable connectivity and existing tooling for monitoring processes. This is the most straightforward delivery target in the entire set of use cases — no light-client problem, no mobile battery constraints, no intermediary wallet provider required.

### 9. Current alternatives and gaps

| Current method | Specific gap |
|---|---|
| Discord `#announcements` | Unauthenticated — no way to verify message is genuinely from IOG vs. impersonation. SPOs must actively monitor channel. |
| Telegram groups | Same authentication problem. Fragmented (different groups for different things). |
| Twitter / X | Unauthenticated, can be spoofed, algorithmic filtering, no delivery guarantee. |

**The specific gap:** There is no channel that is (a) push, (b) authenticated against an on-chain identity, (c) delivered to all SPOs without requiring them to monitor a specific platform, and (d) machine-readable for automated response.

### Open validation questions

- ⚠️ Would IOG/Intersect actually publish alerts through a new system, or would they continue using existing channels even if a new one existed?
- ⚠️ What fraction of SPOs would opt in to running a communication software alongside their Cardano node?
- ⚠️ Is the authentication gap the most-cited pain point from SPOs, or is it something else?
- ⚠️ What is the current incident communication process at IOG/Intersect? Where are the documented failures?

---

## UC-2 · SPOs → Delegators: Operational Announcements

### 1. Communication problem

SPOs want to inform their delegators about operational matters that affect delegation decisions: fee and margin changes, planned maintenance windows, pool retirement notices, mission/identity updates. Delegators currently have no reliable way to receive these without actively seeking them out.

### 2. Senders and recipients

- **Senders:** ~3,000 SPO pools. In practice, the fraction that would actively publish messages is uncertain — probably the subset of pools with mission-driven or community-oriented identities. ⚠️ *Unknown fraction.*
- **Recipients:** Each pool's delegators. A pool might have anywhere from tens to tens of thousands of delegators. Cardano has hundred of thousands active delegators in total, but distributed across pools. Most delegators interact via light wallets, meaning that wallets and infrastructure providers like Blockfrost would need to adopt the solution.

### 3. Value created

- **For SPOs:** Retention and trust-building. Proactively informing delegators of fee changes or maintenance signals professionalism. For mission-driven pools, direct communication is part of their identity proposition.
- **For delegators:** Better informed decision-making about where to delegate. A fee increase without notice is a worse delegator experience than one with advance notice.
- **For wallet infrastructure providers** (Blockfrost, wallet backends): A notification layer increases the value of their services to both DApps and end users, making it a worthwhile integration for providers already operating the subscriber infrastructure.
- **Ecosystem value:** SPO accountability and transparency are governance-adjacent goods.

### 4. Actor set sizes

| Actor | Estimated Count |
|---|---|
| Active publishing SPOs | Unknown — could be dozens to hundreds ⚠️ |
| Total delegators (all pools) | Hundreds of thousands |
| Delegators per typical pool | Tens to tens of thousands |
| Wallet infrastructure providers | ~10 major backends |

### 5. Willingness to pay

- **SPOs:** Possibly small amounts for important announcements (retirement notice, major fee change). Unlikely to pay for routine posts. ⚠️ *Not validated.*
- **Delegators:** No — receiving notifications is a passive benefit.

### 6. Communication pattern

- **Pattern:** One-to-many broadcast from pool to its own delegators; push; asynchronous.
- **Urgency:** Low to medium. A fee change notice with 2 weeks of lead time is the typical case.
- **Message expiry:** Relatively long — days to weeks.
- **Topic scope:** Messages are relevant only to a specific pool's delegators, not to the entire network. Topic namespacing by pool ID is essential.

### 7. Delivery requirements

- Best-effort acceptable for most announcements. Delegators who miss a routine update and later see the change on-chain can still adapt.
- Higher reliability desired for irreversible events (retirement notices), but the cost of missing one is manageable (delegators can see the event on-chain).

### 8. Recipient connectivity profile

**Challenging.** Delegators are predominantly end users on light mobile/browser wallets (Eternl, Lace, Yoroi, etc.). They are not running always-on infrastructure. This means:

- Direct protocol participation is not possible for delegators.
- Delivery requires wallet backend providers to intermediate — they subscribe to pool topics on behalf of their users and forward via conventional push notification infrastructure.
- The effectiveness of this use case is entirely dependent on wallet provider adoption.

### 9. Current alternatives and gaps

| Current method | Specific gap |
|---|---|
| On-chain pool metadata | Static — contains homepage URL, not push notifications. |
| Twitter / social media | Requires delegators to follow specific pool accounts; no wallet integration. |
| Pool-specific Discord/Telegram | Requires delegators to join; low discoverability. |
| Pool website / newsletter | Requires explicit email subscription; not tied to wallet identity. |

**The gap:** There is no standardized push channel from a pool to its delegators that is integrated into the delegator's wallet experience. On-chain identity (delegation relationship) is known but not used for communication.

### Open validation questions

- ⚠️ Do SPOs feel they have a communication problem with their delegators? What do they currently do?
- ⚠️ Would wallet providers (Blockfrost, Eternl, Lace) integrate pool notification delivery into their backends?
- ⚠️ What types of SPO announcements would delegators actually want to receive? Are fee changes the primary case?
- ⚠️ minor note: Is the relevant on-chain relationship the current delegation, or should past delegators also receive messages?

---

## UC-3 · Governance Bodies → Community: Proposal Notifications & Voting Alerts

**Maps to original use case:** Governance (notification sub-case)

### 1. Communication problem

The Constitutional Committee, DRep groups, and protocol teams need to reach ADA holders and DReps with governance-related notifications: new proposals requiring attention, voting deadline reminders, governance status updates. Currently, governance information is announced on social media and discussed in fragmented channels — users who aren't actively following miss events or vote at low rates.

*Scope note: This use case covers the **notification** function only. Vote submission and aggregation belong on-chain (CIP-1694) and should not go through PubSub.*

### 2. Senders and recipients

- **Senders:** Constitutional Committee (~7 members), DRep coalitions or individual DReps, protocol teams (for upgrade-adjacent governance). A small, credentialed set for high-stakes notifications; a larger set for informational content.
- **Recipients:** ADA holders eligible to vote (hundreds of thousands), DReps (~thousands, growing), ultimately mediated through wallet providers (~10 major backends).

### 3. Value created

- **For governance bodies:** Increasing informed participation rates. Low voter turnout is a documented problem in on-chain governance. Proactive notification increases participation.
- **For voters:** Convenience — knowing about proposals without active monitoring. Time-limited votes are easy to miss.
- **Ecosystem value:** Governance legitimacy depends on participation. Higher turnout in representative governance produces more legitimate outcomes.

### 4. Actor set sizes

| Actor | Estimated Count |
|---|---|
| Constitutional Committee | ~7 |
| Active DReps | Hundreds (growing) |
| ADA holders with delegation | hundred of thousands |
| Major wallet backends | ~10 |

### 5. Willingness to pay

- **Governance bodies:** Likely yes for formal institutions (CC, Intersect) — governance communication is part of their mandate and funded. Individual DReps less so. ⚠️ *Not validated.*
- **Recipients:** No.

### 6. Communication pattern

- **Pattern:** One-to-many broadcast; push; asynchronous.
- **Urgency:** Medium — voting deadlines are time-bound (days to weeks).
- **Message expiry:** Tied to the voting period. A notification about a proposal is irrelevant after the vote closes.
- Content is informational: proposal title, summary, deadline, link to full text on IPFS/chain.

### 7. Delivery requirements

- Reasonably reliable — missing governance notifications reduces participation. However, this is not emergency-level; a best-effort system with persistence is acceptable.
- Ordering is not critical — each proposal notification is independent.

### 8. Recipient connectivity profile

**Dual-layer.** DReps are likely more connected (running governance dashboards, using governance DApps actively). General ADA holders are end users on light wallets, requiring wallet backend intermediation.

The most practical delivery path: governance notifications → wallet backends subscribe → filter by relevance per user → push via conventional mobile/web notification infrastructure.

### 9. Current alternatives and gaps

| Current method | Specific gap |
|---|---|
| Cardano governance websites (gov.tools etc.) | Pull-based — users must check actively. |
| Twitter / Discord announcements | Unauthenticated; not integrated with wallet. |
| On-chain events | Machine-readable but no user-facing push. |
| Email (Intersect newsletters) | Not tied to on-chain identity; requires explicit subscription separate from wallet. |

**The gap:** No mechanism connects an on-chain governance event to a push notification in the user's wallet, authenticated as coming from a recognized governance body.

### Open validation questions

- ⚠️ Would wallet teams (Lace, Eternl, Yoroi) prioritize governance notification integration? What is their current plan for surfacing governance events?
- ⚠️ Do governance bodies (Intersect, CC) have a channel strategy beyond social media? Do they identify fragmented notification as a problem?
- ⚠️ Is the user-facing notification the bottleneck for participation, or is it the UX of the voting flow itself?

---

## UC-4 · DReps → Delegators: Voting Intent & Accountability

**Maps to original use case:** Governance (sub-case)

### 1. Communication problem

DReps represent ADA holders and make governance decisions on their behalf. Delegators want to know how their DRep intends to vote on proposals (before the vote) and how they voted (after). This is an accountability channel — delegators who disagree can redelegate. Today there is no standardized mechanism for DReps to communicate their voting rationale to their constituency.

### 2. Senders and recipients

- **Senders:** Individual DReps. Currently hundreds of registered DReps; the number expected to actively publish policy communications is unknown.
- **Recipients:** Each DRep's delegators — a subset of ADA holders. Delegation relationships are known on-chain.

### 3. Value created

- **For DReps:** Trust, accountability demonstration, delegator retention.
- **For delegators:** Informed re-delegation decisions. The governance design assumes delegators can hold DReps accountable — communication is a prerequisite.
- **Ecosystem value:** Functioning representative governance.

### 4. Actor set sizes

| Actor | Estimated Count |
|---|---|
| Active DReps publishing messages | Unknown — likely tens to low hundreds ⚠️ |
| Delegators per DRep | Varies widely — tens to tens of thousands |

### 5. Willingness to pay

- **DReps:** Likely low. This is a professional communication cost similar to a newsletter. May accept small infrastructure costs but unlikely to pay per-message fees. ⚠️ *Not validated.*

### 6. Communication pattern

- **Pattern:** One-to-many within constituency; push; asynchronous.
- **Urgency:** Low — voting intent announcements are made ahead of deadlines.
- **Message expiry:** Tied to governance epoch / proposal voting period.

### 7. Delivery requirements

Best-effort acceptable. Delegators can monitor DRep voting history on-chain; PubSub adds convenience, not criticality.

### 8. Recipient connectivity profile

End-user delegators on light wallets — identical to UC-3. Wallet backend intermediation required.

### 9. Current alternatives and gaps

DRep social media accounts, governance platform profiles, Cardano forum posts. No standardized wallet-integrated channel from a DRep to their specific delegators.

### Open validation questions

- ⚠️ Do active DReps feel they have a communication problem with their delegators? What do they use today?
- ⚠️ Would delegators act differently (redelegate more actively) if they received DRep communications in-wallet?
- ⚠️ Is this distinct enough from general governance notifications (UC-3) to justify separate infrastructure, or could DRep communications be served by the same channel?

---

## UC-5 · DApps → Users: Position Alerts & Protocol Notifications

### 1. Communication problem

DApp protocols need to reach users who have active positions — particularly for time-sensitive events: collateral at risk (lending protocols), contract migrations, DAO votes affecting the protocol. Today, DApps can't push to users who interacted via a wallet without requiring them to follow a social account.

### 2. Senders and recipients

- **Senders:** DApp teams (lending protocols, DEXs, bridging protocols).
- **Recipients:** Users who hold positions in the DApp — hundreds to thousands per protocol. Known on-chain (contract interactions), unknown off-chain (no email, no phone number).

### 3. Value created

- **For DApps:** increase user trust and protect TVL. Migration notices and DAO vote notifications reduce support burden and increase governance participation.
- **For users:** Direct financial protection from at-risk collateral alerts. Awareness of protocol governance votes that affect their positions.
- **Ecosystem value:** Better DeFi UX; reduced user loss from passive positions.

### 4. Actor set sizes

| Actor | Estimated Count |
|---|---|
| DApps with notification need | Tens (the relevant Cardano DApp ecosystem is small) |
| Users per DApp with active positions | Hundreds to thousands |
| Wallet infrastructure providers | ~10, acting as intermediaries between the network and end users |

### 5. Willingness to pay

- **DApps:** Potentially yes — especially for high-value notifications (liquidation warnings, new liquidity pool pair). ⚠️ *Not validated but plausible.*
- **Users:** No — this is a service DApps provide to users.

### 6. Communication pattern

- **Pattern:** One-to-one or one-to-few, targeted by recipient wallet address; push; asynchronous.
- **Important distinction:** Unlike governance and network operations (pure broadcast), this use case requires **targeted delivery by recipient address** — not all subscribers receive the same message. A collateral alert is relevant only to the wallet holding that position.
- **Intermediary actor:** In practice, end users interact via light wallets and cannot directly subscribe to the protocol. Wallet infrastructure providers receive messages on behalf of their users and are responsible for per-user forwarding via their own notification infrastructure. This makes wallet providers an active participant in this use case, not merely a passive relay.
- **Urgency:** High for financial risk alerts; lower for governance votes and migration notices.
- **Message expiry:** Short for financial alerts (minutes to hours); longer for governance and migration content (days).

### 7. Delivery requirements

- **Financial alerts:** High reliability strongly preferred. A missed collateral warning can mean a preventable liquidation.
- **Governance and protocol notifications:** Best-effort acceptable.

### 8. Recipient connectivity profile

End users on light wallets — mediated through wallet backends. Messages may be **targeted** (specific to a user address), which creates a delivery challenge not present in broadcast use cases. Two approaches are possible:

- **(a) Server-side routing:** Wallet backends maintain per-user subscription state and route only the relevant messages to each user. More complex to implement but cleaner from the user's perspective.
- **(b) Client-side filtering:** The wallet subscribes to the full topic for a given DApp protocol and filters messages locally, keeping only those addressed to the user's own wallet. Simpler routing infrastructure, but the wallet client receives all messages from the topic and discards irrelevant ones. This is viable when per-DApp message volumes are low, which is expected given the small Cardano DApp ecosystem.

We should identify if there is real need to target a single user as recipient for messages ⚠️ *Not validated.* 

### 9. Current alternatives and gaps

| Current method | Specific gap |
|---|---|
| Push notifications (DApp mobile apps) | Requires native app installation; most users use browser wallets. |
| In-app UI only | Pull-based — users have to open the DApp to see alerts. |
| Twitter / Discord | Broadcast, not targeted. Users have to follow the DApp account. |
| On-chain events | Machine-readable but no user-facing push; users must poll. |

**The key gap:** DApps know their users by wallet address. There is no existing channel to push a targeted notification to a wallet address. This is the most distinctive value proposition in the entire use case set.

### Open validation questions

- ⚠️ Do lending protocols (Liqwid, etc.) identify missed liquidation warnings as a user experience problem they want to solve?
- ⚠️ Would wallet providers integrate per-DApp notification subscriptions into their backends? What is their current approach to DApp-originated notifications?
- ⚠️ Are DApp teams willing to pay for notification delivery, or do they expect this as free infrastructure?
- ⚠️ Is it relevant for DApps to be able to send messages to specific users? or is it enough to broadcast to their entire user base?

---

## UC-6 · DAOs & Protocol Teams → Token Holders: Governance & Update Notifications

**Maps to original use case:** Governance

*Note: This can be seen as a subset of UC-5 (DApps → users), where the message type is specifically governance and protocol update notifications rather than position-specific alerts.*

### 1. Communication problem

DAO governance structures and protocol teams need to notify token holders about votes, parameter changes, treasury decisions, and protocol updates. This overlaps significantly with UC-3 (Cardano governance) but applies to protocol-specific governance — Minswap's DAO, SundaeSwap's governance, individual project treasuries — rather than Cardano's on-chain governance.

### 2. Senders and recipients

- **Senders:** Protocol teams, DAO governance contracts, project leads — tens to hundreds of projects.
- **Recipients:** Token holders for each protocol — hundreds to thousands per project.

### 3. Value created

Similar to UC-3: governance participation, informed decision-making. Additionally, for protocol teams: user engagement and retention.

### 4. Actor set sizes

| Actor | Estimated Count |
|---|---|
| Active Cardano DAOs / protocol teams with notification need | Tens to hundreds ⚠️ |
| Token holders per protocol | Hundreds to thousands |
| Wallet infrastructure providers | ~10 |

### 5. Willingness to pay

- **Protocol teams:** Possibly — this is a product cost similar to email marketing. More likely for governance-critical notifications. ⚠️ *Not validated.*

### 6–9. Pattern, delivery, connectivity, alternatives

Largely identical to UC-3. Push broadcast to token holder set; mediated through wallet backends; best-effort acceptable for most content; current alternatives are social media and protocol-specific Discord servers.

The main distinction from UC-3 is that the sender is a protocol team rather than a recognized on-chain governance body — the authentication model needs to accommodate permissioned self-registration rather than a curated authority list.

### Open validation questions

- ⚠️ Do Cardano DeFi protocol teams (Minswap, SundaeSwap, Liqwid) see push notification to token holders as a meaningful product gap?
- ⚠️ Would users subscribe to per-protocol governance notifications, or would notification fatigue be a problem?

---

## UC-7 · Users → Solvers: DeFi Intent Broadcasting

**Maps to original use case:** DeFi Intents; Cross-Chain (subtype)

### 1. Communication problem

Users with trading intents (swap, bridge, fee abstraction request) want competing agents to discover and fulfill them. The intent is expressed as a partial transaction; an agent completes it in exchange for a spread or fee. Broadcasting to multiple competing agents is intended to maximize execution quality.

### 2–9. Analysis

This use case has been analyzed extensively in the March 2026 technical review. The framework dimensions are documented here for completeness:

- **Pattern:** Many-to-few (millions of potential users → dozens of solvers); push; near-realtime with short expiry (minutes).
- **Delivery:** Best-effort is acceptable (intents expire); latency-sensitive.
- **Recipient connectivity:** Solvers are always-online servers; users are wallet clients.
- **Current alternatives:** DEX order books on Cardano, direct wallet-to-solver APIs.

### Open validation questions

- ⚠️ Are solver operators interested in an open intent broadcast network, or do they prefer proprietary access?
- ⚠️ What is the actual solver count on Cardano today, and what discovery mechanism do they use?
- ⚠️ When is CIP-118 expected to land, and what is the actual dependency?

---

## UC-8 · Automated Agents: High-Frequency Machine Coordination

**Maps to original use case:** Agent Coordination

### 1. Communication problem

Automated systems (liquidation keepers, arbitrage bots, MEV searchers) need to coordinate at machine speed: broadcasting detected opportunities, bidding on liquidations, negotiating flash loans.

### 2–9. Analysis

- **Pattern:** Many-to-many (or maybe one-to-one; push; near-realtime; sub-second expiry.
- **Required throughput:** 10,000+ msg/sec (from project NFR1.2).
- **Recipient connectivity:** All parties are always-online automated systems.
- **Current alternatives:** Direct WebSocket connections, centralized relay services, mempool monitoring.

---

## UC-9 · Delegated Signers → Scoopers: Sundae Deferred Execution

**Maps to original use case:** None directly — concrete, validated instance of the pattern described in UC-7 (users → solvers), surfaced from the Sundae team.

### 1. Communication problem

Sundae's Strategies feature lets a user lock funds on-chain and authorize a delegated signer key to issue orders against those funds (e.g. DCA, conditional execution). The delegated signer creates and signs orders off-chain and must deliver them to *scoopers* — a permissioned set of Sundae actors that batch orders and submit them on-chain. Today this delivery happens over HTTP POSTs directly to scooper endpoints; Sundae wants to replace that with DMQ.

The user only interacts with the signer at delegation time (on-chain, when locking funds and registering the signer key), so the user ↔ signer link is not a communication problem this use case needs to solve.

### 2. Senders and recipients

- **Senders:** Delegated signers — any actor holding a key authorized by some user. In practice these are likely hosted services (DCA backends, automated strategy operators), but the model permits a user's own wallet to act as its own signer. ⚠️ *Profile and count not validated.*
- **Recipients:** Scoopers — a known, permissioned list of Sundae DEX actors that batch and submit orders on-chain. Small set. ⚠️ *Exact size not validated; likely tens.*

### 3. Value created

- **For signers / strategy operators:** A standardized delivery channel that does not require maintaining per-scooper HTTP integrations or credentials.
- **For scoopers:** Reliable inbound order flow without operating ingress endpoints exposed to arbitrary clients.
- **For users (indirect):** Better resilience in the delivery path means strategies are less likely to fail because one HTTP endpoint went down.
- **Ecosystem value:** Demonstrates DMQ as a substrate for off-chain DeFi coordination; pattern is reusable by other DEXes if it generalizes.

### 4. Actor set sizes

| Actor | Estimated Count |
|---|---|
| Delegated signers | Unknown — scales with strategy adoption ⚠️ |
| Scoopers | Permissioned list, likely tens ⚠️ |
| Order rate | Thousands per hour at scale (per Sundae) |

### 5. Willingness to pay

- **Sundae:** Strong implicit signal — they are actively building against DMQ, which is a more concrete demand indicator than any of UC-1 through UC-6. Whether Sundae as a project, individual signer services, or scoopers ultimately absorb the cost is open. ⚠️ *Not validated.*

### 6. Communication pattern

- **Pattern:** Many-to-few; push; asynchronous.
- **Message size:** ~100s of bytes (signed order).
- **Throughput:** Thousands per hour at scale.
- **Expiry:** Minutes — orders are time-sensitive against market conditions.
- **Unicast vs. multicast:** Unclear — does a signer publish each order to *all* scoopers or to one? Affects whether this is broadcast-with-deduplication or routed delivery. ⚠️ *Not validated.*

### 7. Delivery requirements

- At least one scooper must receive each order for it to execute. If multicast, scoopers need a coordination mechanism to avoid duplicate execution, or DMQ must guarantee single-delivery. ⚠️ *Mechanism not validated.*
- Order loss has direct financial impact: a missed order is a missed trade, and for time-sensitive strategies (DCA, conditionals) it cannot simply be retried later.
- Ordering between independent orders is not required.

### 8. Connectivity profile

- **Scoopers:** Always-on DEX infrastructure — favorable, similar to UC-1.
- **Signers:** Likely hosted services (always-on); possibly user wallets in some configurations. ⚠️ *Profile not validated.*

This is the cleanest connectivity profile in the doc apart from UC-1 — both ends are likely infrastructure rather than light wallets, so no wallet-backend intermediation is required.

### 9. Current alternatives and gaps

| Current method | Specific gap |
|---|---|
| HTTP POST direct to scooper endpoints | Tight coupling between signer and scooper; signer must know each scooper's endpoint and credentials; failure of an endpoint blocks delivery to that scooper; no standardized discovery. |

**The gap:** No uniform, decentralized transport for delivering signed orders from delegated signers to a permissioned scooper set, independent of any individual scooper's HTTP availability.

### Relationship to UC-7

UC-9 is structurally a more concrete, validated instance of the UC-7 pattern (users → solvers): many-to-few push of short-lived signed messages to a small set of always-on executors. The differences are:

- **Permissioning:** Scoopers are a known list; UC-7 solvers were assumed open.
- **Sender identity:** Delegated agents acting on behalf of users, not users directly.
- **Demand validation:** UC-9 has an actor (Sundae) actively building against it; UC-7 was inferred.

If both end up in scope, they likely share most of the protocol-level mechanics.

### Open validation questions

- ⚠️ **Unicast or multicast?** Does a signer publish each order to all scoopers or to one? If multicast, what prevents double execution — scooper-side coordination, first-to-batch-wins, or DMQ-level single-delivery?
- ⚠️ **Scooper count?** Order of magnitude (5? 20? 100?). Materially affects whether broadcast is the right pattern at all.
- ⚠️ **Signer profile?** Are delegated signers typically hosted services (always-on) or could they be user wallets? Determines whether the sender side is infrastructure-class or wallet-class.
- ⚠️ **Why move off HTTP?** What specifically is broken or undesirable about the current path — operational burden, censorship surface, scooper discovery, scaling, authentication? The "why" sharpens the gap analysis.
- ⚠️ **Authentication:** How do scoopers verify a signed order matches the on-chain delegation? DMQ-layer concern or strictly application-layer?
- ⚠️ **Anti-spam:** What prevents an actor with no on-chain delegation from flooding scoopers with bogus orders? In HTTP today this is presumably per-source rate-limiting; in a broadcast network it requires a different defense.
- ⚠️ **Generalization:** Is this pattern Sundae-specific or expected to be reused by other Cardano DEXes (e.g. Minswap)? Determines whether UC-9 represents one project or a class.

---

## UC-10 · Users ↔ Sponsors: Sundae Capacity Exchange (Request-for-Quote Fee Sponsorship)

**Maps to original use case:** None — new structural pattern (request/reply with multi-party quote fan-in). Surfaced from the Sundae team. Currently shipping on Midnight; planned for Cardano subject to a chain-level prerequisite (nested transactions).

### 1. Communication problem

A user wants to submit a transaction but lacks the resource needed to pay its fee — DUST on Midnight, or (in the planned Cardano variant) a babel-fees scenario where the user holds non-ADA tokens but no ADA. The user wants to find a *sponsor* willing to balance the fee portion of their transaction in exchange for compensation in some other token the user does hold.

The communication problem has three sub-steps that today are handled by a Sundae-hosted off-chain broker:

1. **Discovery / Request-for-Quote (RFQ):** the user advertises a request (intended transaction shape, what they can pay with) to the pool of available sponsors.
2. **Quote response:** sponsors return offers (price, signed partial transaction).
3. **Match and settle:** the user picks an offer, the chosen sponsor co-signs, the user submits the resulting transaction on-chain.

DMQ's possible role spans from "transport for steps 1–2 underneath Sundae's existing broker" to "replacement of the broker entirely so users and sponsors are direct network participants." The two ends of that spectrum imply very different architectural commitments. ⚠️ *Scope not validated — see Q1 below.*

### 2. Senders and recipients

- **Senders (users):** End users on light wallets needing fee sponsorship. Open-ended population — anyone transacting on Midnight (or eventually Cardano) without holding the gas token.
- **Recipients / counter-senders (sponsors):** Specialized services that hold NIGHT (or ADA) and operate sponsorship infrastructure. Small, specialized set — likely tens to low hundreds. ⚠️ *Count not validated.*
- **Broker (today):** Sundae-hosted Capacity Exchange server. In a DMQ-based design this role is replaced or thinned, depending on Q1.

### 3. Value created

- **For users:** Onboarding without holding the chain's native gas token; cold-start solved on Midnight (where new users have zero DUST by definition); analogous UX win on Cardano if the babel-fees variant ships.
- **For sponsors:** Revenue from the spread between gas-token cost and the compensation token they receive.
- **For Sundae:** Already-built product on Midnight; DMQ would let the same pattern reach Cardano and reduce reliance on a hosted broker.
- **Ecosystem value:** Lowers a structural friction point for new users; demonstrates DMQ supporting a request/reply pattern beyond pure broadcast.

### 4. Actor set sizes

| Actor | Estimated Count |
|---|---|
| Users (requesters) | End-user scale, open-ended; mediated through wallets |
| Sponsors / providers | Tens to low hundreds ⚠️ |
| Wallet infrastructure providers | ~10 (likely required as user-side intermediaries) |
| Volume on Cardano | ~2 msgs per 20 txs (per Sundae) — modest |
| Message size | 1–2 KB |
| Quote lifetime | 5–10 minutes |

### 5. Willingness to pay

- **Sundae:** Same strong implicit signal as UC-9 — actively building the Midnight version.
- **Sponsors:** Likely yes — sponsorship is itself a paid service, so absorbing transport cost as a business expense is plausible.
- **Users:** Indirectly, via the spread the sponsor charges.

⚠️ *Distribution of cost across the three not validated.*

### 6. Communication pattern

- **Pattern:** Request/reply with multi-party quote fan-in. Structurally distinct from every prior use case in this document. Logical view: user fans request out to many sponsors and receives multiple quotes; wire view depends on Q1.
- **Phases:** (a) RFQ broadcast, (b) quote responses back to the requesting user, (c) selection + final co-signed transaction handoff with the chosen sponsor.
- **Message size:** 1–2 KB (signed partial transaction or quote payload).
- **Lifetime:** 5–10 minutes per RFQ window.
- **Volume:** Modest — ~2 msgs per 20 Cardano txs at scale.
- **Reply routing:** Quotes must reach the specific requesting user — a routing requirement absent in pure broadcast use cases. Closest analog is UC-5's targeted delivery, but in reverse direction.

### 7. Delivery requirements

- RFQ delivery: best-effort acceptable per individual sponsor — the user only needs *one* viable quote to proceed. Loss of any single sponsor's view of the request is tolerable as long as enough sponsors see it.
- Quote delivery back to the user: more critical — a lost quote is a lost competitive offer, directly affecting price.
- The final co-signed transaction handoff is point-to-point between user and chosen sponsor and has the same delivery criticality as UC-9 (a missed handoff is a missed transaction).
- Ordering not required across independent RFQs.

### 8. Connectivity profile

- **Sponsors:** Always-on infrastructure (NIGHT/ADA-holding services). Favorable.
- **Users:** Light wallets — same connectivity challenge as UC-2 through UC-6. Wallet backend intermediation is almost certainly required for users to participate in the protocol; wallets would issue RFQs and receive quotes on the user's behalf.

This makes UC-10 a hybrid: sponsor side looks like UC-1 / UC-9 (always-on infra), user side looks like the notification cluster (mediated through wallet backends).

### 9. Current alternatives and gaps

| Current method | Specific gap |
|---|---|
| Sundae-hosted Capacity Exchange (HTTPS/WebSocket) | Centralized broker — single point of failure, censorship surface, and trust assumption that the broker matches honestly. |
| Direct sponsor APIs (1:1 user → sponsor service) | No price discovery — user takes whatever the sponsor offers; no marketplace dynamics. |
| Holding the gas token directly | Fails the cold-start problem (Midnight) or imposes an awkward UX requirement (Cardano babel-fees scenarios). |

**The gap:** No decentralized RFQ-style marketplace exists today for fee sponsorship. The current shipped system relies on a hosted Sundae server.

### Relationship to other use cases

- **vs. UC-7 (Users → Solvers):** Both are many-to-few with users sending intent-shaped messages to a small set of executors. UC-7 was modeled as one-way push; UC-10 explicitly requires a return path with multiple competing replies. UC-10 is the strongest evidence in this document that request/reply semantics may be a first-class requirement, not just a special case of broadcast.
- **vs. UC-9 (Sundae Strategies):** Both come from Sundae and both involve signed-transaction workflows. UC-9 replaces direct HTTP coupling between two known parties; UC-10 replaces (or augments) a hosted matchmaker. Different "what does DMQ buy us?" answers.
- **vs. UC-5 (DApps → users):** Both require routing a message back to a specific user/wallet, but the trigger and direction differ.

### Open validation questions

- ⚠️ **Q1 — Scope of DMQ's role:** Does DMQ replace Sundae's hosted broker entirely (users and sponsors become direct network participants), or does it serve as transport underneath a still-centralized matching service? The answer materially changes the architectural ask. *This is the highest-priority question.*
- ⚠️ **Cardano timeline:** The use case is shipped on Midnight today; on Cardano it is gated on nested transactions. Is UC-10 in scope for DMQ now (Midnight-only) or contingent on the Cardano protocol change? Determines whether this is a near-term or speculative target.
- ⚠️ **Privacy:** The RFQ contains the user's intended transaction before commitment — broadcasting it to all sponsors leaks trading intent. Is this acceptable, or is selective disclosure / per-sponsor unicast required?
- ⚠️ **Quote authentication and binding:** Is a quote a partial signature (binding offer) or just a price commitment that the sponsor can later refuse to honor? Affects what guarantees the transport must preserve.
- ⚠️ **Sponsor discovery and onboarding:** On Midnight today, sponsors are vetted by Sundae's hosted service. In a DMQ-based design, is sponsor registration open or curated? Who decides?
- ⚠️ **Anti-spam:** RFQs are cheap to fabricate but expensive for sponsors to evaluate (each requires simulated fee balancing). What prevents adversaries from probing sponsor behavior at scale?
- ⚠️ **Reply routing:** Quotes must reach the specific requesting user. Routed by ephemeral request ID, wallet address, ephemeral pubkey? Same problem class as UC-5, opposite direction.
- ⚠️ **Generalization:** Is the RFQ-marketplace pattern Sundae-specific, or do other actors (Hydra fee markets, DEX aggregators) want the same primitive?

---

## UC-11 · Hydra / Gummiworm: Head Negotiation

**Maps to original use case:** None directly — surfaced from the Hydra and Gummiworm teams.

*Insufficient information at this time to complete the nine-dimension framework analysis. This section captures what is known and the questions that need to be answered before a full review can be drafted.*

### What we know

Opening a Hydra head requires the prospective participants to agree on parameters — initial state, contestation period, collateral commitments, the shape of the eventual init transaction — before any on-chain action. According to the Sundae team, this coordination today "piggybacks on Cardano's network layer" — the concrete mechanism behind that phrase is not yet clear to us (see Q3). Gummiworm is reported to have a related but possibly distinct head-negotiation flow.

Reported sizing:

- **Message size:** ~1–2 KB (parameter bundle plus signatures).
- **Lifetime:** ~20s — suggests individual proposal steps expire fast even if the whole negotiation spans longer.
- **Volume:** Dozens per month — head openings are rare events.

### What we don't know

Questions to answer before drafting a full UC entry, roughly in priority order:

1. ⚠️ **What is Gummiworm and how does its head negotiation differ from vanilla Hydra?** Same use case with two flavors, or two distinct use cases? Determines whether this is one section or two.
2. ⚠️ **Discovery or negotiation?** Is the problem to be solved (a) finding counterparties willing to open a head, or (b) running the multi-step protocol once participants are known to each other? These are different shapes — (a) is a marketplace; (b) is small-group coordinated messaging. The brief reads like (b), but worth confirming.
3. ⚠️ **What does "piggybacks on Cardano's network layer" mean concretely today, and why move off?** Specific mechanism (mini-protocol, side mux, ad-hoc out-of-band) and the motivation for moving (performance, supporting non-SPO participants, decoupling, censorship surface, etc.). The "why" is what sharpens the gap analysis, per the rule applied to UC-9.
4. ⚠️ **Group size N per head.** 2-party? Up to 5? Up to 50? Determines whether this is bilateral, multilateral, or chain-of-bilateral coordination.
5. ⚠️ **Authentication model.** How do prospective participants verify each other — on-chain stake/key identity, out-of-band, certificate-based?
6. ⚠️ **Protocol shape.** Fire-and-forget broadcast ("I want to open a head with these params, takers welcome") or stateful back-and-forth (propose → counter → accept)? The 20s lifetime hints at the former; the term "negotiation" hints at the latter.
7. ⚠️ **Public or private?** Are negotiation messages broadcast (anyone can see who is opening a head with whom) or unicast / encrypted between specific candidates?
8. ⚠️ **Continuity with UC-12.** Once a head is negotiated, does the same DMQ channel carry the subsequent operational gossip / signatures (UC-12), or is it a clean separation? Affects whether the two use cases share infrastructure.

### Suggested next step

The answers to Q1 and Q2 alone determine whether this is one use case, two, or actually a sub-case of UC-12 (Gummiworm gossip & signatures). Direct conversation with the Hydra and Gummiworm teams is the most efficient way to close these.

---

## UC-12 · Gummiworm: In-Operation Gossip & Signatures

**Maps to original use case:** None directly — surfaced from the Gummiworm team.

*Placeholder only. We know less about this use case than UC-11 — the brief is not enough to construct a framework analysis, and a conversation with the Gummiworm team is required before drafting one.*

### What we know (from the brief)

Ongoing message and signature gossip during a head's *operation* (distinct from head opening, UC-11). The Gummiworm team has indicated that "some or all" of this traffic could be offloaded to DMQ — the split is not specified.

Reported sizing, with unusually wide ranges:

- **Message size:** hundreds of bytes up to a few MB.
- **Lifetime:** 20s to a few minutes.
- **Volume:** dozens per 5 minutes up to thousands per second.

The drivers behind the size and volume spread are unknown.

### What we need to learn

A conversation with the Gummiworm team is required. Starter questions, all preliminary:

- ⚠️ What "gossip" means concretely — Hydra-internal mini-protocol traffic, multi-signature aggregation, state updates, or something else.
- ⚠️ What the "some or all" split is — which messages are candidates for DMQ offload and which stay on Gummiworm-internal channels, and why.
- ⚠️ What drives the size and volume spreads (idle vs. active head, head size, message type).
- ⚠️ Whether each gossip channel is scoped to a single head's participants or spans multiple heads.
- ⚠️ Delivery and ordering needs — likely different for signature aggregation vs. routine gossip.
- ⚠️ Relationship to UC-11 — does the negotiated channel from UC-11 carry this traffic, or is this a separate setup?

UC-11 and UC-12 likely benefit from being scoped together since they share actors and may share infrastructure.

---

## UC-13 · Multi-Sig Groups: Off-Chain Signature Coordination

**Maps to original use case:** None directly — surfaced as a generic pattern.

*This is a class of use cases rather than a single one (see below). The brief alone — "Coordinating signatures across parties via DMQ" — is too thin to construct a full framework analysis. This entry captures what is structurally clear and the questions to close before drafting.*

### What we know

A multi-sig script (native or Plutus) names a fixed set of N keys and requires M-of-N signatures to authorize a transaction. The off-chain coordination problem is:

1. A proposer drafts a transaction.
2. The draft is distributed to the M-of-N signers.
3. Each signer reviews and produces a signature (or refuses).
4. Once M signatures are gathered, the transaction is aggregated and submitted on-chain.

This is a small-group, known-participant messaging pattern with typed payloads (transaction drafts and signatures) and an on-chain-rooted identity model (the script defines who can sign).

### This is a class, not a single use case

Distinct actor populations share the same pattern:

- **DAO and project treasuries** (Cardano DeFi treasuries, project treasuries).
- **Foundation / corporate custody** (cold wallets, joint accounts).
- **Validator / SPO operational key custody** (keys split across a team).
- **Governance committees** (Constitutional Committee, Intersect-internal multi-sigs).

Each has different urgency, group size, frequency, and trust model. Whether DMQ should target one of these specifically or the generic pattern is itself an open question (Q1 below).

### Structural notes

- **Close adjacency to UC-11.** Both are small-group coordination among known parties producing a co-signed on-chain transaction. Differences: multi-sig groups are *persistent* (defined by the script) and sign many transactions over time; head negotiation is *one-shot* per head and group identity is negotiated per opening. They likely share infrastructure.
- **Distinctive aspects vs. generic group messaging:**
  - Typed messages, not free-form.
  - On-chain-rooted identity (the script).
  - Privacy is likely a hard requirement — treasury and custody scenarios will not tolerate public broadcast of pending proposals.
  - Delivery reliability matters: a missed signature request can stall a transaction.
- **Volume profile.** Per-group volume is very low (transactions per week or per month at most). Ecosystem aggregate depends on how many groups participate.

### What we need to learn

Questions to close before a full UC entry, roughly in priority order:

1. ⚠️ **Who specifically asked for this, which sub-class are they representing, and what is their current multi-sig coordination workflow?** Treasury, custody, validator custody, governance committees, or all of the above? Determines the actor profile we should be designing for, and grounds the gap analysis below in a real workflow rather than an abstract one.
2. ⚠️ **What is the specific gap with that current workflow?** Coordination tooling already exists (purpose-built wallets, PSBT-style file passing, hardware wallet ceremonies, Discord-plus-link workflows). Is the gap file-passing pain, lack of standard, lack of authenticated channel, missing audit trail, something else? Without this, the use case risks being a solution looking for a problem. Same rule as UC-9.
3. ⚠️ **Privacy requirement.** Is encrypted unicast within the group required, or is topic-level access control sufficient, or is broadcast acceptable for some sub-classes? Likely varies by sub-class.
4. ⚠️ **Scope of script types.** Cardano native scripts only, Plutus multi-sig, governance committee signing, or all of the above?
5. ⚠️ **Group size distribution.** 3-of-5 is canonical; are 10+ groups in scope? Affects whether we need only small-group coordination or something larger.
6. ⚠️ **Relationship to UC-11.** Same actors and infrastructure, or genuinely separate communities? Affects whether UC-11 and UC-13 should be merged or kept distinct.
7. ⚠️ **Discussion / free-form messages.** In addition to proposals and signatures, do groups need a discussion channel (reasons for refusal, requests for clarification), or is signing strictly fire-and-forget?

### Suggested next step

Identify which sub-class (treasury, custody, governance, validator) is the primary motivator and run the framework against that one specifically — the generic pattern is too abstract to design against, and answering Q1 narrows the rest substantially.

---

## Comparative Summary

| # | Scenario | Sender count | Recipient count | Recipient type | Communication pattern | Delivery need | Sender pays? |
|---|---|---|---|---|---|---|---|
| UC-1 | Protocol developer teams → SPOs | ~10 | ~3,000 | Always-on servers | Broadcast, push, tiered urgency | High (critical alerts) | Maybe |
| UC-2 | SPOs → delegators | ~hundreds | Hundreds of thousands (mediated) | Light wallets via backends | Broadcast per pool, push | Best-effort | Maybe small |
| UC-3 | Governance → community | ~tens | Hundreds of thousands (mediated) | Light wallets via backends | Broadcast, push | Medium-high | Likely yes |
| UC-4 | DReps → delegators | ~hundreds | Hundreds to thousands each (mediated) | Light wallets via backends | Broadcast per DRep, push | Best-effort | Unlikely |
| UC-5 | DApps → users | Tens | Hundreds to thousands each (mediated) | Light wallets via backends | Targeted by address, push | High (financial alerts) | Likely yes |
| UC-6 | DAOs → token holders | Tens to hundreds | Hundreds to thousands each (mediated) | Light wallets via backends | Broadcast per protocol, push | Best-effort | Maybe |
| UC-7 | Users → solvers | Many | ~dozens of solvers | Always-on servers | Many-to-few, push, short expiry | Best-effort | Per intent |
| UC-8 | Agent coordination | Automated systems | Automated systems | Always-on | Many-to-many (or 1-1), realtime | Hard realtime | Yes |
| UC-9 | Delegated signers → scoopers (Sundae) | Unknown ⚠️ | Tens (permissioned) ⚠️ | Always-on infrastructure | Many-to-few, push, short expiry | High (financial) | Yes (Sundae actively building) |
| UC-10 | Users ↔ sponsors (Capacity Exchange) | End-user scale (mediated) | Tens to low hundreds ⚠️ | Sponsors always-on; users via wallet backends | Request/reply with multi-party quote fan-in | Medium-high | Yes (Sundae actively building) |
| UC-11 | Hydra/Gummiworm head negotiation | Small group (2–N) ⚠️ | Same group (peers) | Likely always-on ⚠️ | Small-group coordination, multi-step ⚠️ | TBD ⚠️ | TBD ⚠️ |
| UC-12 | Gummiworm in-operation gossip & signatures | Per-head participants | Same group (peers) | Likely always-on ⚠️ | Group gossip + signature aggregation | Mixed (signing reliable; gossip TBD) ⚠️ | TBD ⚠️ |
| UC-13 | Multi-sig signature coordination | Group of M-of-N (small) | Same group (peers) | Mixed (varies by sub-class) | Small-group coordination, typed (txs + sigs) | High (signing) | TBD ⚠️ |

---

## Observations

### 1. Wallet backend adoption is load-bearing for most use cases

UC-2 through UC-6, and the user side of UC-10, all depend on wallet providers integrating a notification or messaging layer. Without that, the end-user delivery chain does not exist. This is not a protocol design question — it is a business development and adoption question that needs to be answered before committing to any design direction.

### 2. The notification cluster (UC-1 through UC-6) shares a common architecture

All six notification scenarios share a similar delivery chain (publisher → relay network → wallet backend → user) with authenticated broadcast or targeted delivery. The main split within this cluster is:

- **UC-1:** Small, professional recipient set (SPOs) directly on the protocol — no intermediation needed.
- **UC-2 through UC-6:** Large end-user recipient sets, requiring wallet backend intermediation.

### 3. The actual set of direct protocol participants is small across all use cases

Across UC-1 through UC-6, the number of senders per communication channel is consistently small — tens to a few hundreds at most. On the recipient side, wherever the ultimate audience is large (delegators, token holders, DApp users), those recipients are mediated through wallet infrastructure providers, reducing the set of direct protocol participants to roughly ~10 backends. The new use cases reinforce rather than break this pattern: scoopers (UC-9), sponsors (UC-10), Hydra / Gummiworm head participants (UC-11/12), and multi-sig groups (UC-13) are all small sets — tens at most.

The actual number of nodes that need to participate in the communication protocol is therefore modest — likely in the hundreds, not the hundreds of thousands. Designs optimized for large open networks may be unnecessary overhead.

### 4. The use case set splits into four structural clusters

The thirteen use cases form four clusters with materially different patterns:

- **Notification cluster (UC-1–UC-6):** One-to-many broadcast with optional address-based targeting (UC-5). Wallet-backend mediation is the dominant adoption question.
- **Open many-to-few (UC-7, UC-8):** Many senders pushing intent-shaped messages to a small set of always-on executors, possibly with unauthenticated senders. Higher throughput and lower latency than the notification cluster.
- **Permissioned many-to-few and request/reply (UC-9, UC-10):** Like the previous cluster but with known recipients and, for UC-10, a return path with multi-party quote fan-in. UC-10 is the strongest signal in this document that DMQ may need first-class request/reply semantics rather than only broadcast.
- **Small-group coordination (UC-11, UC-12, UC-13):** Persistent or semi-persistent small groups (Hydra heads, multi-sig sets) exchanging typed messages — transactions, signatures, parameter proposals — with privacy and reliability requirements that differ from broadcast.

These clusters are distinguished by structural axes (pattern, sender openness, return-path requirement, group persistence) — not by scale or configuration. A design that handles one cluster well does not automatically handle the others.

### 5. UC-1 and UC-9 share an unusually clean delivery profile

Both have always-on infrastructure recipients and no wallet-backend intermediation: SPOs (UC-1) and scoopers (UC-9). These are the easiest delivery targets in the document and may form a useful starting point for any phased deployment.

### 6. Targeted and request/reply patterns add routing requirements absent from broadcast

UC-5 (DApps → users) requires targeted delivery by wallet address. UC-10 (Capacity Exchange) requires reply routing back to a specific requester. UC-11/12/13 likely require encrypted unicast or otherwise private channels within a known group. These differ from the topic-broadcast model that dominates the notification cluster, and may push the design toward supporting:

- Address- or identity-based targeting (UC-5).
- Ephemeral request-ID-based reply routing (UC-10).
- Group-scoped private channels (UC-11/12/13).

UC-5's targeted-delivery requirement was previously called out as the single most distinctive technical requirement; with UC-10 and UC-11/12/13 in scope, it is now one of three different routing departures from pure broadcast.

### 7. Demand validation is uneven across the use cases

UC-9 and UC-10 are the strongest demand signals in the document — Sundae is actively building against both. UC-11 and UC-12 are partially specified by the Hydra and Gummiworm teams but have meaningful gaps (see those sections' open questions). UC-13 has no identified primary actor yet. UC-1 through UC-8 remain inferences from project documentation and general ecosystem knowledge.

Priority conversations to close the validation gap, in rough order:

1. **Sundae Labs:** Confirm scope of DMQ's role in UC-9 and UC-10 (transport vs. broker replacement); pin down Cardano timeline for UC-10.
2. **Hydra and Gummiworm teams:** Resolve the Q1/Q2 questions in UC-11 (Gummiworm's distinct flow; discovery vs. negotiation); scope UC-12 enough to draft a full entry.
3. **Wallet backend providers** (Blockfrost, Eternl, Lace, Yoroi): Will they integrate a notification or RFQ-reply layer? What would they need?
4. **Protocol developer teams / Intersect:** Is fragmented emergency coordination (UC-1) a recognized problem? What is the current incident communication process?
5. **DeFi protocols** (lending, DEX, bridging): Do they want to reach their users through a standardized channel (UC-5)? What would they pay?
6. **SPOs and/or DReps:** Do they feel they have a communication problem with their delegators today (UC-2, UC-4)?
7. **Multi-sig sub-class champion:** Identify whoever raised UC-13 and which sub-class (treasury, custody, governance, validator key custody) they represent.

---

## What This Document Intentionally Does Not Cover

- Protocol design and economic model — both depend on the selection of scope and use cases.
- Formal requirements — the existing functional and non-functional requirements documents cover these for the original scope; a revised requirements document for the selected scope is a later deliverable.

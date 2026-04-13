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

---

## Observations

### 1. Wallet backend adoption is load-bearing for most use cases

UC-2 through UC-6 all depend on wallet providers integrating a notification subscription layer. Without that, the end user delivery chain does not exist. This is not a protocol design question — it is a business development and adoption question that needs to be answered before committing to any design direction.

### 2. The notification cluster (UC-1 through UC-6) shares a common architecture

All six notification scenarios share a similar delivery chain (publisher → relay network → wallet backend → user) with authenticated broadcast or targeted delivery. The main split within this cluster is:

- **UC-1:** Small, professional recipient set (SPOs) directly on the protocol — no intermediation needed.
- **UC-2 through UC-6:** Large end-user recipient sets, requiring wallet backend intermediation.

### 3. The actual set of protocol participants may be small

Across UC-1 through UC-6, the number of senders per communication channel is consistently small — tens to a few hundreds at most. On the recipient side, wherever the ultimate audience is large (delegators, token holders, DApp users), those recipients are mediated through wallet infrastructure providers, reducing the set of direct protocol participants to roughly ~10 backends. This means the actual number of nodes that need to participate in the communication protocol is modest on both sides — likely in the hundreds, not the hundreds of thousands.

This is relevant for protocol design: the scale requirements for this use case cluster may be well within the reach of simpler protocols that do not need to handle millions of direct, unmediated participants. Designs optimized for large open networks may be unnecessary overhead.

### 4. UC-5 (DApps → users) is the most technically distinctive

When/If it requires targeted delivery by wallet address — not broadcast — is the one requirement that meaningfully differs from the rest of the notification cluster. DApps may be willing to pay to reach their users through a standardized channel, making this potentially the most commercially interesting scenario in the cluster. It does introduce additional complexity: address-based routing requires wallet backends to maintain per-user subscription state or rely on client-side filtering.

### 5. UC-1 (protocol developer teams → SPOs) stands alone

UC-1 is the scenario where all recipients are always-on infrastructure with no intermediary needed.

### 6. UC-7 and UC-8 are structurally different from the notification cluster

The differences are not a matter of scale or configuration — they are structural:

- **Communication pattern:** UC-7 is many-to-few (users to solvers) and UC-8 is many-to-many or one-to-one (agent to agent). These are not extensions of one-to-many broadcast; they are different patterns.
- **Performance requirements:** Both require higher throughput and lower latency than any of the notification use cases.
- **Sender characteristics:** The potential sender population is significantly larger than in the notification cluster and may include unauthenticated parties (e.g., any user broadcasting an intent to solvers).
- **Data economics:** The economic value of the data exchanged in these use cases may impose constraints on the communication pattern. The competitive value of an intent, for instance, may make direct sender-to-solver communication more appropriate than broadcast through intermediaries — a consideration that does not arise for one-to-many notifications.

### 7. Nothing has been validated with actual actors

The demand assessments in this document are inferences from the project documentation and general ecosystem knowledge. Before committing to any design direction, the following conversations are highest priority:

1. **Wallet backend providers** (Blockfrost, Eternl, Lace, Yoroi): Will they integrate a notification layer? What would they need?
2. **Protocol developer teams / Intersect**: Is fragmented emergency coordination a recognized problem? What is the current incident communication process?
3. **DeFi protocols** (lending, DEX, bridging): Do they want to reach their users through a standardized channel? What would they pay?
4. **SPOs and/or DReps**: Do they feel they have a communication problem with their delegators today?

---

## What This Document Intentionally Does Not Cover

- Protocol design and economic model — both depend on the selection of scope and use cases.
- Formal requirements — the existing functional and non-functional requirements documents cover these for the original scope; a revised requirements document for the selected scope is a later deliverable.

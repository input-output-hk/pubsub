# Cardano PubSub — Technical Review

**Reviewer:** Ezequiel Postan

**Date:** March 2026

**Scope:** Product use cases, functional/non-functional requirements, and AUEB technical report

**References:**
- [Product documentation](https://input-output-hk.github.io/pubsub/)
- [Functional requirements](https://input-output-hk.github.io/pubsub/product/requirements/functional/)
- [Non-functional requirements](https://input-output-hk.github.io/pubsub/product/requirements/non-functional/)
- [Use cases overview](https://input-output-hk.github.io/pubsub/use-cases/)
- [AUEB technical report](D2-Cardano-PubSub-Framework-Design-and-Architecture.pdf)

---

## 1. Executive Summary

This review examines the Cardano PubSub [product documentation](https://input-output-hk.github.io/pubsub/) (five use cases, requirements) and the accompanying [technical report](D2-Cardano-PubSub-Framework-Design-and-Architecture.pdf) describing a three-layer P2P overlay architecture with DHT-based persistence. The review finds that:

1. **The five proposed use cases each suffer from specific technical issues** — including incentive misalignment, unrealistic performance targets, conflation of messaging with application logic, and redundancy with on-chain capabilities.

2. **The technical report's architecture does not address several critical requirements** — including incentives for storage and honest participation, anti-spam protection, payment models, security model and honesty assumptions, QoS differentiation, and privacy. Additionally, the design is optimized for one-to-many broadcast and is not suitable for many-to-many or one-to-one message exchange patterns.

3. **All five use cases collapse into a single defensible primitive**: a standardized mechanism for identifiable parties to submit verifiable events to interested subscribers, with best-effort delivery and no mandatory high throughput or long-term persistence.

4. **Use cases that cannot be served by this primitive** (notably DeFi intent distribution and agent coordination) require fundamentally different communication patterns — not variations of pub/sub — and alternative protocol designs should be explored for them.

---

## 2. Per-Use-Case Findings

### 2.1 DeFi Intents

**Source:** [DeFi Intents use case](https://input-output-hk.github.io/pubsub/use-cases/defi-intents/)

**Product vision:** Users broadcast partial transactions (intents) via a decentralized message bus; agents compete to fulfill them; settlement occurs on-chain via [CIP-118](https://github.com/cardano-foundation/CIPs/tree/master/CIP-0118) nested transactions.

**Key issues:**

- **Incentive misalignment on message forwarding.** Agents that receive intents have a direct financial incentive to suppress them rather than forward to competitors. This is not an edge case but the dominant rational strategy for profit-maximizing agents, and it fundamentally breaks the gossip protocol's assumption of cooperative forwarding. The result is degraded delivery reliability — precisely the opposite of what the system promises.

- **Liquidity fragmentation from suppression.** When agents suppress intents, complementary orders (e.g., Alice wanting to swap A→B and Bob wanting B→A) may never reach the same solver. This causes silent fulfillment failures — users experience their intents simply expiring without knowing that a counterparty existed but was partitioned away.

- **"ADA-free broadcasting" vs. spam prevention.** The system promises users can publish without holding ADA (DMB-2 in the [DeFi Intents spec](https://input-output-hk.github.io/pubsub/use-cases/defi-intents/#message-bus-properties)), but provides no mechanism to prevent spam. The technical report has no rate-limiting design, despite this being listed as a requirement ([FR5.1](https://input-output-hk.github.io/pubsub/product/requirements/functional/#resource-management)). Without economic cost to publish, open intent topics become trivially floodable.

- **Latency target (<500ms p95) is tight for gossip.** The [performance requirement NFR1.1](https://input-output-hk.github.io/pubsub/product/requirements/non-functional/#performance) specifies <500ms latency. The technical report's Hybrid Dissemination model requires O(log n) hops. For 10,000 subscribers at ~30ms per hop, propagation alone approaches the budget before accounting for real-world conditions (NAT traversal, cross-continental latency, congestion).

- **No ordering fairness.** Gossip-based delivery inherently creates variable delivery times based on topology proximity to the publisher. In competitive agent markets, this gives structural advantages to well-positioned nodes — the same class of problem Ethereum's MEV ecosystem has struggled with (see [Flashbots research](https://writings.flashbots.net/)).

- **Hard dependency on CIP-118.** The entire flow depends on nested transactions ([CIP-118](https://github.com/cardano-foundation/CIPs)), which is not yet implemented in Cardano's ledger. If it ships with different semantics or is delayed, the use case collapses.

- **Agent discovery is undesigned.** No mechanism exists for users to discover which agents can fulfill which intents, or for agents to advertise their capabilities. This is acknowledged as "not started" in the use case's [open questions](https://input-output-hk.github.io/pubsub/use-cases/defi-intents/#open-questions-pubsub-specific).

**Assessment:** The pub/sub broadcast model is structurally incompatible with competitive intent fulfillment markets due to rational incentives against message forwarding. This use case requires a fundamentally different protocol design (see [Section 4.1](#41-defi-intents-direct-to-solver-registry)).

### 2.2 Governance

**Source:** [Governance use case](https://input-output-hk.github.io/pubsub/use-cases/governance/)

**Product vision:** Governance bodies push verified, actionable notifications to voters via PubSub-compatible wallets; voters see proposals and vote with one click.

**Key issues:**

- **Vote routing via PubSub introduces unnecessary trust assumptions.** The [scenario](https://input-output-hk.github.io/pubsub/use-cases/governance/#scenario-constitutional-committee-vote) routes votes through PubSub to a "Vote Aggregator" who submits results on-chain. This introduces a middleman that can selectively drop or delay votes. Cardano's current [CIP-1694](https://cips.cardano.org/cip/CIP-1694) governance uses on-chain vote transactions, which are self-certifying. Moving votes off-chain adds complexity and trust requirements with no clear benefit.

- **Conflation of UX with PubSub.** "One-click voting" is a wallet UX feature, not a PubSub feature. PubSub delivers bytes; the wallet renders actionable notifications. Most voters use mobile/browser wallets that will never connect directly to the PubSub protocol — they rely on wallet service provider backends. In practical terms, PubSub's actual role is backend-to-backend notification delivery to a handful of wallet providers, not direct user communication.

- **Redundant persistence.** Under [CIP-1694](https://cips.cardano.org/cip/CIP-1694), governance proposals are submitted as on-chain transactions, with full text typically on IPFS. Storing proposal notifications in the DHT for 30 days ([NFR3.4](https://input-output-hk.github.io/pubsub/product/requirements/non-functional/#reliability)) with 7-10x replication creates a third copy of data already durably available on-chain and IPFS. The DHT adds no data availability that doesn't already exist.

- **Ballot secrecy contradicts the design.** [FR2.2](https://input-output-hk.github.io/pubsub/product/requirements/functional/#privacy-security) requires anonymous messaging for ballot secrecy, but the [vote message schema](https://input-output-hk.github.io/pubsub/use-cases/governance/#message-schema) includes the voter's verification key and signature. Achieving actual ballot secrecy requires blind signatures, ZK proofs, or homomorphic tallying — none of which is designed.

- **Discussion topics are a separate system.** `governance/discussion/{id}` topics with 100k+ messages represent a group messaging application (many-to-many, threading, search, pagination), not a pub/sub notification pattern. This is structurally a different communication paradigm and should not be bundled into PubSub.

- **Notification policy is undefined.** For on-chain events (proposal submitted, vote passed), wallet backends already index the chain — a PubSub notification is redundant. PubSub adds value only for off-chain coordination moments ("proposal under discussion," "please review this draft"). But these are driven by social conventions, not protocol rules, and there is no specification for when such messages should be sent.

**Assessment:** The core value — authenticated, censorship-resistant notification delivery from governance bodies to wallet backends — is legitimate. Everything else (vote routing, persistence, discussions) either belongs elsewhere or duplicates existing capabilities. The use case should be scoped down to notifications only, with voting remaining on-chain.

### 2.3 Network Operations

**Source:** [Network Operations use case](https://input-output-hk.github.io/pubsub/use-cases/network-operations/)

**Product vision:** Authenticated, cryptographically-signed emergency alerts and coordination for SPOs and network operators.

**Key issues:**

- **Chain independence is claimed but not achieved.** The topic registry, authority registry, and replication server membership are all on-chain. During a chain halt, the system operates on stale cached state. This is acceptable in practice (the authority list for emergency topics changes infrequently), but the product should acknowledge this as a degraded mode rather than claiming full independence.

- **Automated client response is unacceptable risk.** The [message schema](https://input-output-hk.github.io/pubsub/use-cases/network-operations/#message-schema) includes action directives (SAFE_MODE, PAUSE) intended for automated execution by validator clients. This creates a remote kill switch: a compromised signing key could instruct every validator to halt block production simultaneously. Regardless of cryptographic safeguards (multi-sig, threshold signatures), the risk profile of remote automated control over validator behavior is too high. PubSub should deliver information; operators should decide responses locally.

- **The node population is small.** Emergency alerts target ~3,000 SPO pools. Broadcasting a signed message to 3,000 nodes is not a demanding distributed systems problem. However, reusing PubSub infrastructure built for other use cases (rather than deploying a separate network) is the pragmatic choice.

- **Chain halt recovery conflates messaging with consensus.** The [recovery scenario](https://input-output-hk.github.io/pubsub/use-cases/network-operations/#scenario-chain-halt-recovery) of validators publishing state reports and a coordinator issuing restart instructions is a mini-consensus protocol disguised as messaging. Cardano already has chain recovery procedures at the [Ouroboros protocol](https://www.iog.io/papers/ouroboros-praos-an-adaptively-secure-semi-synchronous-proof-of-stake-protocol) level; introducing PubSub as a second source of truth for chain state risks split-brain scenarios.

- **Incident evidence overstates PubSub's impact.** The [cited incidents](https://input-output-hk.github.io/pubsub/use-cases/network-operations/#evidence-the-cost-of-the-current-gap) (Ronin, Terra, Solana, Prysm) primarily failed at detection or decision-making, not message transport. PubSub would help at the margins but would not have prevented the losses as framed.

**Assessment:** This has the clearest real-world motivation of all five use cases. The notification delivery piece (signed alerts from authorities to SPOs) is the single most defensible PubSub application. Automated response capabilities should be removed entirely. The use case is an instance of the general authenticated notification primitive.

### 2.4 Cross-Chain

**Source:** [Cross-Chain use case](https://input-output-hk.github.io/pubsub/use-cases/cross-chain/)

**Product vision:** Users express cross-chain intents ("bridge my BTC to Cardano and stake it") and agents handle the complexity, coordinated via PubSub.

**Key issues:**

- **PubSub solves the wrong layer.** Cross-chain security failures (Ronin, Wormhole, Nomad) occurred at the verification layer, not the transport layer. PubSub contributes nothing to proof verification, atomicity, or settlement guarantees — the hard problems in cross-chain operations.

- **This is a subtype of DeFi Intents.** The intent format, topic structure, actor model, and flow are nearly identical to the [DeFi Intents use case](https://input-output-hk.github.io/pubsub/use-cases/defi-intents/). The DeFi Intents page itself includes a ["BTC Bridge with Babel Fees" example](https://input-output-hk.github.io/pubsub/use-cases/defi-intents/#example-flow-btc-bridge-with-babel-fees) that duplicates this use case. There is no justification for a separate entry.

- **Opportunity discovery doesn't fit PubSub.** Protocols advertising yield is marketing content derived from on-chain state. Broadcasting it via PubSub to users on other chains assumes those users run Cardano PubSub clients, which may be unrealistic (maybe partner chains could be considered).

- **Massive scope creep.** The [architectural implications](https://input-output-hk.github.io/pubsub/use-cases/cross-chain/#architectural-implications) (verifier plugins for multiple chains, foreign signature support, multi-chain addressing) represent bridge infrastructure requirements, not messaging layer concerns.

- **Atomicity is unaddressed.** Failure handling is listed as "not started" in the [open questions](https://input-output-hk.github.io/pubsub/use-cases/cross-chain/#open-questions), but this is the central problem of any bridge protocol and is entirely outside PubSub's scope.

**Assessment:** This use case should not exist separately. The intent broadcasting component is a subtype of DeFi Intents (and shares its incentive problems). Everything else belongs in bridge protocol design, not a messaging layer specification.

### 2.5 Agent Coordination

**Source:** [Agent Coordination use case](https://input-output-hk.github.io/pubsub/use-cases/agent-coordination/)

**Product vision:** High-throughput coordination bus for automated systems — liquidation keepers, arbitrage bots, MEV searchers — operating at machine speed.

**Key issues:**

- **Throughput requirements are incompatible with gossip.** [NFR1.2](https://input-output-hk.github.io/pubsub/product/requirements/non-functional/#performance) specifies 10,000+ msg/sec with sub-500ms p99 latency. Gossipped to all subscribers, this generates millions of message deliveries per second network-wide. This is the domain of multicast trees or centralized relay infrastructure, not gossip protocols. 

- **No rational actor would publish opportunities.** If an indexer detects an undercollateralized position, the rational action is to liquidate it — not broadcast it to competitors. If an agent detects a price discrepancy, publishing it destroys the arbitrage. The entire MEV ecosystem's evolution has been toward less public information sharing (private mempools, sealed-bid auctions), not more.

- **Suppression incentives are maximal.** Liquidation opportunities are discrete, high-value, and time-critical. Suppressing a single signal can be worth thousands of dollars. Arbitrage signals are destroyed by sharing. Rational agents would never forward these messages.

- **Private negotiation isn't pub/sub.** The `agents/negotiate/{session}` topic is bilateral encrypted messaging, not broadcast dissemination. The technical report has no protocol design for point-to-point private channels, despite this being a core requirement ([FR1.1](https://input-output-hk.github.io/pubsub/product/requirements/functional/#core-messaging)).

- **Auction and flash loan protocols are application logic.** The [liquidation scenario](https://input-output-hk.github.io/pubsub/use-cases/agent-coordination/#scenario-liquidation-coordination) is an auction protocol; the [flash loan scenario](https://input-output-hk.github.io/pubsub/use-cases/agent-coordination/#scenario-multi-hop-arbitrage) requires request-response semantics. Neither is a messaging concern.

- **One legitimate sub-case exists but is misframed.** DeFi protocols notifying users about their positions (e.g., a lending protocol alerting users that their collateral is at risk) is valuable and fits the general notification primitive. But this is user notification, not agent coordination.

**Assessment:** This is the weakest use case. The economic rationale for publishing is absent, the throughput requirements are unrealistic for the proposed architecture, and the scenarios conflate messaging with application protocols. The legitimate sub-case (DeFi protocol notifications to users) fits the general notification primitive.

---

## 3. Gaps in the Technical Report

The AUEB technical report proposes a three-layer P2P overlay — [SecureCyclon](https://arxiv.org/abs/2309.02952) for peer sampling, [Vicinity](https://link.springer.com/chapter/10.1007/978-3-642-45065-5_2) for navigation, and Hybrid Dissemination for message delivery — with a clique-DHT for persistence. While the overlay construction protocols are well-grounded in distributed systems research, the report does not address several requirements that are critical for a production deployment:

- **Incentives for storage and honest participation.** The report acknowledges that the incentivization mechanism for replication servers is "work in progress." More broadly, there is no analysis of why nodes would honestly forward messages rather than suppress or delay them, nor any peer scoring or misbehavior detection mechanism.

- **Anti-spam protection.** No rate-limiting or spam prevention mechanism is designed for message publication within a topic. This is critical for any topic that allows open or semi-open publishing, and is explicitly required by [FR5.1](https://input-output-hk.github.io/pubsub/product/requirements/functional/#resource-management).

- **Payment models.** There is no concrete design for how the economic costs of operating the network (storage, bandwidth, relay) are funded. The report mentions publishers paying according to replication factor and retention period, but the mechanism is unspecified.

- **Security model and honesty assumptions.** The report does not state explicit honesty assumptions (e.g., what fraction of nodes must be honest for dissemination guarantees to hold). SecureCyclon addresses link manipulation, but application-layer misbehavior (selective message dropping) is not considered.

- **Privacy.** End-to-end encryption ([FR2.1](https://input-output-hk.github.io/pubsub/product/requirements/functional/#privacy-security)), metadata privacy ([FR2.4](https://input-output-hk.github.io/pubsub/product/requirements/functional/#privacy-security)), and anonymous messaging ([FR2.2](https://input-output-hk.github.io/pubsub/product/requirements/functional/#privacy-security)) are listed as product requirements but are entirely absent from the technical design.

- **QoS differentiation.** The architecture treats all messages in all topics identically. There is no mechanism for priority routing, topic-level QoS policies, or backpressure under load — despite the product requiring different QoS levels for different use cases (e.g. best-effort for price feeds vs. guaranteed delivery for governance).

- **Communication patterns beyond one-to-many broadcast.** The architecture is optimized for topic-based broadcast dissemination. Point-to-point messaging ([FR1.1](https://input-output-hk.github.io/pubsub/product/requirements/functional/#core-messaging)) and group messaging with configurable privacy ([FR1.2](https://input-output-hk.github.io/pubsub/product/requirements/functional/#core-messaging)) have no corresponding protocol design. This is appropriate for the notification use case but means the architecture is not recommended for many-to-many or one-to-one exchange patterns referenced in other use cases.

- **Practical networking concerns.** NAT traversal ([FR3.4](https://input-output-hk.github.io/pubsub/product/requirements/functional/#network-discovery)), transport diversity — TCP, WebSocket, WebRTC ([FR3.2](https://input-output-hk.github.io/pubsub/product/requirements/functional/#network-discovery)), and light client protocols ([FR5.5](https://input-output-hk.github.io/pubsub/product/requirements/functional/#resource-management)) are not addressed.

The on-chain topic registry with authenticated publisher lists is a well-designed component that maps directly to the notification use case's needs. The clique-DHT design leveraging on-chain registered SPOs is a sound Cardano-specific architectural choice for the persistence layer, though as noted in [Section 5.4](#54-architectural-implications), the persistence requirements for the defensible use case are more modest than what the DHT is designed for.

---

## 4. Alternative Approaches for Non-PubSub Use Cases

### 4.1 DeFi Intents: Direct-to-Solver Registry

The incentive problems with broadcasting intents via pub/sub (suppression, fairness, liquidity fragmentation — see [Section 2.1](#21-defi-intents)) stem from the broadcast model itself — routing intents through intermediary nodes that are also competitors.

An alternative protocol may sidestep these issues:

1. **On-chain solver registry.** A smart contract allows intent solvers (agents) to register their interest in specific intent types, publishing their endpoints and capabilities. This mirrors the existing topic registry concept but inverts it — instead of listing authorized publishers, it registers interested specialized subscribers.

2. **Direct submission by users.** When a user constructs an intent, their wallet reads the solver registry and submits the intent **directly** to all registered solvers (or a user-selected subset) via point-to-point messages. No intermediary forwarding is needed.

3. **Incentive alignment.** Since the user sends directly to solvers, there is no forwarding step for anyone to suppress. All registered solvers receive the intent simultaneously (within network latency bounds). Solvers compete on execution quality, not on information advantage from topology positioning.

4. **User agency.** Users can choose which solvers to send to — all of them for maximum competition, or a trusted subset. This provides a natural mechanism for reputation-based solver selection without requiring protocol-level enforcement.

This approach is simpler than full pub/sub, avoids the forwarding incentive problem entirely, and leverages the on-chain registry infrastructure that the technical report already designs. It is not a pub/sub protocol, but it addresses the DeFi Intents communication need more effectively than one.

### 4.2 Agent Coordination: Existing Infrastructure

For high-throughput, low-latency machine-to-machine coordination (liquidations, arbitrage, flash loans), the honest assessment is that this problem is best served by direct WebSocket connections, centralized relay services, or purpose-built MEV infrastructure — as every production system in this space already uses. Attempting to serve this via decentralized gossip adds latency, reduces reliability, and introduces incentive problems, with no compensating benefit.

The one sub-case worth retaining — DeFi protocols notifying users about their positions (collateral warnings, order fulfillment) — fits naturally within the general notification primitive (see [Section 5.1](#51-the-single-defensible-use-case)) and does not require a separate agent coordination architecture.

---

## 5. Consolidated Recommendation

### 5.1 The Single Defensible Use Case

Across all five proposed use cases, the analysis converges on a single communication primitive that is both technically sound and economically rational:

> **A standardized mechanism for identifiable parties (protocol maintainers, governance bodies, DApp teams, infrastructure operators) to submit verifiable events to interested subscribers (wallet backends, SPO nodes, other service providers), with best-effort delivery, modest throughput requirements, and no mandatory long-term message persistence.**

### 5.2 Scenarios Covered by This Primitive

This single primitive covers a wide range of practical scenarios across the Cardano ecosystem:

**Governance and protocol-level notifications:**
- Constitutional Committee announcing proposals for review
- Voting deadline reminders and DRep recommendation updates
- Protocol upgrade announcements and security advisories from client teams
- Emergency alerts to SPOs (critical bugs, chain incidents)

**DeFi protocol notifications to users:**
- Lending protocols warning users about at-risk collateral positions or completed liquidations
- DEXs confirming that swap orders have been fulfilled
- Protocols notifying users about contract migrations, DAO votes, new token listings, or new features
- Limited DeFi operational scenarios (e.g., Babel fee requests) where incentives may be aligned with honest message distribution (although fairness is still not ideal)

**SPO-to-delegator communication:**
- SPOs notifying their delegators about planned maintenance windows
- Fee or margin changes, reward distribution updates
- Pool retirement notices or other operational announcements
- SPOs with mission statements could announce updates on their goals

**General ecosystem announcements:**
- DApp developers announcing migrations or version upgrades
- NFT projects notifying holders about events or snapshots
- Marketing and promotional notifications (e.g., "first N users to do X receive Y")
- Blog posts, educational content, or community updates from ecosystem entities

### 5.3 User Experience and Subscription Model

End users would not typically connect directly to the PubSub protocol. Two complementary consumption models are envisioned:

1. **Wallet-integrated subscriptions.** Wallet service providers run PubSub nodes and subscribe to topics on behalf of their users. Users would configure preferred topics within their wallet settings (or wallets could automatically subscribe to topics relevant to the DApps and protocols the user interacts with). The wallet backend filters incoming messages by relevance and delivers them via conventional push notification infrastructure.

2. **Standalone notification app.** A dedicated PubSub client application could allow users to connect to the network directly, browse available topics, and manage their own subscriptions. This serves power users, developers, and anyone who prefers not to depend on a wallet provider for their notification feed.

Both models are compatible with the same underlying PubSub infrastructure. The choice between them is a user preference, not an architectural constraint.

### 5.4 Architectural Implications

For this scoped use case, the architecture could be simplified considerably:

- **Identity and authentication:** On-chain registry of authorized publishers per topic, verified via DID or public key signatures.
- **Persistence:** Lightweight store-and-forward buffer (hours) for catch-up by backends that were briefly offline. On-chain data remains the authoritative source for durable information. The full DHT with weeks-long retention and high replication factors is not, a priori, required.
- **Topic structure:** Simple hierarchical naming convention (`governance/proposals`, `ops/emergency`, `defi/{protocol_id}/alerts`, `spo/{pool_id}/announcements`) with moderated publishing.
- **Client model:** SPO nodes and wallet/infrastructure backends (e.g., [Blockfrost](https://blockfrost.io/)) are the primary subscribers. End users receive notifications via their wallet provider or a standalone client application (see [Section 5.3](#53-user-experience-and-subscription-model)).

### 5.5 Economic Sustainability

The notification use case may have a natural economic model that does not require elaborate incentivization mechanisms:

- **Wallet and infrastructure providers** ([Blockfrost](https://blockfrost.io/), wallet backends) run nodes because they need the data for their own business — delivering notifications is part of the service they offer users. The marginal cost of a PubSub node on top of existing infrastructure is modest.
- **SPOs** are natural participants both as subscribers (network operations alerts) and publishers (delegator communications). Running a PubSub node alongside their existing relay infrastructure is a low additional burden.
- **Message forwarding costs are negligible** for low-throughput notification traffic. Unlike competitive DeFi scenarios where suppressing a message has high economic value (see [Section 2.1](#21-defi-intents) and [Section 2.5](#25-agent-coordination)), there is no meaningful incentive to suppress a governance notification or a maintenance alert. The cost of honest forwarding is trivially small relative to the node operator's existing infrastructure costs. Rate or space limits could be imposed per topic too.

This means the system can operate without complex payment models or slashing mechanisms, at least for the notification use case. If higher-throughput or persistence-heavy use cases are added in the future, economic models would need to be revisited.

### 5.6 What PubSub Should Not Try to Be

The review identifies several patterns that are better served by other approaches:

| Pattern | Why Not PubSub | Better Approach |
|---|---|---|
| Competitive intent distribution | Rational suppression, fairness issues, liquidity fragmentation ([Section 2.1](#21-defi-intents)) | Direct-to-solver via on-chain registry ([Section 4.1](#41-defi-intents-direct-to-solver-registry)) |
| High-frequency agent coordination | Throughput exceeds gossip capacity, no incentive to share ([Section 2.5](#25-agent-coordination)) | Direct connections, centralized relays, MEV-style infrastructure ([Section 4.2](#42-agent-coordination-existing-infrastructure)) |
| On-chain vote submission | Adds trust assumptions vs. direct on-chain voting ([Section 2.2](#22-governance)) | Keep votes on-chain ([CIP-1694](https://cips.cardano.org/cip/CIP-1694)) |
| Cross-chain proof relay | Bridge protocol concern, not messaging ([Section 2.4](#24-cross-chain)) | Dedicated bridge infrastructure (e.g. [LayerZero](https://layerzero.network/), [IBC](https://ibcprotocol.dev/), etc.) |
| Group discussions | Many-to-many chat system, not pub/sub broadcast ([Section 2.2](#22-governance)) | Decentralized messaging protocols (state of the art to be reviewed) |
| Automated validator control | Unacceptable security risk ([Section 2.3](#23-network-operations)) | Local operator policy only |

---

## 6. Summary of Findings

| # | Finding | Severity |
|---|---|---|
| 1 | All five use cases collapse to a single notification primitive once application logic, unrealistic throughput, and incentive-incompatible designs are removed ([Section 5.1](#51-the-single-defensible-use-case)) | High — reshapes product scope |
| 2 | The technical report does not address incentives for honest participation, anti-spam, payment models, security/honesty assumptions, privacy, or QoS differentiation ([Section 3](#3-gaps-in-the-technical-report)) | High — critical gaps for production deployment |
| 3 | DeFi intent broadcasting via gossip is structurally incompatible with competitive markets due to rational message suppression and fairness issues ([Section 2.1](#21-defi-intents)) | High — use case non-viable as designed |
| 4 | Agent coordination lacks economic rationale for publishing and requires throughput beyond gossip capacity ([Section 2.5](#25-agent-coordination)) | High — use case non-viable as designed |
| 5 | Cross-chain is a subtype of DeFi Intents, not a separate use case ([Section 2.4](#24-cross-chain)) | Medium — documentation restructuring |
| 6 | Governance vote routing via PubSub adds trust assumptions over current on-chain model ([Section 2.2](#22-governance)) | Medium — scope reduction needed |
| 7 | Automated validator response to remote commands is unacceptable security risk ([Section 2.3](#23-network-operations)) | High — feature should be removed |
| 8 | Persistence layer is over-engineered for actual needs; on-chain data is the authoritative source ([Section 5.4](#54-architectural-implications)) | Medium — simplification opportunity |
| 9 | The on-chain topic registry and authenticated publisher model is well-designed and reusable ([Section 3](#3-gaps-in-the-technical-report)) | Positive finding |
| 10 | A direct-to-solver protocol using an on-chain registry could address intent distribution more effectively than pub/sub ([Section 4.1](#41-defi-intents-direct-to-solver-registry)) | Constructive recommendation |
| 11 | The notification use case has a natural economic model — node operators have intrinsic business reasons to participate, and low message volumes make forwarding costs negligible ([Section 5.5](#55-economic-sustainability)) | Positive finding |


# PubSub Fee and Incentive Model

Persistence-layer economics for the three-layer dissemination architecture. Deliverable D3. Draft, April 2026.

### Terminology

**Cardano epoch vs settlement period.** This document distinguishes between **Cardano epochs** (the ~5-day protocol cycle used for staking, delegation, and replication server registration) and **settlement periods** (the PubSub-specific interval at which storage proofs are submitted and fees are released). The settlement period length is a tunable parameter, not necessarily aligned with Cardano epochs. See [Open Question 1](#7-open-questions).

**Delegation stake vs locked collateral.** Two different uses of "stake" appear in this document. **Delegation stake** refers to ADA delegated to a stake pool via Cardano's native delegation mechanism. It is not locked: the ADA remains in the delegator's wallet, spendable at any time. PubSub reads this on-chain to determine rate-limiting quotas (Tier 1). **Locked collateral** refers to ADA deposited into a Plutus script address as a slashable security bond. This ADA is locked and only reclaimable when the replication server deregisters (Tier 2). The two are independent: a node's delegation stake sets its anti-spam quota, while a replication server's locked collateral secures its storage commitments.

## 1. Context and Scope

This document addresses Deliverable D3 (Fee and Incentive Analysis) from the [Architecture Building Blocks](architecture-building-blocks.md) scoping document, and directly responds to the gaps identified in [Ezequiel Postan's March 2026 technical review](pubsub-technical-review-march-with-links.md): incentives for honest participation, anti-spam protection, and payment models.

The [AUEB research (D2)](D2-Cardano-PubSub-Framework-Design-and-Architecture.pdf) designed a three-layer dissemination protocol ([SecureCyclon](SecureCyclon%20-%20Dependable%20Peer%20Sampling.pdf), [Vicinity](Vincinity%20-%20A%20pinch%20of%20randomness%20brings%20out%20the%20structure.pdf), Hybrid Dissemination) and a clique-DHT persistence layer with SPO-operated replication servers. It left the incentivization mechanism for replication servers as "work in progress." This document fills that gap. See also [research foundation](../site/architecture/research-foundation.md) and [protocol walkthrough](protocol-walkthrough.md) for how the three layers interact.

> **Key framing:** Ezequiel's review [correctly identifies](pubsub-technical-review-march-with-links.md#55-economic-sustainability) that for the base notification use case, elaborate incentive mechanisms are unnecessary. SPOs and wallet backends participate because they need the data. Forwarding costs at notification-scale are negligible. We adopt this assessment and scope the incentive model to the **persistence layer**, where economic sustainability requires active design.

---

## 2. Two-Tier Incentive Architecture

The incentive model separates into two tiers that align with the architecture's existing split between dissemination (ephemeral) and persistence (durable).

### 2.1 Tier 1: Cooperative Dissemination (No Payment)

The dissemination layer (SecureCyclon + Vicinity + RandCast/RingCast) operates without fees. Participants forward messages cooperatively because:

- **Intrinsic value:** SPOs need network operations alerts. Wallet backends need governance notifications. The data itself is the incentive.
- **Negligible cost:** For notification-scale traffic (low hundreds of messages per topic per day), bandwidth and compute costs are trivially small relative to existing infrastructure costs.
- **No suppression incentive:** Unlike DeFi intents, there is no economic gain from suppressing a governance notification or maintenance alert.
- **Sybil resistance via delegation stake:** Nodes prove identity by referencing a Cardano staking credential (delegation to a pool). The ADA remains in their wallet, but the on-chain delegation record provides a verifiable cost-of-entry signal. No ADA is locked.

This is consistent with [Ezequiel's Section 5.5 assessment](pubsub-technical-review-march-with-links.md#55-economic-sustainability). Anti-spam at this tier is handled by rate limiting at the topic level (quotas proportional to delegation stake, per-topic message caps), enforced locally by each node. This does not require on-chain settlement or locked collateral.

### 2.2 Tier 2: Incentivized Persistence (Paid)

When a topic configures `retentionPeriod > 0` and `replicationFactor > 1`, the persistence layer activates. Replication servers (primarily SPOs) store messages and serve them to subscribers who were offline. This layer has real costs (storage, bandwidth, uptime SLA) and requires economic compensation.

This is where the fee model applies. The [AUEB research (D2 Ch.4)](D2-Cardano-PubSub-Framework-Design-and-Architecture.pdf) already defines the participants and structure:

| Component | AUEB Design (D2) | This Proposal Adds |
|-----------|-------------------|---------------------|
| Replication servers | SPO-operated, registered on-chain with IP + pubkey | Locked collateral requirement, conditional fee release |
| Registration | On-chain, join/leave at Cardano epoch boundaries | Locked collateral in Plutus script (slashable) |
| Publisher funding | "Periodic payments funded by publishers" (unspecified) | Per-settlement-period escrow with conditional release |
| Verification | "Proof of Replication / Proof of Retrieval challenges" (unspecified) | Bloom filter PoS + Merkle spot-checks |
| Penalties | "Security deposit slashed for failures" (unspecified) | Slashing on failed spot-checks, rollback on insufficient proof |

---

## 3. Persistence Layer Fee Mechanism

### 3.1 Proof of Storage (PoS)

The core evidence problem for the persistence layer is: did the replication server actually store the messages and can it serve them on retrieval? This is distinct from proving real-time delivery (which Tier 1 handles cooperatively).

Each replication server maintains a **counting Bloom filter** of message hashes it stores for each topic, indexed by the existing scheme: `hash(topicId.publisherId.sequenceNr)`. The filter acts as a compact proof of storage capacity: roughly 1 KB per 10,000 stored messages at 1% false positive rate (Bose et al, 2008).

Verification works through spot-check challenges:

- **Challenge:** Any network participant can issue a challenge by selecting a random message key from the topic's known sequence range and posting a small challenge deposit on-chain. The deposit prevents griefing (frivolous challenges). The challenge references a specific `hash(topicId.publisherId.sequenceNr)` tuple.
- **Response:** The replication server must produce the message content whose hash matches the challenged key. The Bloom filter provides a fast pre-check; the actual content retrieval proves real storage.
- **On-chain commitment:** The replication server periodically posts a Merkle root of all stored message hashes to the topic's on-chain record. This Merkle root uses the existing topic-partitioned structure from the Topic Registry.

### 3.2 Escrow and Conditional Release

Publishers who configure persistence for a topic deposit fees into a Plutus escrow contract. The deposit covers the cost of storage for the configured `retentionPeriod` and `replicationFactor`.

- **Release condition:** Replication servers submit periodic storage proofs (Merkle root + Bloom filter commitment) once per settlement period. If proofs are valid and spot-checks pass, the Plutus validator releases the proportional fee share for that period.
- **Rollback condition:** If no valid proof is submitted within the timeout, or if spot-checks fail, fees revert to the publisher. Refund is the default; payment requires proof. This is the "rollback-like" property.

Fee calculation per settlement period per topic:

> `fee = message_count x per_message_rate x replicationFactor x retentionPeriod_factor`

The `per_message_rate` emerges from competition among replication servers. The `retentionPeriod_factor` reflects that longer storage costs more. The `replicationFactor` multiplier accounts for the redundancy the publisher requested.

### 3.3 Replication Server Economics

**Locked collateral:** Replication servers deposit ADA into a Plutus script address when registering on-chain (at Cardano epoch boundaries, per D2 design). This ADA is locked for the duration of their registration and slashable on provable storage failure. It is separate from and independent of the server operator's delegation stake.

**Earnings:** Proportional to confirmed settlement periods. A server storing 100 topics with high retrieval success earns more than one storing 10 topics with spotty availability. This naturally selects for reliable infrastructure.

**Loyalty effect:** Topics with consistent storage success (high spot-check pass rate over time) attract lower per-message rates because the risk premium decreases. This creates an implicit reputation: the fee discount is the reputation. No separate scoring system needed.

---

## 4. Integration with Existing Architecture

| Architecture Layer | Existing Component | Incentive Role | Tier |
|--------------------|--------------------|----------------|------|
| Peer Sampling | SecureCyclon | None (cooperative) | Tier 1 |
| Navigation | Vicinity | None (cooperative) | Tier 1 |
| Dissemination | RandCast / RingCast | Rate limiting only | Tier 1 |
| Persistence | Clique DHT + replication servers | Full fee model | Tier 2 |
| Topic Registry | On-chain smart contract | Escrow + settlement | Tier 2 |
| Identity | DID / Identus | Publisher auth for escrow | Both |

### 4.1 On-Chain Footprint

The fee mechanism extends the existing [Topic Registry contract](../contracts/topic-registry/README.md) (see also [formal spec](../formal_spec/topic_registry/README.md)) with two new datum types:

- **EscrowDatum:** Attached to the publisher's deposit UTxO. Contains `topicId`, `depositAmount`, `retentionPeriod`, `replicationFactor`, and `settlementStart`. The Plutus validator enforces release only on valid storage proof.
- **StorageProofDatum:** Submitted by replication servers per settlement period. Contains `topicId`, `serverId`, `merkleRoot`, `bloomFilterCommitment`, and `settlementPeriodNumber`. Validated against the escrow conditions.

Per-settlement-period on-chain cost per topic: one StorageProof transaction per replication server, plus one fee-release transaction. For a topic with `replicationFactor=3` and modest retention, this is four transactions per settlement period.

### 4.2 Compatibility with RandCast vs RingCast

The incentive model is agnostic to the [dissemination layer choice](architecture-building-blocks.md#architecture-decision-randcast-vs-ringcast). Whether the team chooses RandCast (recommended for Phase 1) or RingCast (if BFT ring positioning is solved), the persistence layer operates independently. Dissemination delivers messages to online subscribers; persistence stores them for offline retrieval. The fee model applies only to the latter.

### 4.3 Addressing [Ezequiel's Gaps](pubsub-technical-review-march-with-links.md#3-gaps-in-the-technical-report)

| Gap (from Section 3 of review) | How Addressed |
|---------------------------------|---------------|
| Incentives for storage and honest participation | Escrow + conditional release + slashing for replication servers |
| Anti-spam protection | Tier 1: delegation-weighted rate limits (no locked ADA). Tier 2: publisher pays per message, market kills spam |
| Payment models | Per-settlement-period escrow funded by publishers, proportional to retention x replication |
| Security / honesty assumptions | Tier 1: cooperative (no BFT needed at notification scale). Tier 2: spot-check challenges + slashing |

---

## 5. Anti-Spam Design ([FR5.1](../site/product/requirements/functional.md))

Rate limiting operates at two levels, neither requiring on-chain settlement:

### Topic-Level Caps

Each topic has a maximum message rate configured in the Topic Registry (eg, 100 messages per hour for a governance topic). Nodes enforce this locally: messages exceeding the cap are dropped at the dissemination layer. Authorized publishers (listed on-chain) are exempt from per-sender caps but still subject to the topic-level aggregate cap.

### Delegation-Weighted Quotas

For open topics (no publisher list), message publishing requires a Cardano staking credential. Each sender's rate limit scales with their delegated ADA (read from on-chain stake distribution snapshots), making spam proportionally expensive. A node delegating 100K ADA gets a higher quota than one delegating 1K. No ADA is locked or consumed: the delegation record is read-only. This provides Sybil resistance without on-chain transactions per message.

### Persistence-Layer Cost Barrier

If a topic has persistence enabled, every published message costs the publisher real ADA (via the escrow mechanism). This creates a natural economic barrier: spamming a persistent topic requires continuously funding storage for worthless messages. The market corrects: if subscribers leave a spam topic, the publisher is paying into a system nobody uses.

---

## 6. Open Questions

1. **Settlement period length:** Should the settlement period align with Cardano epochs (5 days), be a fixed shorter interval (eg, 6 hours, 1 day), or be configurable per topic? Longer periods reduce on-chain cost but increase the delay before operators get paid and before rollback kicks in on failure.

2. **Spot-check challenge selection:** Cardano has no on-chain randomness primitive exposed to Plutus. Challenges come from network participants (subscribers, other replication servers) with a deposit to prevent griefing. This means challenge selection is not unbiasable: a colluding challenger could repeatedly target the same keys. Mitigations include requiring challenges to cover a minimum spread of the sequence range, or deriving challenge keys deterministically from the first block hash of the settlement period (not truly random, but unpredictable at escrow time).

3. **Minimum persistence tier:** Ezequiel [suggests](pubsub-technical-review-march-with-links.md#54-architectural-implications) lightweight store-and-forward (hours) may suffice for the notification case. Is the full escrow mechanism needed, or only for topics with `retentionPeriod` > some threshold?

4. **Replication server minimum collateral:** The AUEB design mentions "minimum ADA threshold" but does not specify the locked amount. This affects Sybil resistance, slashing effectiveness, and barrier to entry for smaller SPOs.

5. **Cross-topic storage incentives:** Should replication servers commit to storing all topics assigned by consistent hashing (as in D2 Ch.4), or can they selectively choose profitable topics? The former is simpler but may create unfunded mandates.

---

## References

- Antonov, A., Kolyvas, E., Voulgaris, S. (2024). *Cardano Pub/Sub Framework: Design and Architecture*. AUEB / IOG Research. [D2]
- Postan, E. (2026). *Cardano PubSub Technical Review*. IO Engineering. [March 2026]
- Bloom, B. (1970). 'Space/time trade-offs in hash coding with allowable errors.' *Communications of the ACM*, 13(7).
- Bose, P. et al. (2008). 'On the false-positive rate of Bloom filters.' *Information Processing Letters*, 108(4).
- Lamport, L., Shostak, R., Pease, M. (1982). 'The Byzantine Generals Problem.' *ACM TOPLAS*, 4(3).
- *Architecture Building Blocks* (2026). PubSub Architecture: Building Blocks and Scoping. IO Engineering.

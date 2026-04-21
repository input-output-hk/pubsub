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
- **Sybil resistance via delegation stake:** Nodes reference a Cardano staking credential (delegation to a pool). The ADA remains spendable. Delegation is possible with under 5 ADA, so this is an identity-binding mechanism rather than a strong economic barrier — the real spam deterrent comes from per-topic rate caps (Section 5) and escrow costs.

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

Each replication server maintains a **counting Bloom filter** of message hashes per topic. The hash scheme is `BLAKE2b(salt ‖ topicId ‖ publisherId ‖ sequenceNr)` with a per-filter random salt to prevent adversarial bit-saturation (publishers control their own inputs; cf. Ethereum log bloom filter attacks).

**Sizing.** The formula `m = −n·ln(p) / (ln 2)²` gives ~12 KB for 10,000 messages at 1% theoretical FPR; a counting variant (4-bit counters for deletion support) needs ~48 KB. In practice, counter churn and varying load push the effective FPR higher, so the filter should target ~0.1% theoretical to leave headroom. Exact parameters need empirical tuning before mainnet. Even conservatively sized, overhead is under 5 bytes per message.

**Proving flow.** No message content goes on-chain. The three components work together as follows:

1. **On-chain commitment:** Each settlement period, the replication server posts a **Merkle root** of all stored message hashes to the topic's on-chain record. This is a single 32-byte hash — the tree itself and all message data remain off-chain.
2. **Bloom filter (off-chain):** The server maintains a counting Bloom filter locally as a fast index of what it claims to store. This is not submitted on-chain; it serves as a pre-check during challenges.
3. **Spot-check challenge:** Any participant can challenge a server by posting a small deposit on-chain and naming a specific message key (`BLAKE2b(salt ‖ topicId ‖ publisherId ‖ sequenceNr)`). The server must respond **off-chain** with: (a) the message content whose hash matches the key, and (b) a Merkle inclusion proof against the on-chain root. If the server fails to respond or the proof is invalid, the challenge succeeds and triggers slashing. The challenger's deposit is returned on success, forfeited on frivolous challenges.

### 3.2 Escrow and Conditional Release

Publishers who configure persistence deposit fees into a Plutus escrow script. The UTxO flow:

1. **Deposit.** Publisher creates a UTxO at the escrow script address carrying the total fee for the retention period. The datum records `topicId`, `replicationFactor`, `retentionPeriod`, number of settlement periods, and per-period release amount.
2. **Claim.** Each settlement period, a replication server submits a transaction that spends the escrow UTxO and produces two outputs: (a) a new escrow UTxO with the remaining balance and decremented period count, and (b) a payment UTxO to the server's address. The validator checks that the transaction's redeemer includes a valid `StorageProofDatum` (Merkle root matching the server's on-chain commitment for that period) and that the claimed amount matches the per-period share.
3. **Timeout / rollback.** If no valid claim transaction is submitted before the period deadline (a slot range encoded in the datum), the publisher can reclaim the UTxO. Refund is the default; payment requires proof.

Fee per settlement period per topic:

> `fee = message_count × per_message_rate × replicationFactor × retentionPeriod_factor`

`per_message_rate` emerges from competition among replication servers. Note that the Plutus validator does **not** verify the Merkle proof itself on-chain (too expensive) — it checks that a `StorageProofDatum` was posted for that server and period, and that no successful challenge (slashing) exists. The actual proof verification happens off-chain during spot-checks; the on-chain contract acts as a settlement layer.

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

- **EscrowDatum:** Inline datum on the publisher's deposit UTxO. Fields: `topicId`, `depositAmount`, `replicationFactor`, `retentionPeriod`, `periodsRemaining`, `perPeriodRelease`, `deadlineSlot`. Consumed and re-created each settlement period as servers claim their share (see Section 3.2 step 2).
- **StorageProofDatum:** Posted by replication servers as a reference datum each settlement period. Fields: `topicId`, `serverId`, `merkleRoot`, `settlementPeriodNumber`. The escrow validator checks for its existence when processing a claim.

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

For open topics (no publisher list), publishing requires a Cardano staking credential. Each sender's rate limit is derived from their delegated ADA (read from on-chain stake snapshots) via a **sublinear** function (e.g., `log(stake)` or `sqrt(stake)`) so that whales cannot monopolise topic bandwidth while small delegators still get a usable quota. The exact curve needs simulation against real stake distributions (see [Open Question 6](#6-open-questions)). No ADA is locked or consumed.

Since delegation costs under 5 ADA, these quotas are rate-shaping, not an economic barrier. Inactive users could in theory rent their quota to spammers; binding quotas to peer identity (not just the credential) mitigates this.

### Persistence-Layer Cost Barrier

If a topic has persistence enabled, every published message costs the publisher real ADA (via the escrow mechanism). This creates a natural economic barrier: spamming a persistent topic requires continuously funding storage for worthless messages. The market corrects: if subscribers leave a spam topic, the publisher is paying into a system nobody uses.

---

## 6. Open Questions

1. **Settlement period length:** Should the settlement period align with Cardano epochs (5 days), be a fixed shorter interval (eg, 6 hours, 1 day), or be configurable per topic? Longer periods reduce on-chain cost but increase the delay before operators get paid and before rollback kicks in on failure.

2. **Spot-check challenge selection:** Cardano has no on-chain randomness primitive exposed to Plutus. Challenges come from network participants (subscribers, other replication servers) with a deposit to prevent griefing. This means challenge selection is not unbiasable: a colluding challenger could repeatedly target the same keys. Mitigations include requiring challenges to cover a minimum spread of the sequence range, or deriving challenge keys deterministically from the first block hash of the settlement period (not truly random, but unpredictable at escrow time).

3. **Minimum persistence tier:** Ezequiel [suggests](pubsub-technical-review-march-with-links.md#54-architectural-implications) lightweight store-and-forward (hours) may suffice for the notification case. Is the full escrow mechanism needed, or only for topics with `retentionPeriod` > some threshold?

4. **Replication server minimum collateral:** The AUEB design mentions "minimum ADA threshold" but does not specify the locked amount. This affects Sybil resistance, slashing effectiveness, and barrier to entry for smaller SPOs.

5. **Cross-topic storage incentives:** Should replication servers commit to storing all topics assigned by consistent hashing (as in D2 Ch.4), or can they selectively choose profitable topics? The former is simpler but may create unfunded mandates.

6. **Delegation-weighted quota curve:** What sublinear function (`log`, `sqrt`, piecewise with floor/ceiling) best balances whale vs small-delegator access? Needs simulation against real Cardano stake distributions.

---

## References

- Antonov, A., Kolyvas, E., Voulgaris, S. (2024). *Cardano Pub/Sub Framework: Design and Architecture*. AUEB / IOG Research. [D2]
- Postan, E. (2026). *Cardano PubSub Technical Review*. IO Engineering. [March 2026]
- Bloom, B. (1970). 'Space/time trade-offs in hash coding with allowable errors.' *Communications of the ACM*, 13(7).
- Bose, P. et al. (2008). 'On the false-positive rate of Bloom filters.' *Information Processing Letters*, 108(4).
- Lamport, L., Shostak, R., Pease, M. (1982). 'The Byzantine Generals Problem.' *ACM TOPLAS*, 4(3).
- *Architecture Building Blocks* (2026). PubSub Architecture: Building Blocks and Scoping. IO Engineering.

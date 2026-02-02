# Intent Coordination Infrastructure

*Research compiled: February 2026*

!!! abstract "Summary"
    The coordination layer for intent-based systems is the critical infrastructure gap. Settlement is solved (ERC-7683). But off-chain discovery, propagation, and solver coordination remains centralized across all major protocols.

---

## Executive Summary

The transition from imperative transaction models to declarative intent-based architectures represents the most significant structural shift in decentralized finance (DeFi) infrastructure since the advent of the Automated Market Maker (AMM). Research into the current landscape of intent coordination reveals a stark dichotomy between the theoretical promise of permissionless "intent-centric" architectures and the centralized or federated reality of current production systems.

While protocols such as CoW Protocol, UniswapX, and 1inch Fusion successfully process billions of dollars in volume, they achieve this by circumventing the "hard" problems of decentralized coordination—specifically denial-of-service (DoS) protection and propagation incentives—through the use of permissioned gatekeepers, whitelisted solver networks, and centralized API endpoints.

**The primary technical bottleneck:** The lack of a generalized, DoS-resistant propagation mechanism. Unlike standard Ethereum transactions, which rely on stateless verification of nonces and balances to prevent spam, intents require subjective, state-dependent, and often computationally intensive validation to determine solvability.

**Confidence level:** High regarding current production architectures and market dynamics; cautious regarding proposed decentralized P2P solutions which lack stress-testing in adversarial mainnet environments.

---

## Key Findings (Verified)

### Solver Centralization is Acute and Structural

Analysis of on-chain data confirms that the "decentralized solver market" is effectively an oligopoly:

- **UniswapX:** Wintermute and Tokka Labs account for 60-80% of daily trading volume
- **CoW Protocol:** Top 3 solvers capture >50% of total volume and rewards (early 2025)

This centralization is driven by technical requirements:

- Dutch auctions favor actors with low-latency access to private inventory
- Batch auctions favor sophisticated constraint-solving algorithms and deep capital moats

### Off-Chain Reliance is Systemic

| Protocol | Coordination Method | Decentralization Status |
|----------|-------------------|------------------------|
| **CoW Protocol** | Centralized "Driver" + "Autopilot" service | Federated |
| **UniswapX** | API endpoints (not P2P mesh) | Centralized |
| **1inch Fusion** | "Unicorn Power" staking gates access | Permissioned |
| **Anoma** | P2P gossip (designed) | Testnet only |

**Implication:** Settlement is trustless; coordination is not.

### Security Vulnerabilities in Off-Chain/On-Chain Handoffs

**1inch Fusion Exploit (March 2025):** $5M drained via calldata corruption vulnerability. The attacker manipulated memory layout of transaction data, tricking resolver logic into accepting malicious payload.

**Lesson:** The decoupling of intent signing (off-chain) and intent validation (on-chain) creates dangerous parsing gaps.

### ERC-7683 Standardization Progress

ERC-7683 is the emerging cross-chain intents standard, supported by Uniswap Labs, Across, and ~50 protocols.

**What it covers:**
- Unified `CrossChainOrder` struct
- `resolve()` function for filler queries
- Settlement contract interface

**What it doesn't cover:**
- Discovery/propagation (how fillers find orders)
- Reputation/bonding requirements
- Privacy mechanisms

---

## Key Findings (Unverified/Conflicting)

### Permissionless P2P Intent Gossip Viability

Anoma proposes generalized P2P intent propagation with "sovereign domains" and resource pricing for spam prevention. The architecture is mathematically rigorous but lacks empirical data on performance under heavy DoS attacks in adversarial environments.

**Status:** Theoretical projection, not verified engineering reality.

### Privacy-Efficiency Trade-offs

Conflicting evidence on achieving privacy without sacrificing execution quality:

- **TEE approach (SUAVE):** Fast but relies on hardware trust (Intel SGX)
- **ZK approach:** Trustless but high latency (proof generation)

Side-channel attack concerns make TEEs controversial for high-value financial cryptography.

---

## Detailed Analysis

### Protocol Architectures

#### CoW Protocol: Batch Auction Model

```
[User] --(Sign EIP-712 Intent)--> [Driver]
                                     |
                            (Aggregates into Batch)
                                     |
                                     v
              +----------+----------+----------+
              |          |          |          |
           [Solver A] [Solver B] [Solver C]
              |          |          |          |
              +----------+----+-----+----------+
                              |
                              v
                        [Autopilot]
                  (Selects Highest Surplus)
                              |
                              v
                    [Winning Solver]
              (Executes Atomic Batch Tx)
```

**Bonding requirement:** $500K-$1.5M in stablecoins + COW tokens

**GlueX Incident (Nov 2024):** Solver deployed vulnerable settlement handler. MEV bots drained protocol buffers. CoW DAO voted to slash GlueX bonding pool.

#### UniswapX: Dutch Auction Model

```
Price
  ^
  |   \
  |    \
  |     \  <-- Decay Curve
  |      \
  |       \
  |        *  <-- EXECUTE (First filler wins)
  |         \
  |          \
  |   [Min Price]
  +---------------------------------> Time
```

**Key characteristic:** Speed over optimization. Favors low-latency actors with private inventory.

#### 1inch Fusion: Staked Resolver Network

Access gated by "Unicorn Power" (staked 1INCH tokens):

- **Bucket 1:** Top stakers get exclusive fill window
- **Bucket 2+:** Opens to lower tiers, then public

**Effect:** High barrier to entry → static set of dominant resolvers.

#### Anoma: P2P Intent Gossip (Proposed)

- Decentralized gossip network for intent propagation
- Topic-based routing to specialized solvers
- Resource Machine (ARM) for composing multi-party intents

**Status:** Testnet (July 2025). Mainnet TBD.

---

## The Unsolved Problems

### 1. DoS Resistance

**Why Ethereum mempool fails for intents:**

| Transaction | Intent |
|-------------|--------|
| Stateless validation (nonce, balance, signature) | State-dependent validation (is this solvable?) |
| Failed tx = sender pays gas | Failed intent = no cost to attacker |
| Simple verification | Complex simulation required |

**Attack vector:** Flood network with valid-signature but unsolvable intents (e.g., "Trade 1 ETH for 1,000,000 USDC"). Nodes waste compute; attacker pays nothing.

**Current "solutions" are centralized:** API rate limiting (UniswapX), capital requirements (1inch).

### 2. The Solver's Dilemma (Propagation Incentives)

**Game theory problem:**

If Solver A receives an intent:
- **Keep private:** Higher win probability, worse price for user
- **Propagate:** Lower win probability, better price for user

**Rational behavior:** Never share intents with competitors.

**Consequence:** Private Order Flow silos. Market fragmentation. Users don't get best global price.

### 3. Privacy vs. Latency

- **Public intents:** Enable MEV extraction (front-running, sandwiching)
- **Encrypted intents (ZK):** Add latency (proof generation)
- **TEE-based privacy:** Fast but hardware trust assumptions

No solution currently satisfies all requirements.

---

## Comparative Analysis

| Feature | CoW Protocol | UniswapX | 1inch Fusion | Anoma |
|---------|-------------|----------|--------------|-------|
| **Discovery** | Centralized API | API Endpoint | Staking-gated API | P2P Gossip |
| **Auction** | Batch (surplus max) | Dutch (speed) | Staked Dutch | Matchmaking |
| **Solver Access** | Whitelist + $500K bond | Open (API access) | Unicorn Power staking | Permissionless |
| **DoS Protection** | Gatekeeper | Rate limiting | Staking | Resource pricing |
| **Dominant Solvers** | Top 3 = >50% | Wintermute, Tokka | Top stakers | N/A (testnet) |

---

## Technical Primitives Required

### For Decentralized Coordination

1. **Topic-Based Gossip (libp2p + GossipSub)**
   - Segment network by intent type
   - Reputation scoring for spam prevention

2. **Stake-Based Spam Prevention**
   - Bonding pools with slashing conditions
   - Creates capital floor → solver professionalization

3. **TEEs for Privacy-Preserving Auctions**
   - SUAVE approach: encrypted intents, programmatic disclosure
   - Enables collaboration without data leakage

4. **Standardized Settlement (ERC-7683)**
   - Common order structure
   - Interoperability across protocols

---

## Implications for PubSub

### The Opportunity

A decentralized pubsub layer could:

1. **Standardize coordination** (like ERC-7683 did for settlement)
2. **Enable permissionless discovery** without centralized APIs
3. **Provide topic-based routing** for intent types
4. **Support privacy** through encryption or MPC
5. **Create propagation incentives** through tokenomics

### Design Requirements

From this research:

- **DoS resistance:** Staking or proof-of-work for submission
- **Transparency:** Auditable execution quality
- **Generality:** Support multiple intent formats
- **Privacy options:** Encrypted channels where needed

---

## Data Gaps

- **Long-tail solver profitability:** Insufficient data on whether small solvers can be economically viable
- **Anoma mainnet performance:** P2P gossip unverified under adversarial conditions
- **Bridge reorg stability:** Behavior during deep L2 reorganizations unclear

---

## References

1. [CoW Protocol Architecture](https://metalamp.io/magazine/article/cow-dao-and-cow-protocol-how-intent-based-trading-and-mev-protection-transform-defi) - MetaLamp
2. [CoW Swap Design](https://mixbytes.io/blog/modern-dex-es-how-they-re-made-cow-protocol) - MixBytes
3. [CoW Auction Competition Rules](https://docs.cow.fi/cow-protocol/reference/core/auctions/competition-rules) - CoW Docs
4. [UniswapX Dutch Auction Design](https://xangle.io/en/research/detail/1611) - Xangle Research
5. [UniswapX Filler Strategy](https://docs.uniswap.org/contracts/uniswapx/fillers/mainnet/createfiller) - Uniswap Docs
6. [1inch Fusion Deep Dive](https://li.fi/knowledge-hub/with-intents-its-solvers-all-the-way-down/) - LI.FI
7. [Anoma Architecture Specs](https://specs.anoma.net/) - Anoma
8. [ERC-7683 Cross-Chain Intents Standard](https://www.erc7683.org/spec) - ERC7683.org
9. [GlueX Solver Slashing](https://forum.cow.fi/t/cip-55-slashing-of-the-gluex-solver/2649) - CoW Forum
10. [1inch Fusion Exploit Analysis](https://www.halborn.com/blog/post/explained-the-1inch-hack-march-2025) - Halborn
11. [TEE Party with SUAVE](https://edennetwork.io/blog/tee-party-with-suave/) - Eden Network
12. [Solver Competition Analysis](http://blog.sprinter.tech/building-economic-trust-in-solver-based-networks-part-2/) - Sprinter

# Emergency Coordination Infrastructure

*Research compiled: February 2026*

!!! abstract "Summary"
    Blockchain networks build trustless systems that rely entirely on social trust during emergencies. Current coordination uses unauthenticated Web2 channels (Discord, Twitter, Telegram) with no integration into validator software. The industry needs **Decentralized Emergency Broadcast Systems (DEBS)** with authenticated alerts and automatic client response.

---

## Executive Summary

When protocol-layer mechanisms fail — infinite loops, bridge exploits, algorithmic collapses — network survival devolves to **"Layer 0": the social coordination of human operators**.

This research identifies a systemic **Authentication Gap**: emergency response relies on high-latency, unauthenticated channels susceptible to social engineering and lacking integration with validator clients.

**The paradox:** We build trustless systems that rely entirely on Discord during emergencies.

**Confidence level:** High for incident forensics and current infrastructure; Moderate for proposed solutions (untested at scale).

---

## Part I: Incident Forensics

### 1. Ronin Bridge ($624M) — The 6-Day Silence

| Phase | Event | Failure |
|-------|-------|---------|
| **Nov 2021** | Sky Mavis granted temporary signing rights for Axie DAO validator | Least privilege violation |
| **Dec 2021** | Permission expires but **not revoked** | Procedural governance failure |
| **Mar 23, 2022** | Attacker compromises 5/9 keys, drains $624M | Key centralization |
| **Mar 23-29** | **Zero alerts generated.** Bridge operates normally | **Total monitoring failure** |
| **Mar 29** | User reports failed withdrawal | Reactive discovery |

**Root cause:** No "solvency heartbeat" — no automated check that `Wrapped_ETH_on_Ronin <= Locked_ETH_on_Ethereum`.

### 2. Terra/Luna — The War Room

When UST de-pegged, coordination migrated from on-chain governance to a private Telegram "War Room."

**Leaked logs reveal:**
- Information asymmetry (validators disputed basic facts)
- Proposals for temporary oligarchy ("put five validators in charge")
- Governance bypass (security patch applied without vote)

**The dilemma:** Adhering to slow governance = chain capture. Bypassing it = eroded trust.

**Missing:** A "Martial Law Protocol" — pre-agreed rules for emergency governance acceleration.

### 3. Solana Outages — Discord Consensus Protocol

Solana restarts require manual coordination via Discord:

1. Validators notice halt via Watchtower
2. Assemble in `#mb-validators` Discord channel
3. **Paste ledger heights into chat** (manual, trust-based)
4. Core team circulates Google Doc with restart instructions
5. Wait for 80% stake to restart with exact same config

**February 2024 (JIT Cache bug):**
- 5-hour outage
- Restart at slot 246,464,040
- Coordination entirely via Discord

### 4. Ethereum Prysm Bug — The "Twitter Patch"

Post-Fusaka upgrade, Prysm nodes experienced resource exhaustion:

- Network participation dropped to ~75%
- 382 ETH lost in penalties
- **Fix disseminated via Twitter:** `--disable-last-epoch-target`

**Problems:**
- Operators asleep or not checking Twitter continued losing funds
- No way to verify @prylabs wasn't compromised
- A malicious "patch" tweet could have infected significant stake

---

## Part II: Current Infrastructure

### The Big Three Social Channels

| Channel | Usage | Vulnerability |
|---------|-------|---------------|
| **Discord** | Solana restarts, EthStaker, Cosmos | Admin compromise, no Sybil resistance |
| **Twitter** | Public alerts (@SolanaStatus, @prylabs) | SIM swapping, fake alerts for market manipulation |
| **Telegram** | Private "War Rooms" | Opacity, no cryptographic proof of decisions |

### Monitoring Tools

| Tool | Network | Capability | Limitation |
|------|---------|------------|------------|
| **Watchtower** | Solana | Alerts on delinquency/halt | Read-only, can't receive commands |
| **Beaconcha.in** | Ethereum | Mobile push for missed attestations | Centralized service |
| **Tenderduty** | Cosmos | Uptime tracking, slashing prediction | Requires manual configuration |

**Common limitation:** All are **read-only**. They tell you "the house is on fire" but can't turn on the sprinklers.

### Active Defense Systems

| System | Protocol | Mechanism |
|--------|----------|-----------|
| **Wormhole Governor** | Wormhole | Rate-limits notional value per 24h; auto-pauses on anomaly |
| **Global Accountant** | Wormhole | Ensures wrapped supply ≤ locked collateral |
| **Pre-Crime** | LayerZero | Simulates tx on forked chain; rejects if invariants fail |
| **Circuit Breaker** | Cosmos SDK | Disables specific MsgTypes without halting chain |

---

## Part III: The Three Gaps

### 1. The Authentication Gap

**Problem:** No standardized way to verify emergency alerts.

Validator sees tweet: "CRITICAL BUG — downgrade immediately."

- Is the account compromised?
- Client software can't verify the instruction
- Human verification loop introduces latency and error

**Requirement:** Authenticated broadcasts where validator clients verify cryptographic signatures from registered authorities.

### 2. The Latency Gap

**Problem:** Attacks at machine speed (400ms blocks), response at human speed.

| Incident | Detection Latency | Coordination Latency |
|----------|-------------------|---------------------|
| Ronin | 6 days | ~12 hours |
| Solana Feb '24 | ~5 minutes | ~5 hours |
| Terra | Instant (market) | ~2 days |
| Prysm bug | ~6 minutes | ~hours |

**Requirement:** Automated "reflexive" responses. Nodes self-diagnose and enter safe mode without waiting for Discord.

### 3. The Verification Gap

**Problem:** During halts, consensus is social, not cryptographic.

Validators paste ledger heights into chat. No tool for secure state gossip. Malicious actor could flood with fake data.

**Requirement:** P2P "Snapshot Consensus" tool running independently of main chain to aggregate and sign state observations.

---

## Part IV: Proposed Solutions

### Decentralized Emergency Broadcast System (DEBS)

**Architecture:**

```
[Registry Contract]
   (Authorized Signers: Client Teams, Security Council)
         |
         v
[Alert Message]
   - severity: CRITICAL
   - target: "Prysm < v5.0"
   - action: PAUSE_ATTESTATIONS
   - signature: <cryptographic proof>
         |
         v
[P2P Gossip] (libp2p, separate from tx pool)
         |
         v
[Validator Client]
   - Verify signature against Registry
   - If valid: execute action automatically
   - Log: "SYSTEM ALERT: Pausing attestations per signed directive"
```

**Result:** Emergency patches propagate in seconds, not hours. No Twitter required.

### Standardized Circuit Breakers

Every high-TVL contract should implement:
- `Pausable` interface triggered by Guardian multisig or Pre-Crime oracle
- L1 "Safe Mode" where only withdrawals process if liveness fails >1 hour

### Verified War Rooms

Replace Telegram with token-gated, signed chat:
- Login with Validator Key (proves stake ownership)
- All messages hashed and anchored to permanent storage
- Immutable "black box" for post-mortem and accountability

---

## Comparison Matrix

| Feature | Ethereum | Solana | Cosmos | Bridges |
|---------|----------|--------|--------|---------|
| **Emergency Halt** | Impossible (prioritizes liveness) | Common (cluster stop) | Protocol feature | Governance pause |
| **Coordination** | Client diversity (passive) | Discord (active) | Governance/Chat | Guardian vote |
| **Alerting** | Twitter/GitHub/Email | Discord/Watchtower | Telegram/Gov | On-chain/Governor |
| **Recovery Time** | Variable | Hours | Days (gov) or Hours | Days |
| **Weakness** | Slow patch propagation | Discord reliance | Governance latency | Centralized validators |

---

## Implications for PubSub

### Direct Alignment

PubSub can serve as the **DEBS infrastructure layer**:

1. **Authenticated broadcasts** — Signed messages from registered authorities
2. **Topic-based routing** — Emergency alerts separate from regular traffic
3. **Client integration** — Validator software subscribes to emergency topics
4. **Guaranteed delivery** — P2P gossip with persistence for offline nodes

### Use Cases

| Use Case | Current State | With PubSub |
|----------|---------------|-------------|
| Client bug alert | Twitter, hours | Signed broadcast, seconds |
| Chain halt coordination | Discord chat | Verified state gossip |
| Bridge anomaly | Manual detection | Automated alert + pause |
| Governance emergency | Telegram War Room | Auditable signed chat |

### Design Requirements

From this research:

1. **Severity levels** — INFO, WARNING, CRITICAL with different handling
2. **Target filtering** — By client, version, validator set
3. **Action payloads** — Not just text, but executable instructions
4. **Signature verification** — On-chain registry of authorized signers
5. **Independence** — Runs even when main chain is halted

---

## Data Gaps

- **Wormhole Governor effectiveness:** No public data on how many exploits it has prevented
- **Pre-Crime adoption:** Unclear how many LayerZero OApps actually use it
- **Recovery time benchmarks:** No standardized measurement across networks

---

## References

1. [Ronin Hack Analysis](https://www.halborn.com/blog/post/explained-the-ronin-hack-march-2022) - Halborn
2. [Ronin Network Exploit](https://www.merklescience.com/blog/hack-track-analysis-of-ronin-network-exploit) - Merkle Science
3. [Ronin Bridge Heist](https://www.elliptic.co/blog/540-million-stolen-from-the-ronin-defi-bridge) - Elliptic
4. [Terra Emergency Management Analysis](https://arxiv.org/abs/2207.01700) - arXiv (includes War Room analysis)
5. [Terra Emergency Management](https://arxiv.org/pdf/2207.01700) - arXiv
6. [Solana Mainnet Beta Stall Postmortem](https://solana.com/news/mainnet-beta-stall---postmortem) - Solana
7. [Solana Feb 2024 Outage Report](https://solana.com/news/02-06-24-solana-mainnet-beta-outage-report) - Solana
8. [Restarting a Solana Cluster](https://docs.solanalabs.com/operations/guides/restart-cluster) - Solana Docs
9. [Solana Validator Monitoring](https://docs.solanalabs.com/operations/best-practices/monitoring) - Solana Docs
10. [Prysm Mainnet Postmortems](https://prysm.offchainlabs.com/docs/misc/mainnet-postmortems/) - Prysm Docs
11. [Wormhole Governor Whitepaper](https://github.com/wormhole-foundation/wormhole/blob/main/whitepapers/0007_governor.md) - GitHub
12. [LayerZero Whitepaper V2](https://layerzero.network/publications/LayerZero_Whitepaper_V2.1.0.pdf) - LayerZero (includes Pre-Crime)
13. [Cosmos Circuit Breaker Module](https://docs.cosmos.network/v0.53/build/modules/circuit) - Cosmos SDK Docs

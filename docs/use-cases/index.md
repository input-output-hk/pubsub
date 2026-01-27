# Use Cases

Cardano PubSub is designed around five core use cases. These aren't hypothetical — they represent real needs from the Cardano ecosystem and drive every architectural decision.

## The Five Use Cases

| Use Case | Problem It Solves | Key Stakeholders |
|----------|-------------------|------------------|
| [DeFi Intents](defi-intents.md) | Users can't express trading intent without ADA for fees | DeFi protocols, Wallets |
| [Governance](governance.md) | Voters miss proposals, voting is high-friction | DReps, DAOs, Voters |
| [Autonomous Agents](autonomous-agents.md) | AI agents need fast, cheap coordination | DeFi bots, MEV searchers |
| [Cross-Chain](cross-chain.md) | Bridging liquidity is complex and fragmented | Partner chains, Bridges |
| [Token-Gated Social](token-gated-social.md) | Communities rely on censorable Web2 platforms | NFT projects, DAOs |

## Why These Five?

These use cases were selected because they push the **architectural extremes**:

```
                        High Reliability
                              ↑
                              │
               Governance ●   │
                              │
    Low Latency ←─────────────┼─────────────→ High Privacy
                              │
        DeFi ●                │                    ● Social
                              │
             Agents ●         │         ● Cross-Chain
                              │
                              ↓
                        High Throughput
```

| Use Case | Architectural Stress Test |
|----------|---------------------------|
| **DeFi Intents** | Latency < 500ms — if we can do DeFi, we can do anything slower |
| **Governance** | 100% delivery guarantee — if we can guarantee delivery, we can handle best-effort |
| **Agents** | 10k+ msg/sec — if we can handle agent swarms, we can handle human traffic |
| **Cross-Chain** | Foreign chain verification — if we can verify Bitcoin, we can verify anything |
| **Social** | E2EE + token gating — if we can do private groups, we can do public broadcasts |

## How to Read These Documents

Each use case document contains:

1. **Executive Summary** — What and why (for PMs and execs)
2. **Actors & Roles** — Who's involved
3. **Operational Flow** — Step-by-step scenario
4. **Technical Specifications** — Topics, payloads, requirements (for engineers)
5. **Open Questions** — What we still need to figure out

## Traceability

Each use case maps to specific requirements:

| Use Case | Functional Reqs | Non-Functional Reqs |
|----------|-----------------|---------------------|
| DeFi Intents | FR1.1, FR1.4, FR3.1, FR5.1 | NFR1.1, NFR1.2, NFR2.5 |
| Governance | FR1.3, FR2.1, FR4.2 | NFR3.1, NFR3.4, NFR4.2 |
| Agents | FR1.4, FR4.4, FR5.3 | NFR1.2, NFR1.3, NFR2.1 |
| Cross-Chain | FR3.2, FR4.4 | NFR6.2, NFR6.3 |
| Social | FR1.2, FR2.1, FR2.4 | NFR4.1, NFR4.3, NFR5.1 |

See [Requirements](../product/requirements/index.md) for full requirement definitions.

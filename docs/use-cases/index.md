# Use Cases

Cardano PubSub is designed around five core use cases. These represent real needs from the Cardano ecosystem — validated by research, incident forensics, and community feedback — and drive every architectural decision.

## The Five Use Cases

| Use Case | Problem It Solves | Key Stakeholders |
|----------|-------------------|------------------|
| [DeFi Intents](defi-intents.md) | Users can't express trading intent without ADA for fees | DeFi protocols, Wallets, Agents |
| [Governance](governance.md) | Voters miss proposals, voting is high-friction | DReps, DAOs, SPOs, Voters |
| [Network Operations](network-operations.md) | Emergency coordination relies on Discord/Twitter | SPOs, Client Teams, Security Council |
| [Cross-Chain](cross-chain.md) | Bridging liquidity is complex and fragmented | Partner chains, Bridges, Users |
| [Agent Coordination](agent-coordination.md) | Automated systems need fast, cheap coordination | Keepers, Searchers, Protocols |

## Why These Five?

These use cases were selected because they:

1. **Have documented evidence** — Real incidents, measured losses, or proven demand
2. **Push architectural extremes** — Each stresses the system differently
3. **Serve distinct stakeholders** — From end users to protocol operators
4. **Enable Cardano's roadmap** — DeFi Intents, CIP-1694 governance, cross-chain growth

```
                        High Reliability
                              ↑
                              │
          Network Ops ●       │       ● Governance
                              │
    Low Latency ←─────────────┼─────────────→ High Throughput
                              │
              DeFi ●          │          ● Agents
                              │
                              │       ● Cross-Chain
                              ↓
                        Complex Verification
```

| Use Case | Architectural Stress Test |
|----------|---------------------------|
| **DeFi Intents** | Latency <500ms — if we can do DeFi, we can do anything slower |
| **Governance** | 99.99% delivery — if we can guarantee delivery, we can handle best-effort |
| **Network Operations** | Works during chain halts — if we can operate independently, we're resilient |
| **Cross-Chain** | Foreign chain verification — if we can verify Bitcoin proofs, we can verify anything |
| **Agent Coordination** | 10k+ msg/sec — if we can handle agent swarms, we can handle human traffic |

## How to Read These Documents

Each use case document contains:

1. **The Problem** — What pain point exists today
2. **The Solution** — How PubSub addresses it
3. **Actors & Scenarios** — Who's involved, step-by-step flows
4. **Technical Specification** — Topics, schemas, requirements
5. **Open Questions** — What we still need to figure out

## Traceability

Each use case maps to specific requirements:

| Use Case | Functional Reqs | Non-Functional Reqs |
|----------|-----------------|---------------------|
| DeFi Intents | FR1.1, FR1.4, FR3.1, FR5.1 | NFR1.1, NFR1.2, NFR2.5 |
| Governance | FR1.3, FR2.1, FR4.2 | NFR3.1, NFR3.4, NFR4.2 |
| Network Operations | FR1.3, FR2.1, FR4.2 | NFR3.1, NFR3.4, NFR6.1 |
| Cross-Chain | FR3.2, FR4.4 | NFR6.2, NFR6.3 |
| Agent Coordination | FR1.4, FR4.4, FR5.3 | NFR1.2, NFR1.3, NFR2.1 |

See [Requirements](../product/requirements/index.md) for full requirement definitions.

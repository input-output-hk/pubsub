# Non-Functional Requirements

!!! info "Benchmark Reference"
    Compare with existing systems benchmarks like [Waku Documentation](https://docs.waku.org/)

## Performance

| ID | Requirement | Rationale | Use Cases |
|----|-------------|-----------|-----------|
| NFR1.1 | Message latency < 500ms for direct peer connections (P95) | DeFi and agent coordination require near-instant delivery | [DeFi Intents](../../use-cases/defi-intents.md) (architectural stress test), [Autonomous Agents](../../use-cases/autonomous-agents.md) |
| NFR1.2 | Support minimum 10,000 messages per second per node | Agent swarms generate high message volumes during market events | [Autonomous Agents](../../use-cases/autonomous-agents.md) (architectural stress test) |
| NFR1.3 | Handle 1,000+ concurrent connections per node | Relay nodes must serve many peers; SPO coordinator nodes especially | [Autonomous Agents](../../use-cases/autonomous-agents.md), [DeFi Intents](../../use-cases/defi-intents.md) |
| NFR1.4 | Message delivery success rate > 99.9% for online peers | Financial intents and votes cannot be lost | [DeFi Intents](../../use-cases/defi-intents.md), [Governance](../../use-cases/governance.md) |
| NFR1.5 | Protocol overhead < 10% of message payload | Mobile wallets and bandwidth-constrained users need lean protocols | [Token-Gated Social](../../use-cases/token-gated-social.md), [DeFi Intents](../../use-cases/defi-intents.md) |

## Scalability

| ID | Requirement | Rationale | Use Cases |
|----|-------------|-----------|-----------|
| NFR2.1 | Support network growth to 1 million+ nodes | Mass adoption requires protocol to scale with Cardano ecosystem growth | All use cases (long-term viability) |
| NFR2.2 | Maintain performance with 100,000+ topics | Each DAO, NFT project, and agent type needs topic namespacing | [Token-Gated Social](../../use-cases/token-gated-social.md), [Governance](../../use-cases/governance.md) |
| NFR2.3 | Enable horizontal scaling through sharding | Single-shard designs hit limits; sharding allows organic growth | All use cases (architectural foundation) |
| NFR2.4 | Support dynamic load balancing | Traffic patterns shift (market events, proposal deadlines) | [DeFi Intents](../../use-cases/defi-intents.md), [Governance](../../use-cases/governance.md) |
| NFR2.5 | Handle traffic spikes of 10x normal load | Market crashes, viral governance proposals, NFT drops cause surges | [DeFi Intents](../../use-cases/defi-intents.md), [Autonomous Agents](../../use-cases/autonomous-agents.md), [Token-Gated Social](../../use-cases/token-gated-social.md) |

## Reliability

| ID | Requirement | Rationale | Use Cases |
|----|-------------|-----------|-----------|
| NFR3.1 | 99.9% uptime for core protocol functions | Developers and protocols need dependable infrastructure | All use cases (table stakes) |
| NFR3.2 | Automatic failover and recovery mechanisms | Manual intervention doesn't scale; system must self-heal | [DeFi Intents](../../use-cases/defi-intents.md) (24/7 markets), [Autonomous Agents](../../use-cases/autonomous-agents.md) |
| NFR3.3 | No single point of failure in the network | Decentralization promise; central points get attacked or censored | [Token-Gated Social](../../use-cases/token-gated-social.md) (censorship resistance), [Governance](../../use-cases/governance.md) |
| NFR3.4 | Message persistence for 30 days minimum | Governance proposals need multi-week voting windows; offline users need catch-up | [Governance](../../use-cases/governance.md) (7-day voting), [Token-Gated Social](../../use-cases/token-gated-social.md) |
| NFR3.5 | Graceful degradation under adverse conditions | Partial functionality beats total failure during attacks or high load | All use cases (resilience) |

## Security

| ID | Requirement | Rationale | Use Cases |
|----|-------------|-----------|-----------|
| NFR4.1 | Resist Sybil attacks through proof-of-work or stake | Open networks are vulnerable to fake-node floods; economic cost deters attackers | All use cases (network integrity) |
| NFR4.2 | Prevent message replay attacks | Replayed intents can cause double-execution; replayed votes corrupt results | [DeFi Intents](../../use-cases/defi-intents.md), [Governance](../../use-cases/governance.md) |
| NFR4.3 | Cryptographic primitives meeting NIST standards | Industry-accepted security baseline; required for enterprise adoption | All use cases (trust foundation) |
| NFR4.4 | Regular security audits by third parties | External audits catch blind spots; essential for financial use cases | [DeFi Intents](../../use-cases/defi-intents.md), [Cross-Chain](../../use-cases/cross-chain.md) |
| NFR4.5 | Vulnerability disclosure and patching process < 30 days | Fast response limits exploit windows; responsible disclosure standard | All use cases (operational security) |

## Usability

| ID | Requirement | Rationale | Use Cases |
|----|-------------|-----------|-----------|
| NFR5.1 | SDK integration time < 1 hour for basic functionality | Developer experience drives adoption; friction kills momentum | All use cases (adoption enabler) |
| NFR5.2 | Documentation coverage for 100% of public APIs | Undocumented APIs cause frustration and misuse | All use cases (developer experience) |
| NFR5.3 | Example applications for common use cases | Working code beats documentation; devs learn by example | [Token-Gated Social](../../use-cases/token-gated-social.md) (chat app), [DeFi Intents](../../use-cases/defi-intents.md) (intent submitter) |
| NFR5.4 | Error messages with actionable solutions | "Error 500" helps no one; messages should guide resolution | All use cases (developer experience) |
| NFR5.5 | Backwards compatibility for 2 major versions | Breaking changes kill production apps; wallets need migration windows | [DeFi Intents](../../use-cases/defi-intents.md), [Governance](../../use-cases/governance.md) |

## Interoperability

| ID | Requirement | Rationale | Use Cases |
|----|-------------|-----------|-----------|
| NFR6.1 | Support standard cryptographic libraries | Custom crypto is risky; leverage battle-tested implementations (MLS, libp2p) | [Token-Gated Social](../../use-cases/token-gated-social.md) (MLS encryption) |
| NFR6.2 | Compatible with major blockchain networks | Primary value prop: connect Cardano to Bitcoin, Ethereum, partner chains | [Cross-Chain](../../use-cases/cross-chain.md) (architectural stress test) |
| NFR6.3 | Bridge capability with other messaging protocols | Ecosystem play; interop with Matrix, XMPP expands reach | [Token-Gated Social](../../use-cases/token-gated-social.md) |
| NFR6.4 | Standard data formats (JSON, Protobuf) | Reduces parsing burden; tooling exists for standard formats | All use cases (interoperability) |
| NFR6.5 | Cross-platform support (Linux, Windows, macOS, Mobile) | Users are everywhere; mobile especially critical for wallets | [Token-Gated Social](../../use-cases/token-gated-social.md), [Governance](../../use-cases/governance.md) (mobile voting) |

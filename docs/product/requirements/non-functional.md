# Non-Functional Requirements

!!! info "Benchmark Reference"
    Compare with existing systems benchmarks like [Waku Documentation](https://docs.waku.org/)

## Performance

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR1.1 | Message latency < 500ms for direct peer connections (P95) | Chat apps require near-instant delivery; >500ms feels laggy to users |
| NFR1.2 | Support minimum 10,000 messages per second per node | High-throughput use cases (trading signals, IoT telemetry) need this capacity |
| NFR1.3 | Handle 1,000+ concurrent connections per node | Relay nodes must serve many peers simultaneously; limits adoption if too low |
| NFR1.4 | Message delivery success rate > 99.9% for online peers | Unreliable messaging breaks trust; critical for financial/transactional use cases |
| NFR1.5 | Protocol overhead < 10% of message payload | Bandwidth-constrained environments (mobile, IoT) can't afford bloated protocols |

## Scalability

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR2.1 | Support network growth to 1 million+ nodes | Mass adoption target; protocol must not collapse at scale (see: early BitTorrent DHT issues) |
| NFR2.2 | Maintain performance with 100,000+ topics | dApps and large platforms need topic namespacing without degradation |
| NFR2.3 | Enable horizontal scaling through sharding | Single-shard designs hit limits; sharding allows network to grow organically |
| NFR2.4 | Support dynamic load balancing | Traffic patterns shift; nodes must adapt without manual intervention |
| NFR2.5 | Handle traffic spikes of 10x normal load | Viral events, market crashes, breaking news cause sudden surges |

## Reliability

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR3.1 | 99.9% uptime for core protocol functions | Developers won't build on unreliable infrastructure; table-stakes for production use |
| NFR3.2 | Automatic failover and recovery mechanisms | Manual intervention doesn't scale; system must self-heal |
| NFR3.3 | No single point of failure in the network | Decentralization promise; central points get attacked, censored, or fail |
| NFR3.4 | Message persistence for 30 days minimum | Store-and-forward (FR1.3) needs meaningful retention; covers typical offline durations |
| NFR3.5 | Graceful degradation under adverse conditions | Partial functionality beats total failure; users tolerate slowness better than errors |

## Security

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR4.1 | Resist Sybil attacks through proof-of-work or stake | Open networks are vulnerable to fake-node floods; economic cost deters attackers |
| NFR4.2 | Prevent message replay attacks | Replayed messages can cause double-spends, duplicate actions, or confusion |
| NFR4.3 | Cryptographic primitives meeting NIST standards | Industry-accepted security baseline; required for enterprise/regulated adoption |
| NFR4.4 | Regular security audits by third parties | Internal review has blind spots; external audits build trust and catch vulnerabilities |
| NFR4.5 | Vulnerability disclosure and patching process < 30 days | Fast response limits exploit windows; industry expectation for responsible disclosure |

## Usability

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR5.1 | SDK integration time < 1 hour for basic functionality | Developer experience drives adoption; friction = abandonment (use case: hackathon projects) |
| NFR5.2 | Documentation coverage for 100% of public APIs | Undocumented APIs cause frustration and misuse; devs won't guess |
| NFR5.3 | Example applications for common use cases | Working code beats documentation; devs learn by copying (chat app, notification service) |
| NFR5.4 | Error messages with actionable solutions | "Error 500" helps no one; messages should guide resolution |
| NFR5.5 | Backwards compatibility for 2 major versions | Breaking changes kill production apps; migration windows needed |

## Interoperability

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR6.1 | Support standard cryptographic libraries | Custom crypto is risky and hard to audit; leverage battle-tested implementations |
| NFR6.2 | Compatible with major blockchain networks | Primary use case: Web3 dApps need to integrate with Ethereum, Cardano, etc. |
| NFR6.3 | Bridge capability with other messaging protocols | Ecosystem play; users shouldn't be locked into one protocol (Matrix, XMPP bridges) |
| NFR6.4 | Standard data formats (JSON, Protobuf) | Reduces parsing burden; tooling already exists for standard formats |
| NFR6.5 | Cross-platform support (Linux, Windows, macOS, Mobile) | Users are everywhere; platform exclusion limits addressable market |

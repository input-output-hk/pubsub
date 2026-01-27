# Requirements

!!! info "Audience: Product Managers, Engineers"

This section outlines the functional and non-functional requirements for Cardano PubSub.

## Sections

- [Functional Requirements](functional.md) — What the system does
- [Non-Functional Requirements](non-functional.md) — How well the system performs

## Requirements Traceability

Each requirement is traced to the [Architectural Drivers](../../architecture/drivers/index.md) that depend on it.

### Functional Requirements Coverage

| Req ID | Description | Drivers |
|--------|-------------|---------|
| FR1.1 | Point-to-point encrypted messaging | DEF-01, SOC-01 |
| FR1.2 | Group messaging with privacy levels | SOC-01 |
| FR1.3 | Store-and-forward for offline delivery | GOV-01 |
| FR1.4 | Multiple message types (text, binary, structured) | DEF-01, AI-01 |
| FR1.5 | Message history retrieval | GOV-01, SOC-01 |
| FR2.1 | End-to-end encryption | GOV-01, SOC-01 |
| FR2.2 | Anonymous messaging | SOC-01 |
| FR2.3 | Traffic obfuscation | SOC-01 |
| FR2.4 | Metadata privacy protection | SOC-01 |
| FR2.5 | Ephemeral messages with TTL | DEF-01 |
| FR3.1 | Automatic peer discovery | DEF-01 |
| FR3.2 | Multiple transport protocols | XCB-01 |
| FR3.3 | DHT-based content routing | GOV-01 |
| FR3.4 | NAT traversal | All |
| FR3.5 | Relay and direct connections | All |
| FR4.1 | SDKs (JS, Go, Rust, Python) | All |
| FR4.2 | REST and WebSocket APIs | GOV-01 |
| FR4.3 | Message filtering | All |
| FR4.4 | Custom protocol extensions | AI-01, XCB-01 |
| FR4.5 | Debugging and monitoring tools | All |
| FR5.1 | Adaptive rate limiting | DEF-01 |
| FR5.2 | Bandwidth management | AI-01 |
| FR5.3 | Selective message relaying | AI-01 |
| FR5.4 | Storage quota management | SOC-01 |
| FR5.5 | Light client protocols | All |

### Non-Functional Requirements Coverage

| Req ID | Description | Drivers |
|--------|-------------|---------|
| NFR1.1 | Latency <500ms (P95) | DEF-01 |
| NFR1.2 | 10,000 messages/second/node | DEF-01, AI-01 |
| NFR1.3 | 1,000+ concurrent connections | AI-01 |
| NFR1.4 | >99.9% delivery success | GOV-01 |
| NFR1.5 | <10% protocol overhead | All |
| NFR2.1 | Scale to 1M+ nodes | All |
| NFR2.2 | 100,000+ topics | SOC-01 |
| NFR2.3 | Horizontal scaling (sharding) | SOC-01 |
| NFR2.4 | Dynamic load balancing | All |
| NFR2.5 | 10x traffic spike handling | DEF-01 |
| NFR3.1 | 99.9% uptime | GOV-01 |
| NFR3.2 | Automatic failover | GOV-01 |
| NFR3.3 | No single point of failure | All |
| NFR3.4 | 30-day message persistence | GOV-01 |
| NFR3.5 | Graceful degradation | All |
| NFR4.1 | Sybil attack resistance | AI-01 |
| NFR4.2 | Replay attack prevention | All |
| NFR4.3 | NIST-compliant crypto | SOC-01 |
| NFR4.4 | Third-party security audits | All |
| NFR4.5 | <30 day vulnerability patching | All |
| NFR5.1 | <1 hour SDK integration | All |
| NFR5.2 | 100% API documentation | All |
| NFR5.3 | Example applications | All |
| NFR5.4 | Actionable error messages | All |
| NFR5.5 | 2-version backwards compatibility | All |
| NFR6.1 | Standard crypto libraries | All |
| NFR6.2 | Major blockchain compatibility | XCB-01 |
| NFR6.3 | Protocol bridging | XCB-01 |
| NFR6.4 | Standard data formats (JSON, Protobuf) | All |
| NFR6.5 | Cross-platform support | All |

## Validation Status

!!! note "Requirements Validation"
    These requirements are derived from architectural analysis and stakeholder input. They will be validated through:
    
    1. **Stakeholder review** — Lace, Midnight, Identus teams
    2. **Technical feasibility** — Engineering assessment
    3. **Prototype testing** — Beacon MVP validation

# Requirements

!!! info "Audience: Product Managers, Engineers"

This section outlines the functional and non-functional requirements for Cardano PubSub.

## Sections

- [Functional Requirements](functional.md) — What the system does
- [Non-Functional Requirements](non-functional.md) — How well the system performs

## Requirements Traceability

Each requirement is traced to the [Use Cases](../../use-cases/index.md) that depend on it.

### Functional Requirements by Use Case

| Use Case | Key Requirements |
|----------|------------------|
| [DeFi Intents](../../use-cases/defi-intents.md) | FR1.1, FR1.4, FR3.1, FR5.1 |
| [Governance](../../use-cases/governance.md) | FR1.3, FR2.1, FR4.2 |
| [Network Operations](../../use-cases/network-operations.md) | FR1.3, FR2.1, FR4.2 |
| [Cross-Chain](../../use-cases/cross-chain.md) | FR3.2, FR4.4 |
| [Agent Coordination](../../use-cases/agent-coordination.md) | FR1.4, FR4.4, FR5.3 |

### Non-Functional Requirements by Use Case

| Use Case | Key Requirements |
|----------|------------------|
| [DeFi Intents](../../use-cases/defi-intents.md) | NFR1.1, NFR1.2, NFR2.5 |
| [Governance](../../use-cases/governance.md) | NFR3.1, NFR3.4, NFR4.2 |
| [Network Operations](../../use-cases/network-operations.md) | NFR3.1, NFR3.4, NFR6.1 |
| [Cross-Chain](../../use-cases/cross-chain.md) | NFR6.2, NFR6.3 |
| [Agent Coordination](../../use-cases/agent-coordination.md) | NFR1.2, NFR1.3, NFR2.1 |

## Validation Status

!!! note "Requirements Validation"
    These requirements are derived from use case analysis and stakeholder input. They will be validated through:
    
    1. **Stakeholder review** — Wallet teams, DeFi protocols, SPOs
    2. **Technical feasibility** — Engineering assessment
    3. **Prototype testing** — Early prototype validation

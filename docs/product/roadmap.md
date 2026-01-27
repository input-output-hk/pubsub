# Roadmap

!!! info "Audience: All stakeholders"

We follow a **lean, phased approach** focused on rapid prototyping (~1 month per prototype) to iterate quickly with stakeholder feedback.

---

## Phase 1: Architecture & Prototyping

**Goal:** Validate architecture and build working prototypes.

- Finalize architecture design
- Hire Tech Lead, complete vendor selection
- Build working P2P prototype with Identus integration
- Produce SDK draft for early integrators

**Prototypes:**

| Prototype | Focus |
|-----------|-------|
| P1: Networking | Ouroboros + GossipSub hybrid |
| P2: Identity | Identus DID integration |
| P3: Storage | Tiered storage (hot/durable) |
| P4: Integration | Full stack prototype |

---

## Phase 2: PubSub Network

**Goal:** Launch decentralized, SPO-operated messaging network.

- Deploy SPO testnet, tune performance, complete security audit
- Launch production mainnet
- Ship wallet integrations and developer documentation

---

## Phase 3: Full Economy

**Goal:** Sustainable economic model with community governance.

- Publish tokenomics RFC and gather community feedback
- Design governance framework
- Launch economic incentives and establish DAO

---

## Dependencies

| Dependency | Owner | Risk | Mitigation |
|------------|-------|------|------------|
| Identus DID resolution | Identus Team | Low | Internal team, stable API |
| SPO participation | Community | Medium | Economic incentives, outreach |
| Wallet integration | Wallet teams | Medium | Early engagement, simple SDK |
| Plutus Events (Leios) | Core Team | Medium | Not required for initial launch |

---

## Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2025-01 | Decentralized-first approach | Build for long-term, SPO-powered network |
| 2025-01 | Identus for identity | Ecosystem alignment, less maintenance |
| 2025-01 | Ouroboros-native networking | SPO adoption, native feel |
| TBD | Economic model (token vs. ADA-only) | Pending Phase 3 research |

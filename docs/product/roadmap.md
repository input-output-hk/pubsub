# Roadmap

!!! info "Audience: All stakeholders"

We follow a **lean, phased approach** focused on rapid prototyping (~1 month per prototype) to iterate quickly with stakeholder feedback.

---

## Phase 1: Architecture & Prototyping

**Goal:** Validate architecture and build working prototypes.

- Finalize architecture design
- Hire Tech Lead, complete vendor selection
- Build working P2P prototype with modular DID integration
- Produce SDK draft for early integrators

**Prototypes:**

| Prototype | Focus |
|-----------|-------|
| P1: Networking | Three-layer protocol (SecureCyclon + Vicinity + Hybrid Dissemination) |
| P2: Identity | DID Resolver Mesh (did:key → did:prism → did:pkh) |
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
| DID resolution (Identus, did:pkh) | Identus Team / Veramo | Low | Modular design reduces single-vendor risk |
| SPO participation | Community | Medium | Economic incentives, outreach |
| Wallet integration | Wallet teams | Medium | Early engagement, simple SDK |
| Plutus Events (Leios) | Core Team | Medium | Not required for initial launch |

---

## Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2025-01 | Decentralized-first approach | Build for long-term, SPO-powered network |
| 2025-01 | Modular DID identity (Identus as premier plugin) | Chain-agnostic design, no vendor lock-in |
| 2025-01 | Ouroboros-native networking | SPO adoption, native feel |
| TBD | Economic model (token vs. ADA-only) | Pending Phase 3 research |

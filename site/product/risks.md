# Risks & Asks

!!! info "Audience: Executives, Decision Makers"

## Risk Register

### Technical Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **Technical Complexity** | High | High | Phased approach; proven tech choices (MLS, RocksDB) |
| **Ouroboros Integration Challenges** | Medium | High | Early prototyping with TXpipe/Anastasia Labs; IOG Research protocols provide proven foundation |
| **Identity Provider Dependency** | Low | Medium | Modular DID Resolver Mesh supports multiple methods; no single-vendor lock-in |
| **Performance Targets Unmet** | Medium | Medium | Conservative initial targets; benchmark early |

### Delivery Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **Vendor Delivery Issues** | Medium | High | Multiple vendor options (TXpipe, Anastasia, Sundae); clear milestones |
| **Team Scaling** | Medium | Medium | Tech Lead hire is critical path; recruiting started |
| **Protocol Dependencies** | Low | High | Plutus Events not required for initial phases |

### Adoption Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **Wallet Integration Delays** | Medium | High | Early engagement; simple SDK; integration support |
| **SPO Adoption < 50 nodes** | Low | Medium | Leverage existing relationships; economic incentives in Phase 3 |
| **Competing Solution Emerges** | Low | Medium | First-mover advantage; native integration is hard to replicate |

### Contingency Scenarios

| Scenario | Trigger | Response |
|----------|---------|----------|
| **SPO adoption fails** | <30 nodes after 6 months | Hybrid model with professional relay operators |
| **Identus pivots away** | Major roadmap change | Modular design allows seamless switch to did:pkh / did:key / did:web |
| **Wallet adoption slow** | <3 integrations after 1 year | Focus on dApp integrations instead |

---

## Asks

### Engineering Resources

| Ask | Rationale | Timeline |
|-----|-----------|----------|
| **Tech Lead hire** | Critical for architecture decisions | By Q1 2025 |
| **2-3 senior engineers** | Core development | By Q2 2025 |
| **Vendor budget (TXpipe/Anastasia)** | Ouroboros integration expertise | Q1 2025 |

### Protocol-Level Support

| Ask | Rationale | Owner |
|-----|-----------|-------|
| **Plutus Events in Leios roadmap** | Enables on-chain triggers for notifications | Core team |
| **Identity provider partnerships** | DID integration support (Identus, Veramo) | Product |
| **SPO communication channel** | Adoption outreach | Community |

### Stakeholder Commitments

| Stakeholder | Commitment Needed | Status |
|-------------|-------------------|--------|
| **Wallet teams** | Integration commitment, timeline | ⬜ To engage |
| **DeFi protocols** | Use case validation, early adoption | ⬜ To engage |
| **Governance Team** | CIP-1694 integration requirements | ⬜ To engage |

---

## PRDs & Resources

| Document | Status | Link |
|----------|--------|------|
| Cardano PubSub Product Requirements | ✅ Draft Complete | [View](index.md) |
| Market Requirements Document | ✅ Draft Complete | [View](market.md) |
| Architecture Design | 🟡 In Progress | [View](../architecture/index.md) |
| AUEB Pub/Sub Research Paper | ✅ Available | *Internal* |

---

## Marketing & Communications Plan

### Phase 1: Development (2025)

| Activity | Audience | Owner | Timeline |
|----------|----------|-------|----------|
| Technical blog: "Why Cardano Needs Native Messaging" | Developers, Community | PM | Q1 2025 |
| Vendor/contributor Discord server | Engineers | PM | Q1 2025 |
| SPO outreach (informal) | Operators | Community | Ongoing |

### Phase 2: Launch (2026)

| Activity | Audience | Owner | Timeline |
|----------|----------|-------|----------|
| Mainnet launch announcement | All | Marketing | At launch |
| Integration tutorial series | Developers | DevRel | At launch |
| SPO onboarding program | Operators | Community | Pre-launch |

### Phase 3: Growth (2026+)

| Activity | Audience | Owner | Timeline |
|----------|----------|-------|----------|
| Hackathon sponsorships | Developers | DevRel | Ongoing |
| Ecosystem partnerships | Projects | BD | Ongoing |
| Community governance launch | All | PM | Q4 2026 |

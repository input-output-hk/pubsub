# Risks & Asks

!!! info "Audience: Executives, Decision Makers"

## Risk Register

### Technical Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **Technical Complexity** | High | High | Phased approach (Beacon first); proven tech choices (MLS, RocksDB) |
| **Ouroboros Integration Challenges** | Medium | High | Early prototyping with TXpipe/Anastasia Labs; fallback to libp2p-only if needed |
| **Identus Dependency** | Medium | Medium | Maintain abstraction layer; Identus team is internal to IOG |
| **Performance Targets Unmet** | Medium | Medium | Conservative initial targets; benchmark early in Phase 1 |

### Delivery Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **Midnight Deadline Pressure** | High | High | Beacon is intentionally minimal; scope can be reduced further |
| **Vendor Delivery Issues** | Medium | High | Multiple vendor options (TXpipe, Anastasia, Sundae); clear milestones |
| **Team Scaling** | Medium | Medium | Tech Lead hire is critical path; recruiting started |
| **Protocol Dependencies** | Low | High | Plutus Events in Leios; executive escalation path defined |

### Adoption Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **Wallet Integration Delays** | Medium | High | Lace is internal; early engagement; simple SDK |
| **SPO Adoption < 50 nodes** | Low | Medium | Leverage existing relationships; economic incentives in Phase 3 |
| **Competing Solution Emerges** | Low | Medium | First-mover advantage; native integration is hard to replicate |

### Contingency Scenarios

| Scenario | Trigger | Response |
|----------|---------|----------|
| **Midnight delays 6+ months** | Mainnet pushed to 2026 | Pivot Beacon to Cardano governance use case |
| **SPO adoption fails** | <30 nodes after 6 months | Hybrid model with professional relay operators |
| **Identus pivots away** | Major roadmap change | Fork or migrate to did:web / did:key |

---

## Migration Risk: Beacon → PubSub

!!! warning "Critical Path"
    The Beacon-to-PubSub migration is the highest-risk technical transition. Failure means breaking Midnight integrations.

### Migration Strategy

| Phase | Action | Risk Level |
|-------|--------|------------|
| **1. API Freeze** | Lock Beacon APIs before Midnight launch | ✅ Low |
| **2. Shadow Network** | Run PubSub alongside Beacon, mirroring traffic | 🟡 Medium |
| **3. Gradual Cutover** | Route increasing % of traffic to PubSub | 🟡 Medium |
| **4. Beacon Gateway** | Keep Beacon as API gateway to PubSub backend | ✅ Low |
| **5. Deprecation** | 12-month notice before Beacon-only endpoints removed | ✅ Low |

### Compatibility Guarantees

| Guarantee | Description |
|-----------|-------------|
| **API Stability** | Beacon REST/WebSocket APIs supported for 24 months post-PubSub launch |
| **Topic Compatibility** | Same topic structure in both systems |
| **Identity Portability** | Identus DIDs work identically |
| **Message Format** | Beacon payloads valid in PubSub |

---

## Asks

### Engineering Resources

| Ask | Rationale | Timeline |
|-----|-----------|----------|
| **Tech Lead hire** | Critical for architecture decisions | By Feb 2025 |
| **2-3 senior engineers** | Core Beacon development | By Mar 2025 |
| **Vendor budget (TXpipe/Anastasia)** | Ouroboros integration expertise | Q1 2025 |

### Protocol-Level Support

| Ask | Rationale | Owner |
|-----|-----------|-------|
| **Plutus Events in Leios roadmap** | Enables on-chain triggers for notifications | Core team |
| **Identus partnership formalization** | DID integration support | Product |
| **SPO communication channel** | Adoption outreach | Community |

### Stakeholder Commitments

| Stakeholder | Commitment Needed | Status |
|-------------|-------------------|--------|
| **Midnight Team** | API requirements, launch timeline | 🟡 In Progress |
| **Lace Team** | Integration commitment, design resources | 🟡 In Progress |
| **Governance Team** | CIP-1694 integration requirements | ⬜ Not Started |

---

## PRDs & Resources

| Document | Status | Link |
|----------|--------|------|
| Beacon PRD | 🟡 In Progress | [View](../beacon/index.md) |
| Cardano PubSub Product Requirements | ✅ Draft Complete | [View](index.md) |
| Market Requirements Document | ✅ Draft Complete | [View](market.md) |
| AUEB Pub/Sub Research Paper | ✅ Available | *Internal* |
| Charles Hoskinson Feedback | ✅ Incorporated | *Internal* |

---

## Marketing & Communications Plan

### Phase 1: Beacon Development (Now - Q2 2025)

| Activity | Audience | Owner | Timeline |
|----------|----------|-------|----------|
| Technical blog: "Why Cardano Needs Native Messaging" | Developers, Community | PM | Feb 2025 |
| Vendor/contributor Discord server | Engineers | PM | Jan 2025 |
| SPO outreach (informal) | Operators | Community | Ongoing |

### Phase 2: Beacon Launch (Q3 2025)

| Activity | Audience | Owner | Timeline |
|----------|----------|-------|----------|
| Launch announcement | All | Marketing | At launch |
| Integration tutorial series | Developers | DevRel | At launch |
| Midnight partnership announcement | Press | Marketing | At launch |

### Phase 3: PubSub Network (2026)

| Activity | Audience | Owner | Timeline |
|----------|----------|-------|----------|
| Decentralization announcement | All | Marketing | At launch |
| SPO onboarding program | Operators | Community | Pre-launch |
| Hackathon sponsorships | Developers | DevRel | Ongoing |

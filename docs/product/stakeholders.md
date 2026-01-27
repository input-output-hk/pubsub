# Stakeholders & Team

!!! info "Audience: All stakeholders"

## Core Team

| Role | Name | Status |
|------|------|--------|
| **Product Manager** | @Reza Baram | ✅ Active |
| **Tech Lead** | *Hiring* | 🟡 Target: Feb 2025 |
| **Senior Engineers** | *Hiring 2-3* | 🟡 Target: Mar 2025 |

## Key Stakeholders

### Lace Team (Wallet Integration)

| Role | Contact | Engagement |
|------|---------|------------|
| Tech Leads | @Piotr Czeglik / @Rhys | 🟡 Active discussions |
| Designer | @Allan Leone | 🟡 Notification UX design |

**Key Decisions Needed:**

- Notification UI/UX patterns
- SDK integration approach
- Push notification infrastructure (FCM/APNs vs. WebSocket)

### Midnight Team (Launch Customer)

| Role | Contact | Engagement |
|------|---------|------------|
| Product Lead | *To be confirmed* | 🟡 Scheduling intro |
| Tech Lead | *To be confirmed* | 🟡 Scheduling intro |

**Key Decisions Needed:**

- Notification message format
- Launch timeline confirmation
- SLA requirements

### Identus Team (Identity Infrastructure)

| Role | Contact | Engagement |
|------|---------|------------|
| Tech Lead | *Via Atala team* | ⬜ To be engaged Feb 2025 |

**Key Decisions Needed:**

- DID resolution API stability
- Verifiable Credential support timeline
- Integration support level

## Potential Vendors

*Recommended by Charles Hoskinson for Ouroboros networking expertise:*

| Vendor | Expertise | Engagement Status |
|--------|-----------|-------------------|
| **TXpipe (Phil)** | Deep Cardano network stack knowledge; building alternative node implementation | ⬜ To contact Q1 2025 |
| **Anastasia Labs (Santiago)** | Building Midgard; strong networking components | ⬜ To contact Q1 2025 |
| **Sundae Labs (Pi)** | Involved in Leios; protocol expertise | ⬜ To contact Q1 2025 |

**Vendor Selection Timeline:**

| Milestone | Target |
|-----------|--------|
| Initial outreach | Jan 2025 |
| Technical discussions | Feb 2025 |
| Vendor selection | Mar 2025 |
| Contract signed | Apr 2025 |

---

## Metrics and KPIs

### Phase 1: Beacon (2025)

| Category | Metric | Target | Measurement |
|----------|--------|--------|-------------|
| **Delivery** | Beacon MVP | May 2025 | Ship date |
| **Delivery** | Midnight integration | Sep 2025 | Launch support |
| **Quality** | Message delivery rate | >99.9% | Monitoring |
| **Quality** | API latency (p99) | <500ms | Monitoring |
| **Adoption** | Lace integration | Complete | Binary |

### Phase 2: PubSub Network (2026)

| Category | Metric | Target | Measurement |
|----------|--------|--------|-------------|
| **Network Health** | Active SPO relay nodes | 100+ | Node count |
| **Adoption** | Wallet integrations | 5+ | Count |
| **Adoption** | dApp integrations | 30+ | Count |
| **Technical** | Message latency (p99) | <1 second | Monitoring |
| **Technical** | Messages per day | 1M+ | Logs |

### Phase 3: Full Economy (2027)

| Category | Metric | Target | Measurement |
|----------|--------|--------|-------------|
| **Economic** | SPO participation rate | >10% of active SPOs | Registry |
| **Economic** | Fee revenue (monthly) | Sustainable ops | Treasury |
| **Governance** | DAO participation | >50 voters | Voting records |

---

## Success Factors

| Factor | Status | Owner | Target Date |
|--------|--------|-------|-------------|
| **Meeting Midnight Deadline** | 🟡 On Track | PM | Sep 2025 |
| **Lace Integration Complete** | ⬜ Not Started | Engineering | Aug 2025 |
| **Identus DID Standard Adopted** | ⬜ Not Started | PM | Jun 2025 |
| **SPO Testnet (50+ nodes)** | ⬜ Not Started | Engineering | Jun 2026 |
| **Seamless Beacon→PubSub Migration** | ⬜ Not Started | Engineering | Nov 2026 |

---

## Communication Channels

| Channel | Purpose | Audience |
|---------|---------|----------|
| **#pubsub-internal** (Slack) | Core team coordination | Team only |
| **#pubsub-stakeholders** (Slack) | Stakeholder updates | Lace, Midnight, Identus |
| **Discord server** (public) | Community, vendors, contributors | Public |
| **Weekly sync** (30 min) | Status update | Core team + stakeholders |
| **Monthly review** (1 hr) | Progress, blockers, decisions | Executives |

---

## RACI Matrix

| Decision | Responsible | Accountable | Consulted | Informed |
|----------|-------------|-------------|-----------|----------|
| PRD approval | PM | PM | Stakeholders | All |
| Architecture decisions | Tech Lead | PM | Vendors | Team |
| Vendor selection | PM | PM | Tech Lead, Finance | Team |
| API design | Tech Lead | PM | Lace, Midnight | All |
| Launch go/no-go | PM | Executive | All stakeholders | All |

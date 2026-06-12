# Specification Quality Checklist: Topic Registry (Mock, In-Memory)

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-06-11
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

> Note: per the project's house style (001–003, 008), these are library/protocol specs; "user" = the node author / integrating developer, and trait/type names appear because they ARE the published domain language (the deliverable), not incidental implementation detail. Mechanism choices (state-shape of the effective-subscription intersection, container types) are deliberately left to `/speckit-plan` (FR-014 says so explicitly).

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Notes

- **Parallels feature 008** (subscription list): same node-owned-reader + push-watch + `Control`-trait-split idiom, mirrored for topics. The two registries share **no trait** (distinct on-chain artifacts per `docs/node-lifecycle/README.md`).
- **Grounded in the formal spec** (`formal_spec/topic_registry/`, READ-ONLY): `Topic.publishers` empty ⇒ open topic; governance fields (owners/admins/replication/retention/alive/epoch) are out of node scope and deferred to feature 012 (FR-017).
- **Two node-integration points** extend the existing accept path (`handle_signed_message`): topic-validity (effective subscriptions = subscription-list ∩ registered topics, FR-014) and authorized-publisher enforcement (FR-015), the latter ordered before signature verification.
- **Scope decision resolved** (`/speckit-clarify` 2026-06-11): publisher-authorization *enforcement* (US3/FR-015) is **in scope — enforce by dropping** unauthorized-publisher messages. See the spec's Clarifications log. Cross-stream warmth (a "registries warm" signal before a node accepts traffic) was reviewed and left at the default — **no** signal; nodes converge from the streams (consistent with 008's deferral of a snapshot-complete marker) and tests poll to steady state.
- **Structural decision pending**: an ADR is required at `/speckit-plan` (FR / Assumptions) — interface split, global vs node-keyed watch, effective-subscription model, accept-path ordering.
- **Coordination**: `Node::new` gains a third generic registry parameter (`T: TopicRegistry`) — a signature change touching `main.rs` + all `tests/` call sites (atomic, like 008's). N-003 in `IMPLEMENTATION_NOTES.md` must be updated to record the publisher-authorization slice closed here vs what remains (FR-018).
- Items marked incomplete would require spec updates before `/speckit-clarify` or `/speckit-plan`. None are incomplete.

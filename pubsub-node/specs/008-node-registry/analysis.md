# Cross-Artifact Analysis: Subscription Registry (008)

**Input**: spec.md, plan.md, tasks.md, contracts/subscription-registry.md, data-model.md, research.md, ADR 0013, ADR 0014, constitution v1.1.0.

Records `/speckit-analyze` findings and their resolutions per the constitution's analysis-ledger rule (Development Workflow). Pre-implementation pass — no code exists yet, so this is cross-artifact consistency checking (the whole job at this stage). A second pass MUST run after implementation to verify artifact claims against the code (contracts §E).

## Session 2026-06-10 (pre-implementation)

### Findings

| ID | Category | Severity | Location(s) | Summary | Status / Resolution |
|----|----------|----------|-------------|---------|---------------------|
| F3 | Constitution VI / inconsistency | MEDIUM | spec FR-001/005/006; data-model; contract; ADR 0014 §1 | The node-facing `SubscriptionRegistry` trait carried write methods (`set_interest`/`unregister`) that the node never calls and the 012 chain reader cannot implement (chain writes are transactions) — unjustified surface on the domain interface. | **RESOLVED (applied this session).** Split into read-only `SubscriptionRegistry` (`watch_members`, `entry`, node-facing, 012-implementable) + `SubscriptionRegistryControl: SubscriptionRegistry` (`set_topics`, `unregister`, operator/test). `Node` holds `Arc<dyn SubscriptionRegistry>` — read-only at the type level. Propagated to spec/plan/data-model/contract/ADR 0014/tasks. |
| F1 | Coverage traceability | MEDIUM | spec US2 AS1–AS6 ↔ tasks T005/T007 | US2's acceptance scenarios assert on *emitted* events (observed by a watcher), but the US2 task (T005) verifies state via `entry()` read-back; the event-emission assertions land in US1's T007. | **ACKNOWLEDGED.** Intentional split (makes US2 independently testable via `entry()`). T007 already lists the `Joined`/`TopicsChanged`/`Left`/no-op emission semantics, so US2's acceptance criteria are covered — across T005 (state) + T007 (emission). No change required. |
| T2 | Constitution III / coordination | MEDIUM | tasks T017; ADR 0014 §2 | T017 edits feature 004's accepted ADR 0011 (illustrative comment) + the CLAUDE.md block to land the `Event::MembershipUpdate` seam rename — a cross-feature touch. | **ACCEPTED (gated).** ADRs are code-side/editable (not a Principle-V protocol-spec edit); T017 and the stop-the-line rule require the 004 author's sign-off before landing. |
| Term1 | Inconsistency (terminology) | LOW | (was) `set_interest`, "topic-interest set", "interest" prose | User preference: "topics" over "interest". | **RESOLVED (applied this session).** `set_interest` → `set_topics`; "topic-interest set"/"interest set" → "topic set"; "interest-scoped" → "topic-scoped"; etc. Residual "interest" is limited to the verbatim `Input`, the ADR 0013 filename, and natural "interested in" prose. |
| C1 | Inconsistency (terminology) | LOW | spec.md §Input | The verbatim `Input` block uses pre-rename names (`Registry`, `RegistryEvent`, `subscribe`, …). | **ACKNOWLEDGED.** By convention — `Input` is historical provenance; the Clarifications log supersedes it. No change. |
| G1 | Coverage gap (by design) | LOW | spec FR-019, FR-020 | Negative/non-goal requirements have no tasks. | **ACKNOWLEDGED.** Correct — negative requirements need no build task; marked N/A in coverage. |
| U1 | Underspecification | LOW | spec FR-003; tasks T004 | FR-003 (on-chain decode types stay module-internal) has no concrete test — nothing to decode in the mock. | **ACKNOWLEDGED.** Boundary established by module structure (T004) + the §E lib-surface check (T018); revisit at 012. |
| T1 | Task granularity | LOW | tasks T011 | T011 spans `node.rs` + `config.rs` + `error.rs` + `main.rs` + all `tests/` callers in one task. | **ACCEPTED.** The signature change must land atomically with all callers to keep the checkpoint green (green-checkpoints rule). Description enumerates the touch set. |

### Coverage Summary

All buildable requirements map to ≥1 task (see the per-requirement table in `plan.md`/this report's prior run). FR-019/FR-020 are non-goals (N/A). 100% coverage of buildable FR/SC (28/28).

### Constitution Alignment

No violations. **I** traced to spec/ADRs/protocol docs. **II** TDD ordering present per story (tests fail first); registry interaction treated as critical. **III** ADR 0013 + 0014 authored; F3's split folded into ADR 0014; T2 coordination gated. **IV** the `joining.md` config-vs-chain ambiguity surfaced (ADR 0013 + PR #52), not silently resolved. **V** no task edits `docs/`/`formal_spec/` (the joining.md fix is the separate human PR #52). Engineering standards (logs-not-tested, parse-at-edge, forward-compat-for-named-consumers, no new deps, reproducible) all reflected.

### Metrics

- Total requirements: 21 FR + 9 SC (FR-019/FR-020 non-goals)
- Total tasks: 18
- Coverage: 100% of buildable requirements
- Ambiguity / duplication: 0 / 0
- Critical issues: 0 (HIGH: 0)

### Next Actions

No CRITICAL/HIGH findings — **cleared for `/speckit-implement`** (TDD order, checkpoint commits). The MEDIUM items are resolved (F3, Term1) or gated/acknowledged (F1, T2). The post-implementation analyze pass MUST verify contracts §E against the code.

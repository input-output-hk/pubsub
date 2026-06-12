# Cross-Artifact Analysis: Topic Registry (013)

**Input**: spec.md, plan.md, tasks.md, research.md, data-model.md, contracts/topic-registry.md, quickstart.md, ADR 0016, constitution v1.2.0, `formal_spec/topic_registry/`.

Records `/speckit-analyze` findings and their resolutions per the constitution's analysis-ledger rule (Development Workflow). Pre-implementation pass — no 013 code exists yet, so this is cross-artifact consistency checking + formal-model fidelity. A post-implementation pass MUST run after `/speckit-implement` to verify artifact claims against the code (contracts §E).

## Session 2026-06-11 (pre-implementation — `/speckit-analyze`)

### Findings

| ID | Category | Severity | Location(s) | Summary | Status / Resolution |
|----|----------|----------|-------------|---------|---------------------|
| F1 | Inconsistency / underspecification | MEDIUM | spec FR-004; data-model `from_file`; tasks T007 | FR-004 mandated **both** "strict unknown-field rejection" **and** "governance fields … MUST be ignored"; with `deny_unknown_fields` those conflict, and data-model waffled ("the impl chooses"). | **RESOLVED (applied) — by simplification.** Per the maintainer's call (2026-06-11), the mock topic-registry file carries **only** `id` + optional `publishers` (the facts the node consumes); governance fields are **not** part of the mock format (012's on-chain domain). Strict `deny_unknown_fields` applies uniformly — any field outside `id`/`publishers` is a load error, no accepted-but-ignored fields. The "ignore governance" clause is dropped, not reconciled. Propagated to spec FR-004, data-model, research D10, tasks T007, ADR 0016 §1. |
| F2 | Ambiguity (observability timing) | LOW | spec FR-014; data-model accept-path | "recorded with an operator log line" could read as fold-time vs drop-site. | **ACKNOWLEDGED.** Realized at the **message-drop site** (`topic_not_registered` cause), per the data-model accept-path table; logs are operator UX, not test-anchored (constitution), so the exact moment is non-contractual. No further change. |
| F3 | Coverage gap | LOW | spec FR-007; tasks T006 | T006's enumerated cases don't explicitly include the gap-free/duplicate-free **burst↔live atomicity** assertion (008's analogous T007 did). | **ACKNOWLEDGED (recommendation).** T006 SHOULD add a case asserting a watch opened concurrently with a write observes the write exactly once (no gap/dup at the boundary). Recorded for the implementer; not a blocker. |
| F4 | Coverage gap | LOW | spec SC-002; tasks T006/T014 | SC-002's *multi-watcher* exactly-once fan-out is only implicitly covered (T014's multi-node share one registry). | **ACKNOWLEDGED.** Covered by T014 (multiple nodes = multiple watchers); a dedicated 2-watcher case in T006 is optional. |
| F5 | Inconsistency (naming) | LOW | quickstart §2 vs data-model/contract | Quickstart used `effective_subscriptions_sorted()`; the getter is `effective_subscriptions()`. | **ACKNOWLEDGED.** Test-only sort convenience; harmless. Rename at implementation if it drifts. |

### Coverage Summary

All buildable FR/SC map to ≥1 task. FR-017 is a negative (non-goal); FR-018 is partial-negative + T018. 100% coverage of buildable FR/SC.

### Constitution Alignment

No violations (evaluated against v1.1.0 at analyze time; re-evaluated against v1.2.0 below). **I** all FRs trace to spec/ADR 0016/formal-spec/008-precedent. **II** TDD ordering present per story (T006<T007, T008<T009, T012<T013); integration tests after wiring follow the 008 pattern; logs never asserted. **III** ADR 0016 authored at plan time. **IV** the formal model's governance/`alive` deferral (FR-017) and the identity-unification (N-009) are surfaced, not silently resolved. **V** no `docs/`/`formal_spec/` edits.

### Metrics

- Total requirements: 29 (19 FR + 10 SC); FR-017 negative, FR-018 partial-negative
- Total tasks: 19
- Coverage: 100% of buildable FR/SC
- Ambiguity / duplication: 2 / 0
- Critical issues: 0 (HIGH: 0)

### Next Actions

No CRITICAL/HIGH — cleared for `/speckit-implement` (TDD order, checkpoint commits). F1 resolved; F2–F5 acknowledged LOW. The post-implementation pass MUST verify contracts §E against the code.

## Session 2026-06-11 (post-rebase — constitution v1.2.0 alignment + design review)

A rebase onto `origin/main` brought (a) the constitution amendment **v1.2.0** (new Engineering Standard: *Declarative test construction*) + the merged `MembershipScript` builder, and (b) a maintainer design review of validation placement and formal-model fidelity. Both are reconciled into the artifacts.

| ID | Category | Severity | Summary | Resolution |
|----|----------|----------|---------|------------|
| V1 | Constitution alignment (v1.2.0) | — | The new *Declarative test construction* standard requires multi-step test state to be built via a test-only builder beside the type. 013's tests script registry-write sequences and mixed `TopicRegistryUpdate`+`MembershipUpdate` `apply` sequences. | **APPLIED.** plan.md cites v1.2.0 + the standard; tasks T005 adds `src/topic_registry/test_support.rs` (`TopicRegistryScript` + `TopicRegistryEvent` constructors, mirroring `MembershipScript`); the Tests header + T006/T008/T012 build scripts via the builders (reusing `MembershipScript` for the membership half). |
| V2 | Design (validation placement) | — | Should topic-validity be enforced in the node (fold + intersect) or by the subscription registry (sanitize before emit)? | **CONFIRMED: node.** Publisher authorization forces the node↔topic-registry stream regardless, so validity is a free intersection; sanitizing in the subscription registry couples two independent artifacts, breaks the 012 swap, and bypasses the event-queue fold. Recorded in research D11, ADR 0016 §4, spec Clarifications + plan note 9. No design change (confirms FR-011..016). |
| V3 | Formal-model fidelity + identity | — | Define the registry entry against the Quint `Topic` record; clarify the publisher/node-id identity relationship. | **APPLIED.** Node-facing projection stays **publishers-only** (owners/admins/R/T/`alive` deferred to 012 — no node consumer; mock writes permissionless); publishers keyed by `PublicKey` ≡ subscription-list node id at 011, recorded as **N-009**; numeric `TopicID`+`name` collapse to string `TopicId`. Recorded in research D8/D12, data-model "Formal-model grounding", spec Clarifications/Assumptions, ADR 0016 §1, plan note 10, IMPLEMENTATION_NOTES N-009. N-003 updated (013 closes its publisher-authorization item 1). |

### Constitution re-evaluation (v1.2.0)

Re-checked against the added standard: **Declarative test construction** — satisfied by the `test_support.rs` builder plan (V1). All other principles/standards unchanged from the v1.1.0 evaluation above. No violations.

### Next Actions

Cleared for `/speckit-implement`. The post-implementation analyze pass MUST verify contracts §E against the code (incl. the new `test_support.rs` builder and the publishers-only projection).

## Session 2026-06-12 (analyze pass 2 — convergence + LOW remediation)

A second `/speckit-analyze` pass after the design-review + v1.2.0 + F1 edits, to catch cascade drift. `contracts/topic-registry.md` (untouched in the prior session) verified consistent with the updated spec/data-model. One new trivial drift (G1) surfaced; G1 + the carried LOWs F3/F5 were then remediated.

| ID | Category | Severity | Summary | Resolution |
|----|----------|----------|---------|------------|
| G1 | Inconsistency (cascade drift) | LOW | plan.md's Scale/Scope + tasks T005 reference `src/topic_registry/test_support.rs`, but the plan's source-code **tree diagram** omitted it. | **RESOLVED (applied).** Added the `test_support.rs` line to the plan's source tree (after `in_memory.rs`). |
| F3 | Coverage gap (was carried) | LOW | FR-007's gap-free/duplicate-free burst↔live atomicity clause wasn't explicitly enumerated in T006. | **RESOLVED (applied).** Added an **atomicity** case to T006: opening `watch()` then immediately writing yields the write exactly once (no gap/dup at the boundary). |
| F5 | Inconsistency (naming, was carried) | LOW | quickstart §2 called `effective_subscriptions_sorted()`; the defined getter is `effective_subscriptions()`. | **RESOLVED (applied).** Quickstart now uses `effective_subscriptions()` — sorting inline before the `assert_eq!` (the `.contains()` site just renamed). |

**Carried (still acknowledged, not changed)**: F2 (operator log emitted at the drop site — non-contractual, logs not test-anchored) and F4 (SC-002 multi-watcher fan-out covered by T014's multi-node test).

**Convergence**: pass 1 → 1 MEDIUM + 4 LOW; pass 2 → 0 MEDIUM + 1 new LOW (G1), then all actionable LOWs (G1/F3/F5) applied. 0 critical / 0 high throughout. The artifact set has converged — cleared for `/speckit-implement`.

## Session 2026-06-12 (post-implementation — verify artifacts against code)

The required post-implementation pass (constitution Development Workflow: "once the implementation exists, a consistency pass MUST verify artifact claims about the implementation against the implementation itself"). Implementation landed across 5 green checkpoint commits (US1 → US2 → US3 → US4 → polish; PR #55) plus a post-implementation naming refinement; this pass verifies the artifacts' §E claims against the code.

**Verify-against-code (contracts §E) — all confirmed:**

- `git diff main -- src/lib.rs` adds only `mod topic_registry;` + the six `pub use` items (registry, control, error, event, watch, in-memory impl). ✅
- `Node::new<N: Network, R: SubscriptionRegistry, T: TopicRegistry>(…, subscription_registry: Arc<R>, topic_registry: Arc<T>)`; `Node::effective_subscriptions` added; `subscriptions`/`candidates`/`peers` unchanged; no extra pub fns. ✅
- `Event::TopicRegistryUpdate`; `ConfigError::{DuplicateTopicEntry, InvalidPublisherKey}`; `PublicKey` gains only `Ord, PartialOrd`. ✅
- `handle_signed_message` order: subscribed → **registered** → **authorized** → verify (drop causes `topic_not_registered` / `publisher_not_authorized` precede `invalid_signature`). ✅
- `handle_topic_registry_update` private in `state.rs`; `registered_topics` `pub(crate)`; `InMemoryTopicRegistry` internals (`Inner`, `RawTopicList`, `RawTopic`, `decode_hex`, `fanout`) private; global `watch()` with no per-subscriber filter. ✅
- Doctest in `src/network.rs` updated for the 6-arg `Node::new` (doctests run under `cargo test`, not `--all-targets`). ✅

**Green**: `cargo fmt --check`, `clippy --all-targets --all-features -D warnings`, `build --all-targets --locked`, `test` (incl. doctests) — all pass; no new dependencies.

| ID | Category | Severity | Summary | Status |
|----|----------|----------|---------|--------|
| P1 | Inconsistency (stale comment, pre-existing) | LOW | `src/state.rs`'s `Effect` justification comment still reads "008's **RegistryUpdate** arm" — a 004-era label (008 renamed that seam to `MembershipUpdate`; 013 adds `TopicRegistryUpdate`). **Inherited from `main`, not introduced by 013.** | **Deferred** — out of 013 scope; flagged for a future 004/008 comment cleanup. |

**Post-implementation naming refinements** (applied, all green; recorded for the ledger):

- `Node::new`'s subscription-registry parameter renamed `registry` → `subscription_registry` (symmetry with `topic_registry`; the bare `registry` was ambiguous once two registries existed). Zero API impact (Rust has no named args). Private reader fn `registry_reader_loop` → `subscription_registry_reader_loop` to pair with `topic_registry_reader_loop`. (commit on PR #55)
- Reviewed and **kept**: `test_support.rs` (matches the constitution-cited `subscription_registry/test_support.rs` worked example) and `NodeState.registered_topics` (the qualifier disambiguates from the sibling `subscriptions` field and names registry provenance; the registry's own unqualified `topics` field is unambiguous in its context).

**Metrics**: 19 tasks complete (100%); 29 buildable FR/SC, 100% covered; 0 critical / 0 high; 1 LOW (P1, pre-existing/deferred).

### Next Actions

Implementation faithfully realises spec/plan/contracts/ADR 0016 — **PR #55 ready for review**. No 013 defects. P1 is a pre-existing 004-era comment, optionally cleaned up separately.

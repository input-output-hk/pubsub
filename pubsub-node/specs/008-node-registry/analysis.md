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

## Session 2026-06-10 (post-implementation)

The implementation surfaced one artifact-vs-code mismatch, reconciled in the docs:

| ID | Category | Severity | Summary | Resolution |
|----|----------|----------|---------|------------|
| F4 | Inconsistency (artifact vs code) | MEDIUM | Spec/contract/ADR 0014/data-model described the node holding the registry as `Arc<dyn SubscriptionRegistry>`. An `async fn` trait is not `dyn`-compatible, so the code consumes it **generically** — `Node::new<N: Network, R: SubscriptionRegistry>(…, Arc<R>)` — exactly as `Network` is consumed (`Arc<N>`, ADR 0007). | **RESOLVED (applied).** Spec FR-001, contract §A/§B, ADR 0014 §4, and data-model updated to the generic `Arc<R>` shape with the `async`-trait/`dyn`-incompatibility rationale. (Residual descriptive `Arc<dyn …>` prose in plan.md/research.md is non-normative.) |

Contracts §E verification against code (post-implementation): `git diff main -- src/lib.rs` shows only the new `mod subscription_registry;` + the registry `pub use` block; `set_topics`/`unregister` live on `SubscriptionRegistryControl` (node holds only the read trait); `handle_membership_update` is private in `state.rs`; the candidate set is `pub(crate)` on `NodeState` (exposed via `Node::candidates`), distinct from the config `peers` field. All green: `cargo fmt && build && clippy --all-targets -D warnings && test`.

## Session 2026-06-10 (post-implementation, unified-watch refinement)

A design refinement landed after the post-implementation pass above: the read
interface was collapsed to a single node-keyed `watch`, and the `entry`
point-read was removed. Recorded here so the ledger reflects the shipped design
(commit `1001d63`). All artifacts (spec FR-001/007/008/014/015/016/018, plan,
research, data-model, contract, quickstart, tasks, checklist) and ADR 0014 were
reconciled in the same change; the suite is green.

| ID | Category | Severity | Summary | Resolution |
|----|----------|----------|---------|------------|
| F5 | Refinement (interface simplification) | — | The read trait carried two methods — `watch_members(topics)` (membership stream) + `entry(node)` (self point-read for the node's own topics). Two reads where one stream suffices: a node-keyed watch can replay the node's own entry as its head event, so the node derives **both** its own subscription set and its candidate sets from one stream and starts empty. | **APPLIED.** `SubscriptionRegistry` is now a single `fn watch(&self, node: PeerId) -> impl Future<…> + Send` (RPITIT + `Send`; the node-owned reader awaits it in a spawned task). The cold-start burst replays the watcher's **own** entry first (`Joined { node, own_topics }` → subscriptions) then the scoped members of those topics (→ candidates), then live deltas. `handle_membership_update` branches on `node == self_id` (own → `subscriptions`, others → `candidates`), so self-exclusion is a property of the fold. (Supersedes F3's two-method read split — the read trait is now `watch`-only; `SubscriptionRegistryControl` is unchanged.) |
| F6 | Refinement (type removal) | — | `entry`/`SubscriptionEntry` had no consumer once the node derives its topics from the watch head (010 reads via `watch`; 012 is the impl) — unjustified surface under the forward-compatible-interfaces standard (no ROADMAP consumer). | **APPLIED.** `entry` and the `SubscriptionEntry` struct removed entirely (not demoted); `lib.rs` re-exports six items, no `SubscriptionEntry`. The US2 registry tests now assert through the `watch` stream (head `Joined` carries the node's own id + topics; the empty-vs-unregister distinction is observed via a watcher's delta stream). A materialized-entry read can return at 012 if a consumer needs one. |
| F7 | Behavior change (FR-018 relaxed) | — | With topics derived from the stream rather than a startup `entry(self_id)` lookup, the original "fail fast on absent entry" clarification no longer fits: there is no startup point-read to fail on. | **APPLIED.** `Node::new` seeds `NodeState` with an empty subscription set and spawns the reader (`watch(self_id)`); subscriptions + candidates converge as the burst drains. Construction no longer fails fast — a node with no entry stays at empty derived state ("registered but not yet present / initializing"); the reader logs at `error` if the watch cannot open. `NodeError` gains **no** variant. The superseded clarification is marked in spec.md's Clarifications log. |

Contracts §E re-verification against code (this refinement): `SubscriptionRegistry` exposes only `watch` (RPITIT + `Send`); `entry`/`SubscriptionEntry` absent from `lib.rs` and the trait; `Node::new` performs no startup point-read and returns no registration-not-found error (`error.rs` unchanged); `tests/common` awaits subscription convergence before send-then-observe. All green: `cargo fmt --check && build --all-targets && clippy --all-targets -D warnings && test`.

## Session 2026-06-11 (post-implementation, remove node-local subscription mutators)

A review question — "do we still need `Node::subscribe`/`unsubscribe` now that the write domain is operator-driven?" — surfaced a live inconsistency, resolved by [ADR 0015](../../docs/decisions/0015-node-has-no-local-subscription-mutators.md).

| ID | Category | Severity | Summary | Resolution |
|----|----------|----------|---------|------------|
| F8 | Inconsistency (artifact/code vs source-of-truth) | MEDIUM | `NodeState.subscriptions` (the message accept-filter) had **two writers**: the registry fold (authoritative, from the `watch` stream) and the node-local `subscribe`/`unsubscribe` mutators (002/ADR 0008, retained sync by ADR 0012). A caller could subscribe the node to a topic it is not registered for — the same accountability hole ADR 0013 closed for config — and a later self-event would clobber it (split-brain accept-filter). | **APPLIED (ADR 0015).** Removed `Node::subscribe`/`unsubscribe` + `SubscribeOutcome`/`UnsubscribeOutcome` + the `NodeState` mutators; `subscriptions` is now written only by `handle_membership_update`. Read-only `subscriptions()` getter retained. `tests/topic_runtime.rs` rewritten: mutator scenarios replaced by one registry-driven *narrowing* test (`set_topics` → `watch` → fold), initial-filter + decoupled-emission retained. Superseded notes added to ADR 0008 (runtime surface) and ADR 0012 §2; ADR 0014 §4 + alternatives, plan §8, contract §B, data-model, spec Assumptions reconciled. Runtime *narrowing* works via the registry today; *expansion*/re-scoping deferred to 012 (watch scoped at `watch(self_id)` time). |

Code verification: `grep -n "fn subscribe\|fn unsubscribe\|SubscribeOutcome\|UnsubscribeOutcome" src/` returns nothing; the sole writer of `NodeState.subscriptions` is `handle_membership_update`. All green: `cargo fmt --check && build --all-targets && clippy --all-targets -D warnings && test`.

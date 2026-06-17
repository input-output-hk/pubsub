# Cross-Artifact Analysis: 014-registry-consistency

Ledger of `/speckit-analyze` findings and their resolutions (Constitution §Development Workflow — analysis ledger; findings are not closed by commit messages, only by an entry here).

## Session 1 — 2026-06-16 (post-`/speckit-tasks`, pre-implementation)

Cross-artifact consistency pass over `spec.md` (FR-001..015, SC-001..010, US1–US2, Clarifications 2026-06-15 ×5), `plan.md`, `tasks.md` (T001–T013), against the constitution. Coverage was 100% (every FR/SC mapped to ≥1 task); 6 findings, all remediated in-session (read-only analysis produced the report; the edits were applied immediately after with the maintainer's "address all" instruction).

### Findings

| ID | Category | Severity | Location | Summary | Resolution |
|----|----------|----------|----------|---------|------------|
| C1 | Constitution / task ordering | **CRITICAL** | tasks.md T004/T005/T006 | Green-checkpoint break: strict drop (T004, then checkpoint commit #1) was activated *before* the readiness gate (T005) and the 013-test rework (T006). Activating strict drop breaks pre-existing 013 tests (the `state.rs` US2 subscribe-before-register pure-core test **and** the multi-node integration test, which becomes racy without the readiness gate). The T004/T005 commits would have run `cargo test` non-green, conflicting with the green-checkpoint MUST. | **Resolved.** Re-sequenced so T004 (fold) + T005 (readiness gate) + T006 (013-test rework + cold-start integration) are **one logical green increment** committed only at T006 (now checkpoint commit #1, the US1 MVP). T004/T005 carry explicit "no commit; working tree intentionally red until T006" notes. Dependency graph + Implementation Strategy updated; US2's checkpoint renumbered #3→#2. |
| F1 | Inconsistency / wrong file path | **HIGH** | plan.md §Source tree + §Summary + §Scale/Scope; tasks.md T008 | The receive-path authorization check lives in `src/state.rs::handle_signed_message`, not `src/message.rs`; `message.rs` is unchanged. Plan listed `message.rs` as CHANGED and T008 said "In src/message.rs / handle_signed_message" — would send the implementer to the wrong file. | **Resolved.** Plan Source tree moves the `handle_signed_message` check line under `state.rs` and lists `message.rs` under UNCHANGED (with a note that it carries no receive-path logic); plan §Summary/§Scale-Scope corrected; T008 corrected to `src/state.rs handle_signed_message`. |
| L1 | Duplication | LOW | spec FR-003 / FR-003a / FR-003b | FR-003b (drop-logging) read as a third drop rule, restating logging implied by FR-003/003a. | **Resolved.** FR-003b reworded as the explicit **shared operator-visibility clause** for FR-003 and FR-003a ("not a third drop rule"). |
| L2 | Terminology drift | LOW | spec FR-004, FR-012; tasks T006/T009 | "SC-004" overloaded — *this* spec's SC-004 (accept/drop-matrix preservation) vs **013 SC-004** (the removed subscribe-before-register dynamic). | **Resolved.** All references to the removed dynamic prefixed "**013 SC-004**" (spec FR-004 now contrasts the two explicitly); 014's own SC-004 left unprefixed. |
| L3 | TDD ordering | LOW | tasks T008 → T009 | The explicit receive-path no-regression matrix (T009) trails the refactor (T008). | **Resolved (clarified).** T008 now states the pre-existing 013 US3 matrix tests are the **continuous regression guard** through the refactor; T009 is the explicit pin making that guard explicit. No reordering needed (the guard is continuous). |
| L4 | Organization | LOW | spec FR-008 | FR-008 bundles a US1 clause (defensive fold) and a US2 clause (carry `TopicEntry`) under the "declarative topic entry" heading. | **Resolved.** FR-008 gains a delivery note: the defensive fold lands in US1, the `TopicEntry` carry in US2; both behaviour-equivalent to a bare set + defensive checks. |

### Coverage summary

100% — all 15 FRs and 10 SCs map to ≥1 task (full table in the analyze report). No unmapped tasks (T001 setup; T002 foundational; T010–T013 polish/cross-cutting are standard non-story tasks).

### Constitution alignment

One conflict (C1, green-checkpoint MUST) — **resolved** by re-sequencing. All other principles satisfied: TDD ordering (T003→T004, T007→T008; II), ADR 0020 authored at plan time (III), the cross-registry ordering ambiguity 013 flagged (N-015/S7) surfaced and resolved not silently (IV), no read-only-spec edits (V), logs-not-a-test-surface honored, declarative test construction reused, and the readiness gate is **event-driven (`SnapshotComplete`), not timing-based** — an improvement over 013's "relies on timing" reproducibility note (Engineering Standard: reproducible tests).

### Metrics

- Total requirements (buildable): 25 (15 FR + 10 SC)
- Total tasks: 13
- Coverage: 100%
- Ambiguity: 0 | Duplication: 1 (L1, resolved) | Critical: 1 (C1, resolved)
- Post-remediation open findings: **0**

### Disposition

**GO for `/speckit-implement`.** All findings remediated in spec/plan/tasks this session; no open items. The single behavioural consequence to keep visible during implementation is C1's: the US1 invariant is **one** green commit (T004+T005+T006), not three — the working tree is intentionally red between T004 and T006.

## Session 2 — 2026-06-17 (post-implementation, verify-against-code)

Post-implementation pass (constitution: verify artifact claims against the implementation), after 014 was rebased onto merged 004-connections and the code landed green (`fmt`, `clippy -D warnings`, 16 test binaries, doctests). Coverage 100% (16 FR + 12 SC, all code-verified). The verify-against-code sweep confirmed every normative claim (atomic 5-structure cascade, dial trigger, `SnapshotComplete` on both registries, oneshot readiness gate, `TopicEntry` projection + `pub(crate)`, timer fully removed from `src/`, direct `subscriptions_snapshot`, no log-content assertions). Three **doc-vs-as-built drift** findings — all remediated in this session.

### Findings

| ID | Category | Severity | Location | Summary | Resolution |
|----|----------|----------|----------|---------|------------|
| D1 | Inconsistency / verify-against-code | MEDIUM | data-model §6, contracts §E, quickstart §3, ADR 0020 §5 | Described the readiness gate as "`Node::new` drains the topic watch to `SnapshotComplete`" (the originally-planned shape), but the as-built mechanism is a **non-blocking** `Node::new` + an **in-node oneshot** between the topic and membership reader producers. | **Resolved.** data-model §6 rewritten to the oneshot (non-blocking, one-shot cold-start await, fail-safe); contracts §E and quickstart §3 corrected; ADR 0020 §5 gains an as-built pointer to the amendment. |
| D2 | Inconsistency | MEDIUM | spec Edge Cases | The "Connection-state cascade (forward, out of scope here)" bullet contradicted FR-002/FR-010 (now in scope + implemented). | **Resolved.** Rewritten to state the cascade clears `upstream`/`downstream` (in scope post-rebase). |
| D3 | Terminology / provenance | LOW | spec references block | The 004-connections reference still called it "open, not on `main`". | **Resolved.** Marked superseded, pointing to Clarifications 2026-06-17. |

### Disposition

**GO.** Implementation matches the (now-corrected) artifacts; 0 open findings. No blocking action was introduced: `Node::new` is non-blocking, and the only wait is a one-shot cold-start await inside the membership reader (fail-safe; steady state has no gating or timer).

## Session 3 — 2026-06-17 (b) (single-indexer readiness collapse)

Post-review design change (maintainer review): the two `SnapshotComplete` events — one a fold no-op (topic), one the dial trigger (membership) — modelled two independent chain read-positions, which the realistic single-chain-indexer model contradicts. **Decision: collapse to a single registry indexer reader.** The two reader producers + the in-node oneshot are replaced by one `registry_indexer_loop` that drains the topic burst before the membership burst (cold-start ordering now intrinsic to the reader's sequence — no oneshot) and then pushes the existing `Event::ConnectionSetup` (the single dial trigger; no new node-`Event` variant). The per-stream `SnapshotComplete` markers become reader-consumed stream-replay delimiters; both fold arms are now symmetric no-ops. Registries stay separate data artifacts (readiness signal only collapses). Code + artifacts updated; full gate green (`fmt`, `clippy -D warnings`, all test binaries + doctests). Recorded in ADR 0020 (Amendment 2026-06-17 (b)) and spec Clarifications Session 2026-06-17 (b). This supersedes Session 2's D1 (the oneshot is removed) and its "only wait is a one-shot await" disposition note — there is now no cold-start await at all. **GO**, 0 open findings.

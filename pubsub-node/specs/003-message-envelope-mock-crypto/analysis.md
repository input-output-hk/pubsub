# `/speckit-analyze` Findings — 003 Message Envelope + Mock Crypto

**Purpose**: Cross-artifact consistency analysis of `spec.md`, `plan.md`, and `tasks.md` (with corroborating reads of `research.md`, `data-model.md`, `contracts/library-api.md`, `quickstart.md`, ADR 0009, ADR 0010, IMPLEMENTATION_NOTES.md, and the requirements checklist). Mirrors 002's per-pass-findings ledger (`specs/002-topic-subscription-filtering/analysis.md`) — pass-numbered, each finding either RESOLVED (fix applied in-pass) or NO-OP (acknowledged design choice, no edit required).

**Convergence rule**: per ROADMAP §4, iterate passes until a pass closes with zero substantive findings; that pass is the closure record.

**Entry state**: 003's design phase landed after four `/speckit-checklist` passes that converged to zero findings on the requirements checklist (`checklists/requirements.md`). `/speckit-analyze` is the cross-artifact equivalent — checklist validates the spec; analyze validates spec ↔ plan ↔ tasks ↔ supporting docs.

---

## Pass-1 Findings (2026-06-04)

Pass-1 walk loaded the seven core artifacts plus the two ADRs and IMPLEMENTATION_NOTES.md, built the FR / SC / US semantic inventory, and ran the six detection passes (Duplication / Ambiguity / Underspecification / Constitution Alignment / Coverage Gaps / Inconsistency).

### Coverage Summary

| Dimension | Total | Covered | Coverage % |
|-----------|-------|---------|------------|
| Functional Requirements (FR-001 … FR-020) | 20 | 20 | 100% |
| Success Criteria (SC-001 … SC-008) | 8 | 7 buildable + 1 operator-UX | 100% (within scope) |
| User Stories (US1 … US4) | 4 | 4 | 100% |
| Tasks (T001 … T028) | 28 | 28 (all map to FR / SC / US / quality-gate) | 100% |

**Requirement → task mapping** (every FR has at least one task; matrix mirrors `data-model.md §19`):

| FR | Tasks | Notes |
|----|-------|-------|
| FR-001 (`Message` enum + `SignedMessage` + `PlainMessage` + `RoutingFrame` rename) | T012 (rename), T013 (reshape), T014 (re-exports) | Largest single-commit migration in 003 |
| FR-002 (`PublisherId` newtype) | T011 | `[P]` parallelizable with crypto-mock work |
| FR-003 (concrete byte-newtype types + `Display`) | T004 | Includes redacting `Debug` on `PrivateKey` |
| FR-004 (`Signer` trait) | T006 | No type params, no associated types |
| FR-005 (`Verifier` trait + `VerifyError` non_exhaustive) | T005, T006 | `#[non_exhaustive]` enum |
| FR-006 (`crypto::mock` module + "MOCK — not unforgeable" warning) | T007, T009, T010 | Module-level + per-struct rustdoc |
| FR-007 (`with_seed` / `from_entropy`) | T002 (deps), T009 | `rand` + `rand_chacha` deps |
| FR-008 (`generate_keypair`) | T009 | Draws from internal RNG |
| FR-009 (mock algorithm + `PUBLIC_SUFFIX`) | T008, T010 | Byte-symmetric construction |
| FR-010 (`signed_bytes` seam on `PlainMessage`) | T013 | No version_tag byte (Q1) |
| FR-011 (`MessageHash::of` on `PlainMessage`) | T002 (sha2 dep), T013 | Content-anchored (N-005) |
| FR-012 (`Node` verifier field) | T017 | `Arc<dyn Verifier>` constructor parameter |
| FR-013 (receive-task ordering — topic filter first) | T018 | Per Q6 |
| FR-014 (`invalid_signature` event) | T018 | Tests-don't-check-logs convention |
| FR-015 (002 `topic_drop` rename) | T018 (atomic with FR-014), T020 (002 test migration) | Same-commit rule |
| FR-016 (signature-only validation) | Negative coverage: T018 implements *only* the signature check; chain-integrity tests *deliberately absent* (deferred per N-003) | Constitution Principle II TDD trigger scope-limited to signature authenticity |
| FR-017 (TOML unchanged) | T019 (main.rs wires verifier programmatically) | No new TOML field; 002's `node-config.toml.md` contract inherited |
| FR-018 (011 swap-readiness) | Architectural invariant — pinned by trait shapes in T004 / T006 / T010; no 003-era test anchor (correctly — there is no 011 verifier yet to swap against) | Forward-looking; verified at the spec / ADR lock-in level |
| FR-019 (linearizability across verification step) | T018 | Verifier is stateless; mutex-protected snapshot path |
| FR-020 (receive-task confinement + pattern-match) | T018 | No new async task; synchronous `verify` call |

**SC → task mapping**:

| SC | Tasks / Verification | Notes |
|----|----------------------|-------|
| SC-001 (US1 demo under 30s) | T016 (US1 tests realize the demonstration) | Trivially satisfied for SHA-256 |
| SC-002 (binary contrast under 30s) | T016 (AS-2 + AS-3) | Test-anchored on `received_messages()` only |
| SC-003 (filter composition) | T022 (US3 tests) | Zero false-pos / false-neg in snapshot |
| SC-004 (spec-quickstart cohesion ≤ 1 hour) | T026 (quickstart walkthrough) | Same as 002 SC-004 |
| SC-005 (MockCryptoScheme reproducibility) | T023 (US4 AS-1 + AS-2) | At least 10 successive `generate_keypair` calls |
| SC-006 (rustdoc MOCK warning at 4 sites) | T007, T009, T010, T025 (audit) | Module + struct levels |
| SC-007 (`topic_drop` rename atomic) | T018 (atomic same-commit), T020 (002 test migration), T022 (Rust-level grep test), T028 (polish-phase grep) | Belt-and-braces verification |
| SC-008 (operator-visible at default log level) | Operator-UX criterion — intentionally NOT test-anchored per FR-014's tests-don't-check-logs convention; verified manually during the SC-001 / SC-002 demonstration | Symmetric to 002 SC-006 |

### Pass-1 Findings Table

Pass-1 located four LOW-severity acknowledgments and zero substantive (CRITICAL / HIGH / MEDIUM) findings. The artifact set converged through four prior checklist passes that swept the spec-internal consistency surface; pass-1 confirms that the cross-artifact surface (spec ↔ plan ↔ tasks ↔ supporting docs) is also converged.

| ID | Category | Severity | Location(s) | Summary | Resolution |
|----|----------|----------|-------------|---------|-----------|
| A-L1 | Inconsistency (cosmetic) | LOW | `plan.md` line 122 (Source Code §6 inline comment) | Inline `Cargo.toml` change note lists only the three runtime deps (`rand`, `rand_chacha`, `sha2`); `proptest` (test-only) is acknowledged in the canonical Technical Context (`plan.md` lines 29–34) and in plan.md line 22 (Summary) but not echoed in this brief inline source-tree comment. | NO-OP — the source-tree comment is deliberately a one-liner pointer to the canonical Technical Context block; duplicating the full dep list inline would invite drift. The canonical surface (Technical Context + Constitution Check §"Justified dependencies") is correct. |
| A-L2 | Style | LOW | `tasks.md` T013 body | T013's task body embeds ~30 lines of inline byte-layout pseudocode (the FR-010 encoding sketch). Dense but informational. | NO-OP — `/speckit-tasks`-style tasks are designed to be self-contained for `/speckit-implement`. The inline pseudocode duplicates `data-model.md §16b` + research.md but loses no information. Trimming would force `/speckit-implement` to context-switch mid-task. |
| A-L3 | Coverage Note | LOW | `tasks.md` (no FR-018-specific task) | FR-018 (011 swap-readiness) is verified architecturally — by the trait-surface choices encoded in T004 (concrete byte-newtype types) + T006 (no-associated-types trait pair) + T010 (mock impl preserves the trait surfaces) — but has no test anchor in 003. `data-model.md §19` matrix correctly cites §§1, 2, 3, 8, 9, 16 (entity definitions, not tests). | NO-OP — there is no 011 `Ed25519Verifier` to swap against in 003. The invariant becomes test-anchored when feature 011 lands and its swap pass verifies the trait surfaces did not budge. FR-018 is a forward-looking architectural invariant correctly framed as such in spec text. |
| A-L4 | Coverage Note | LOW | `spec.md` SC-008 | SC-008 (operator-visible at default log level) is intentionally not test-anchored per FR-014's tests-don't-check-logs convention. Listed as "operator UX" in spec; pass-1 confirms no task asserts log content. | NO-OP — convention is explicit; spec frames SC-008 as operator UX symmetric to 002 SC-006. |

### Detection-Pass Sweep — Detailed

**A. Duplication**: zero substantive duplicates across FRs or SCs. Each FR-001 … FR-020 covers a distinct concern (envelope shape, identity type, byte newtypes, traits, mock, encoding seam, hash, Node field, ordering, log event, rename, validation scope, config, swap-readiness, linearizability, task confinement). FR-014 and FR-015 share the `message_dropped` event marker, but with distinct `cause` values and distinct emission sites — the "shared event marker, distinct cause" pattern is the deliberate project-wide drop-event convention (saved memory `feedback_message_dropped_event_convention`), not redundancy.

**B. Ambiguity**: zero vague adjectives in normative FR clauses. The spec uses "natural single-process implementation" (FR-019 non-normative parenthetical) and "trivially satisfied" (plan.md Performance Goals) — both in explanatory, non-normative text. No TODO / TKTK / ??? / placeholder markers. No unresolved `[NEEDS CLARIFICATION]` markers.

**C. Underspecification**: zero. Every FR carries a verb + an object + (where applicable) a measurable outcome. The two negative requirements (FR-016 — what the receive path MUST NOT inspect; FR-017 — what MUST NOT change in TOML) explicitly enumerate the deferred items and trace each to N-003 / inherited 002 contracts. FR-018's forward-looking architectural invariant lists the specific surfaces that MUST remain unchanged across the 011 swap.

**D. Constitution Alignment**: all five principles ✅ pass per `plan.md` Constitution-Check block (both initial gate and post-Phase-1 gate). Cross-checked against pass-1 reading of `.specify/memory/constitution.md`:
  - Principle I (Correctness Over Optimization): no optimization-led decisions; receive-task ordering choice (Q6) justified architecturally, not by a performance target.
  - Principle II (Test-Driven for Correctness Claims): T016 explicitly TDD-first (fails initial, then T017–T020 turn green); chain-integrity TDD correctly scoped out via FR-016 + N-003.
  - Principle III (Document Structural Decisions as ADRs): two ADRs cover 003 — ADR 0009 (crypto trait shape, pre-spec) + ADR 0010 (protocol-message type hierarchy, post-Phase-1 when the type-shape concern surfaced).
  - Principle IV (Specs as Ambiguity Detectors): six `/speckit-clarify` rounds + four `/speckit-checklist` passes; round-5 audit closed at zero spec-level findings.
  - Principle V (Specs Are Read-Only): no plan-level edits to `../formal_spec/`, `../docs/`, or `../docs/extensions/`.
  Engineering Standards (property-based testing, observable state transitions, justified dependencies, reproducible tests) each addressed in plan.md and realized in tasks (T023 proptest; FR-014 structured event; T002 + research §5 dep justifications; seeded PRNG + `Timestamp::from_millis` in tests).

**E. Coverage Gaps**: zero — every FR has at least one task; every user story has at least one test task; every buildable SC has at least one task; the one operator-UX SC (SC-008) is intentionally not test-anchored per FR-014's documented convention.

**F. Inconsistency / Terminology Drift**: zero substantive. Verified consistency across artifacts for:
  - **"Envelope" terminology**: prose use throughout spec / plan / data-model matches the synthesis §2.3 meaning (whole signed message); Rust type names are `Message` / `SignedMessage` / `PlainMessage` / `MessagePayload`; 001's routing wrapper renamed `RoutingFrame` (ADR 0010). The Assumptions section (spec line 201) explicitly pins this convention; no leftover use of "envelope" as a Rust identifier.
  - **`Message::Signed(SignedMessage { plain, signature })`**: shape consistent across spec FR-001, plan §Summary, data-model §16-§16a, contracts/library-api.md, tasks T013, ADR 0010.
  - **`MessageHash::of(&PlainMessage)`**: signature-malleability-immunity rationale present in FR-011, spec Clarifications hash-input bullet, plan.md Technical-approach bullet, data-model §4 + §16b, ADR 0010 Consequences, IMPLEMENTATION_NOTES.md N-005.
  - **Drop event shape**: `event = "message_dropped"` + `cause = "topic_not_subscribed"` / `cause = "invalid_signature"` consistent across FR-014, FR-015, US3 acceptance scenarios, plan.md, data-model §18 (Tracing event shape), tasks T018 / T020 / T022 / T028.
  - **`TestSigner` / `TestVerifier` / `MockCryptoScheme`**: naming consistent across all surfaces.
  - **Receive-task ordering** (topic filter before verification, per Q6): consistent across FR-013, US3 AS-4, US3 Independent Test step 4, plan.md Technical-approach bullet, tasks T018.
  - **Dependency surface**: three runtime (`rand` / `rand_chacha` / `sha2`) + one test-only (`proptest`) consistent across plan.md Technical Context, plan.md Constitution-Check §"Justified dependencies", research.md §5, tasks T002. (Per A-L1 above, the brief inline source-tree comment in plan.md line 122 lists only the runtime trio — judged a deliberate one-liner pointer, not drift.)

### Pass-1 Verdict (provisional — superseded by pass-2)

**Zero substantive findings at pass-1; four LOW-severity acknowledgments, all NO-OP.** The four prior `/speckit-checklist` passes (logged in commits `3e90063`, `b837518`, `2f990b6`, `7d628a6`) closed the spec-internal consistency surface; pass-1 of `/speckit-analyze` was a partial cross-artifact sweep but operated on context that had just survived a compaction event. The user requested a deeper pass-2 to verify nothing had been missed.

**Pass-1 declared convergence prematurely.** Pass-2 surfaced one MEDIUM finding (FR-019 carries pre-Q6 ordering language that contradicts FR-013) plus two additional LOW cascade-drift items that pass-1's sweep missed. See pass-2 below.

---

## Pass-2 Findings (2026-06-04) — deep walk after compaction

Pass-2 loaded **all** supporting artifacts in full: spec.md (215 lines), plan.md (191 lines), tasks.md (249 lines), research.md (10 sections), data-model.md (19 sections + FR matrix), contracts/library-api.md (full Rust surface delta), quickstart.md (10 sections), ADR 0009 (full), ADR 0010 (full), IMPLEMENTATION_NOTES.md N-003 / N-004 / N-005, the constitution, the four-pass requirements checklist, and ROADMAP §4 process notes. The deep read surfaced findings pass-1's progressive-disclosure approach missed.

### Pass-2 Findings Table

| ID | Category | Severity | Location(s) | Summary | Recommendation |
|----|----------|----------|-------------|---------|----------------|
| F-M1 | Inconsistency / Conflict | **MEDIUM** | `spec.md` FR-019 (line 165) — three sub-clauses | FR-019 carries pre-Q6 ordering language that contradicts FR-013's Q6 topic-filter-first decision in three places: (1) "the **post-verification** topic-filter check (002 FR-004)"; (2) "a `subscribe(T)` returning `Added` on another task MUST be visible to the topic-filter check on any inbound message whose **signature verification began** after the `subscribe` call returned"; (3) "the linearizability requirement is on the **filter+snapshot path that follows verification**, not on the verification step itself." All three statements describe filter-AFTER-verification ordering. Q6 resolved the order to **filter FIRST, then verification** — encoded in FR-013, US3 AS-4, US3 Independent Test step 4, plan.md Technical-approach, data-model.md §17 pipeline diagram, and contracts/library-api.md pipeline diagram. The Q6 cascade missed FR-019 entirely. Substantively, FR-019's linearizability claim is correct regardless of ordering (the verifier is stateless; the filter+snapshot path is mutex-protected); only the framing wording is stale. T018 implements FR-013's ordering correctly — implementation is unaffected. But the spec contains a real internal contradiction in normative text. | **Rewrite FR-019** to use the post-Q6 ordering: state the linearizability requirement over (topic-filter check → signature verification → snapshot append) as a sequence; drop the three "post-verification" / "follows verification" framings. The linearizability anchor remains 002 FR-015 (subscription-set mutations visible at the filter point); the verifier itself is stateless so its addition is pure-function-clean. Spec-edit only; no plan / data-model / tasks / contracts / quickstart changes required (those are already Q6-correct). |
| F-L1 | Inconsistency | LOW | `tasks.md` T020 | T020 says "Update each such site in the same commit as T018 **ideally, OR as a follow-up commit immediately after T018** if scope keeps T018's commit focused." This is softer than FR-015's MUST language: "Any 002-era test that filters log capture on `event == \"topic_drop\"` **MUST be updated in the same commit** to filter on `event == \"message_dropped\"` AND `cause == \"topic_not_subscribed\"`." In practice T020 is expected to be a no-op (most 002 tests do not filter on log content per the tests-don't-check-logs convention from FR-014 + 002 FR-011 / FR-014), but the wording loosens FR-015's atomicity. | Tighten T020 phrasing: "MUST land in the same commit as T018 per FR-015 — in practice this task is typically a no-op because the tests-don't-check-logs convention means few or no 002 tests filter on `event == \"topic_drop\"`." OR leave the practical expectation note and just drop the "OR as a follow-up commit" clause. |
| F-L2 | Inconsistency (cascade drift — pass-3 miss) | LOW | `plan.md` line 122 (Project Structure source-tree inline comment) | Inline `Cargo.toml` change note reads `# extended: + rand, rand_chacha, sha2 (all in [dependencies])` — does not echo proptest. Pass-3's CHK078 fix patched plan.md Summary (line 22) and Constitution-Check §"Justified dependencies" (line 78) to acknowledge proptest as a 4th `[dev-dependencies]` entry, but this third site (line 122 inside the Source-Code tree block) is still pre-pass-3 wording. Same cascade as CHK078, missed by pass-3's grep. Canonical Technical Context (plan.md lines 29–34) lists all four — no impact on `/speckit-implement` since the canonical surface is correct. | Update the inline comment to `# extended: + rand, rand_chacha, sha2 in [dependencies]; + proptest in [dev-dependencies]` for full cascade consistency with CHK078's spirit. |
| F-L3 | Documentation Staleness | LOW | `ROADMAP.md` §2 003 entry (carried over from CHK081) | ROADMAP §2 still describes 003 with pre-spec preview wording: 3 open questions + "TDD trigger: YES. Chain integrity (parent-hash linkage, sequence monotonicity) and authenticity (signature binding) are protocol-behavior claims." Spec FR-016 + N-003 supersede: chain integrity is **deferred** until 008 / 012; the TDD trigger applies to **signature authenticity only** in 003. CHK081 deferred this per ROADMAP §4's "working document" stance. | NO-OP for `/speckit-analyze` (deferred per CHK081 + ROADMAP §4). Optional polish if the project decides to sync ROADMAP entries with landed-feature artifacts at some checkpoint. |
| F-L4 | Coverage Note (NO-OP, from pass-1) | LOW | `spec.md` FR-018 (011 swap-readiness) | FR-018 is verified architecturally — by the trait-shape choices in T004 (concrete byte newtypes) + T006 (no associated types) + T010 (mock impl preserves trait surfaces) — and has no test anchor in 003 because there is no 011 `Ed25519Verifier` yet to swap against. The invariant becomes test-anchored when feature 011 lands and verifies the trait surfaces did not budge. | NO-OP. Forward-looking architectural invariant correctly framed in spec text. |
| F-L5 | Coverage Note (NO-OP, from pass-1) | LOW | `spec.md` SC-008 (operator-visible at default log level) | Intentionally not test-anchored per FR-014's tests-don't-check-logs convention. Symmetric to 002 SC-006. | NO-OP. Convention explicit; correctly framed as operator UX. |
| F-L6 | Style (NO-OP, from pass-1) | LOW | `tasks.md` T013 inline byte-layout pseudocode | T013's task body embeds ~30 lines of inline byte-layout pseudocode duplicating data-model §16b + research §7. | NO-OP. `/speckit-tasks` outputs are designed to be self-contained for `/speckit-implement`; trimming would force context-switches mid-task. |

### Detection-Pass Re-Sweep (deep)

**A. Duplication**: zero substantive duplicates. FR-013 (ordering) vs FR-020 (task confinement) cover distinct concerns (which-runs-first vs which-task-hosts-the-step); FR-014 vs FR-015 share the `message_dropped` event marker by deliberate cross-feature convention; SC-001 vs SC-002 cover the accept vs reject case of US1 (distinct demonstrations).

**B. Ambiguity**: zero vague adjectives in normative text. Only place worth flagging was FR-019's "post-verification" wording — but that's a contradiction (F-M1), not ambiguity. No TODO / TKTK / `[NEEDS CLARIFICATION]` / placeholder markers.

**C. Underspecification**: zero. Negative requirements (FR-016 "MUST NOT inspect chain integrity", FR-017 "MUST NOT change TOML", FR-018 "MUST remain identical across 011 swap") each enumerate the deferred items explicitly with revisit triggers traced to N-003 / inherited contracts / forward-looking architecture.

**D. Constitution Alignment**: cross-checked against `.specify/memory/constitution.md` v1.0.0 (Principles I–V + Engineering Standards + Development Workflow). All five principles pass per plan.md Constitution Check both gates (initial pre-Phase-0 and post-Phase-1). TDD obligation (Principle II's "envelope handling, message verification" carve-out) correctly encoded in T016 (red-green-first); chain-integrity tests correctly scoped out via FR-016 + N-003. Property-based testing (Engineering Standards) realized via T023's proptest. Reproducible tests via seeded PRNG (`MockCryptoScheme::with_seed`) + `Timestamp::from_millis` for deterministic timestamps. Justified dependencies covered in plan.md §"Justified dependencies" (ADR 0009 covers the structural choice; Constitution's exemption clause covers `rand` / `sha2` / `proptest` as standard-Rust-ecosystem crates). Green checkpoints + logical increments encoded in T001 / T024 + the breaking-change atomicity notes on T012 / T013.

**E. Coverage Gaps**: every FR has at least one task (matrix mirrors data-model §19); every user story has at least one test task; every buildable SC has at least one task; SC-008 is intentionally operator-UX-only.

**F. Inconsistency / Terminology Drift**: one substantive finding (F-M1 above). Everything else cross-checked clean — `Message` enum shape, `MessageHash::of(&PlainMessage)` content-anchored input, `signed_bytes` seam on `PlainMessage` (not `Message`), `RoutingFrame` rename, drop-event shape (`event = "message_dropped"` + `cause` field), test-helper signatures, dep-list surface, ADR cross-references (FR-by-FR), N-entry cross-references, and tests-don't-check-logs convention all consistent across the seven core artifacts plus the two ADRs plus IMPLEMENTATION_NOTES.

### Pass-2 Verdict

**One MEDIUM finding + two LOW cascade-drift items + three LOW NO-OP acknowledgments.**

F-M1 is a real spec-internal contradiction that survived four `/speckit-checklist` passes (CHK068 caught the snapshot-append citation typo inside the same FR-019 paragraph but did not question the surrounding "post-verification" framing). The Q6 cascade scope was incomplete; FR-013 + US3 received the Q6 updates while FR-019 did not. The substantive linearizability claim is still correct, and the implementation in T018 follows Q6 correctly — so this is a "spec internally contradicts itself in normative text" finding, not a "implementation would be wrong" finding. Still worth a spec edit before `/speckit-implement` to avoid a fresh reviewer surfacing it later.

F-L1 and F-L2 are minor cascade-drift cleanups consistent with the pass-3 CHK078-shape pattern (proptest acknowledgment + atomic-commit phrasing) that previous passes' greps missed. F-L3 is the pre-existing CHK081 deferral. F-L4 / F-L5 / F-L6 are NO-OP acknowledgments carried from pass-1.

### Pass-2 Metrics

- Total Requirements: 20 FR + 8 SC = 28
- Total Tasks: 28
- Coverage %: 100% (all FRs and all buildable SCs)
- Ambiguity count: 0 (no vague adjectives in normative FRs; no unresolved placeholders)
- Duplication count: 0
- Critical issues: 0
- High issues: 0
- **Medium issues: 1** (F-M1 — FR-019 ordering contradiction)
- Low issues: 6 (3 substantive minor + 3 NO-OP acknowledgments)

---

## Next Actions

**Substantive remediation** (recommended before `/speckit-implement`):

- **F-M1**: edit `spec.md` FR-019 to drop the three "post-verification" / "filter+snapshot path that follows verification" framings; restate the linearizability requirement over the post-Q6 ordering (topic filter → verification → snapshot append). Spec edit only — plan / data-model / tasks / contracts / quickstart are already Q6-correct.

**Minor remediation** (optional but consistent with pass-3 CHK078's cleanup):

- **F-L1**: tighten `tasks.md` T020 phrasing to match FR-015's MUST same-commit language (drop "OR as a follow-up commit"; note the practical no-op expectation under FR-014's tests-don't-check-logs convention).
- **F-L2**: update `plan.md` line 122 inline source-tree comment to echo proptest alongside the runtime trio.

**Deferred** (not blocking):

- **F-L3**: ROADMAP.md §2 003 entry sync — per CHK081 + ROADMAP §4's "working document" stance.

**Then proceed to `/speckit-implement`** — per ROADMAP §4's session-boundary guidance (commit `7e758a0`), run in a fresh Claude Code session for clean code-generation context.

---

## Pass-2 closure (2026-06-04)

The three substantive findings from pass-2 are FIX-APPLIED in the same commit as this closure note.

- **F-M1 → spec.md FR-019**: three pre-Q6 ordering phrasings replaced with post-Q6 (filter-first) language. Additionally tightened during the fix-apply (per the clarifying-question exchange that confirmed the actual concurrency model) to drop the "concurrent inbound messages can be verified without contention" wording — which was forward-looking for 011 Ed25519 offload but could be misread as in-Node parallelism that doesn't exist — replaced with explicit serial-receive-task framing + single-writer snapshot invariant + forward-looking note for 011's CPU-heavy verifier offload per FR-020.
- **F-L1 → tasks.md T020**: "OR as a follow-up commit immediately after T018" softening removed; T020 now reads "MUST land in the same commit as T018, per FR-015's MUST-same-commit atomicity requirement" with a practical no-op expectation note under FR-014's tests-don't-check-logs convention.
- **F-L2 → plan.md line 122**: inline source-tree `Cargo.toml` comment now echoes `proptest` in `[dev-dependencies]` alongside the runtime trio in `[dependencies]`, completing the CHK078 cascade pass-3's grep missed.

**Green-checkpoint sweep**: `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` + `cargo build` + `cargo test` all pass (doc-only edits on Spec-Kit artifacts; code state unchanged from `2de9d90`).

**Pass-2 verdict (final)**: 003 artifact set is consistent post-fix. The one remaining LOW item (F-L3 — ROADMAP.md §2 003 entry staleness) stays deferred per CHK081 + ROADMAP §4's working-document stance; not blocking. **Ready for `/speckit-implement`.**

### Convergence trajectory (analyze + checklist combined)

Mirrors 001's precedent recorded in ROADMAP §4 ("severity trends 9 → 6 → 5 → 0 on 001; pass-1 fixes structural issues; pass-2 cleans up cascades; pass-3 is polish; pass-4 is confirmation") and the checklist's own four-pass convergence on 003 (CHK023/043 cascade → CHK067/068/069 → CHK078 → zero):

| Pass | Surface | Substantive findings | Notes |
|---|---|---|---|
| Checklist pass-1 | Spec-internal | 9 | CHK023/043 cascade (10 sites), CHK022/046, CHK024/044, CHK025, CHK027, CHK047, CHK048, CHK054 (3 sites) |
| Checklist pass-2 | Spec-internal | 3 + 1 minor | CHK067 (N-004), CHK068 (FR-019 cite), CHK069 (plan.md proptest), CHK075 (CLAUDE.md N-005) |
| Checklist pass-3 | Spec-internal | 1 | CHK078 (plan.md Summary + Constitution-Check proptest cascade); CHK081 deferred |
| Checklist pass-4 | Spec-internal | 0 | Zero-finding closure |
| Analyze pass-1 | Cross-artifact (partial — post-compaction) | 0 (provisional) | Pass-1 declared premature convergence |
| **Analyze pass-2** | **Cross-artifact (deep)** | **1 MEDIUM + 2 LOW** | **F-M1 (FR-019 ordering), F-L1 (T020 phrasing), F-L2 (plan.md inline comment) — all FIX-APPLIED in this commit** |

The deep cross-artifact sweep caught what the spec-internal checklist passes (CHK068 in particular) could not see by their narrower scope: the Q6 cascade had updated FR-013 + US3 but missed FR-019's surrounding ordering framing.

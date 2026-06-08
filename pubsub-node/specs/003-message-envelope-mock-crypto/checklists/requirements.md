# Specification Quality Checklist: Message Envelope + Mock Crypto

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-06-03
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

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

- Items marked incomplete require spec updates before `/speckit-clarify` or `/speckit-plan`
- This spec was authored after an extensive pre-spec design discussion (recorded
  in ADR 0009, IMPLEMENTATION_NOTES.md N-003 and N-004, and the saved memory
  `feedback_message_dropped_event_convention`). The structural decisions that
  would normally be open at the spec stage — crypto trait shape, mock-crypto
  factory, canonical encoding, parent_hash type+encoding, timestamp shape,
  drop-event convention — were pinned during pre-spec, so this spec captures
  them as normative requirements rather than as open questions.
- The "no implementation details" criterion is interpreted in this project to
  mean "no language/framework choices that aren't already established by
  prior ADRs". Per the project convention shared with 001/002 specs, type
  names like `Signer`, `Verifier`, `MockCryptoScheme`, `Message`, `PublicKey`,
  `PublisherId` etc. are treated as protocol-level entity names that the
  spec normatively defines; this is the precedent set by 002's spec, which
  also mentions concrete type names (`TopicId`, `SubscribeOutcome`, `NodeConfig`).
  The library is the product; its public type surface is in scope for the
  spec to fix.
- FR-007 mentions `rand_chacha::ChaCha20Rng` by name. This is necessary because
  the PRNG choice affects test reproducibility — naming the specific RNG locks
  the byte-stream contract that test assertions in US4 depend on. Same
  reasoning extends to the `sha2` crate (FR-011) and to the `rand` /
  `rand_chacha` direct-dependency additions in the Assumptions section.
- US3's filter-ordering claim (signature check before topic check) is captured
  in FR-013 specifically because some US3 acceptance scenarios assert on the
  `cause` field — the ordering pins which cause wins when both filters would
  reject.

---

## Pre-/speckit-tasks Readiness Checklist (Pass 1)

**Created**: 2026-06-03
**Purpose**: validate the cross-artifact consistency of the spec, plan, research, data-model, contracts/, quickstart, ADRs 0009/0010, and IMPLEMENTATION_NOTES entries after the ADR 0010 type-hierarchy restructure, and confirm readiness for `/speckit-tasks`. Each item tests **the quality of the requirements / planning artifacts** ("Unit Tests for English" per the `/speckit-checklist` framing) — NOT implementation behavior.

Items are numbered starting at CHK017 to follow the 002 convention (CHK001–CHK016 conceptually correspond to the auto-generated Specification Quality Checklist items above; CHK017+ are the `/speckit-checklist` Pre-`/speckit-tasks` Readiness Pass items).

### Cross-Artifact Consistency (post-ADR-0010 cascade detection)

- [x] CHK017 - Is the `Message` reshape (from struct to `#[non_exhaustive]` enum) consistently described across spec.md FR-001, plan.md Summary + Technical Approach, data-model.md §16, contracts/library-api.md Message table, quickstart.md type tour, and ADR 0010? [Consistency, Spec §FR-001, ADR 0010] _verified: all six artifacts describe the `#[non_exhaustive] enum Message { Signed(SignedMessage) }` shape consistently._
- [x] CHK018 - Is the `Message::Signed(SignedMessage)` variant described with identical field surfaces in spec.md FR-001, data-model.md §16a, and contracts/library-api.md (no drift in field names or types)? [Consistency, Spec §FR-001] _verified._
- [x] CHK019 - Are `SignedMessage`'s two fields (`plain: PlainMessage`, `signature: Signature`) defined identically in data-model.md §16a and contracts/library-api.md, and consistent with spec.md FR-001's prose description? [Consistency, Spec §FR-001] _verified._
- [x] CHK020 - Is `PlainMessage`'s field set (topic, publisher_id, parent_hash, sequence, timestamp, payload — signature excluded) defined identically in spec.md FR-001, data-model.md §16b, and contracts/library-api.md? [Consistency, Spec §FR-001] _verified._
- [x] CHK021 - Is `MessagePayload`'s preservation (unchanged from 002, `#[non_exhaustive]`, sole `Ping(u64)` variant) consistently described in spec.md FR-001, data-model.md §16c, plan.md Project Structure, and contracts/library-api.md? [Consistency, Spec §FR-001] _verified._
- [x] CHK022 - Is the rename of 001's `Envelope` routing wrapper to `RoutingFrame` consistently mentioned in spec.md FR-001 + Assumptions terminology bullet, plan.md Project Structure + Summary, data-model.md §16d, contracts/library-api.md re-exports + What-does-NOT-change, quickstart.md, and ADR 0010? [Consistency, ADR 0010] _resolved: data-model.md §0 opening prose updated in this pass to flag the rename (was referencing the 001-era `Envelope` without noting the rename); all other artifacts already consistent._
- [x] CHK023 - Is `PlainMessage::signed_bytes` (not `Message::signed_bytes`) the canonical-encoding seam consistently across spec.md FR-010, data-model.md §16b, contracts/library-api.md PlainMessage table, quickstart.md tour, and IMPLEMENTATION_NOTES.md N-004? [Consistency, Spec §FR-010] _resolved: ten cascade drift sites updated in this pass (spec.md FR-004 + Key Entities MessageHash + Assumptions Canonical-encoding bullet + US1 Independent Test; plan.md Constitution-Check green-checkpoints note; research.md §7 explicit-match heading; data-model.md §4 MessageHash ZERO + of(); contracts/library-api.md MessageHash ZERO + Signer::sign rows)._
- [x] CHK024 - Is the no-placeholder signing workflow consistently described across spec.md FR-010, research.md §4, contracts/library-api.md PlainMessage table, and quickstart.md type tour? [Consistency, Spec §FR-010] _resolved: see CHK044 — `Signature::placeholder()` removed from data-model.md §3 and contracts/library-api.md Signature table; signing workflow now reads "PlainMessage → sign(plain.signed_bytes()) → SignedMessage" without a placeholder step everywhere._
- [x] CHK025 - Is `MessageHash::of(&PlainMessage)` (not `&SignedMessage` or `&Message`) consistently specified across spec.md FR-011, ADR 0010 Consequences, contracts/library-api.md MessageHash table, IMPLEMENTATION_NOTES.md N-005, and research.md §2? [Consistency, Spec §FR-011] _resolved: data-model.md §4 MessageHash::of signature updated from `&Message` to `&PlainMessage` in this pass; all other artifacts already consistent._
- [x] CHK026 - Is the content-anchored hash rationale (signature-malleability immunity, Cardano `tx_hash = blake2b(body)` alignment, content-addressing) consistently summarized across spec.md FR-011 + Clarifications, ADR 0010 Consequences, and IMPLEMENTATION_NOTES.md N-005? [Consistency] _verified: rationale appears in all three locations with the same four-point structure (malleability immunity, Cardano alignment, content addressing, cross-scheme stability)._
- [x] CHK027 - Is the receive-task pattern-match (`match frame.message { Message::Signed(signed) => { /* filter → verify → snapshot */ } }`) consistently described in spec.md FR-013, FR-020, data-model.md §17, and contracts/library-api.md Receive-task pipeline section? [Consistency, Spec §FR-013, §FR-020] _resolved: data-model.md PublisherId `as_public_key` description was the only drift site (used `msg.publisher_id` instead of `signed.plain.publisher_id`); fixed in this pass. FR-013 + FR-020 + the pipeline diagrams all agree._
- [x] CHK028 - Is the topic-filter-first ordering (Q6) consistently described in spec.md FR-013, US3 acceptance scenarios, data-model.md §17, and contracts/library-api.md? [Consistency, Spec §FR-013] _verified: round-4 Q6 closure (commit `4fd16db`) brought all four artifacts into alignment; no post-ADR-0010 drift detected._
- [x] CHK029 - Is the test-support helper (`build_signed_message`, `build_signed_message_simple`) returning `Message` (the enum, post-ADR-0010) consistent between research.md §4 and quickstart.md / plan.md Project Structure references? [Consistency, research.md §4] _verified: research.md §4 was rewritten for ADR 0010 with the new return-shape; quickstart.md type tour and plan.md Project Structure both reference the helper consistently._
- [x] CHK030 - Is the migration order (research.md §6's eight steps) reflected accurately in plan.md Project Structure's commit-grouping prose and consistent with the Constitution's green-checkpoints rule? [Consistency, research.md §6] _verified: research.md §6 lists 8 ordered steps, each shaped as a green-checkpoint commit; plan.md Constitution Check + Project Structure align with the same ordering._

### Requirement Clarity & Measurability

- [x] CHK031 - Is the precise byte layout of `PlainMessage::signed_bytes` unambiguously specified in FR-010 (field order, u32-BE length-prefixes, the 32-byte fixed-width `parent_hash` slot, the `MessageHash::ZERO` sentinel, the `MessagePayload` variant tag values, endianness of sequence and timestamp)? [Clarity, Spec §FR-010] _verified: FR-010 (post-ADR-0010) names every field with width and endianness; the rustdoc obligation locks this further at implementation time._
- [x] CHK032 - Is `MessagePayload::Ping`'s variant tag value (`0x00`) explicitly fixed in FR-010 and referenced consistently in research.md §7's variant-tag-stability mechanism? [Clarity, Spec §FR-010] _verified: FR-010 pins `0x00` for `Ping`; research.md §7 explicit-match mechanism prevents tag drift via reordering._
- [x] CHK033 - Are the `Display` formats for `PublicKey`, `Signature`, `MessageHash`, `PublisherId` unambiguously specified as full lowercase hex in FR-003 + Clarifications Q4, with `PrivateKey` explicitly excluded? [Clarity, Spec §FR-003] _verified: FR-003 Display bullet names each type explicitly, including the no-Display rule for PrivateKey._
- [x] CHK034 - Is the `PrivateKey` discipline (no derived `Debug`, no `Display`, no `Hash`, hand-written redacting `Debug` impl) unambiguously specified in FR-003 + Clarifications Q3? [Clarity, Spec §FR-003] _verified: FR-003 PrivateKey bullet pins all four properties._
- [x] CHK035 - Is the property-based signature-binding test described in research.md §8 measurable and verifiable (specific invariant + clear input space)? [Measurability, research.md §8] _verified: §8 names the invariant ("verifier accepts the matching signer's signature; rejects any modified (key, msg, sig)") and the input space ((seed, msg) pairs) — `proptest`-amenable._
- [x] CHK036 - Is "operator-visible at default log level" in SC-008 measurable as written, and clearly separated from the test-anchored portion (`received_messages()` absence assertion)? [Measurability, Spec §SC-008] _verified: SC-008 post-Q4-convention-reminder explicitly separates the operator-UX criterion from the test-anchored one._
- [x] CHK037 - Is the rustdoc-as-protocol-surface contract clear about which artifacts trigger same-commit rustdoc updates (FR-010 for `PlainMessage::signed_bytes`; SC-006 for the MOCK warning's four locations)? [Clarity, Spec §FR-010, SC-006] _verified: FR-010 final sentence + SC-006 enumerate the rustdoc obligations explicitly._

### Coverage

- [x] CHK038 - Does every 003 FR (FR-001 through FR-020) have at least one entity entry in data-model.md and at least one contract clause in contracts/library-api.md? [Coverage, data-model.md §19] _verified: data-model.md §19 FR matrix is complete for FR-001 through FR-020._
- [x] CHK039 - Does every US1–US4 acceptance scenario map to ≥1 FR for its test-anchored assertion (received_messages() presence/absence)? [Coverage] _verified: every US AS references received_messages() (presence/absence) — directly anchored on FR-013 (receive-task ordering) + 001 FR-006 (snapshot append, transitive)._
- [x] CHK040 - Are the four scenario classes (valid+on-topic, valid+off-topic, invalid+on-topic, invalid+off-topic) all covered by US1+US3 acceptance scenarios after the Q6 ordering update? [Coverage, Spec §US1, US3] _verified: US3 AS-1/AS-2/AS-3/AS-4 cover the full 2×2 matrix; US1 AS-1/AS-2 cover valid/invalid on-topic._
- [x] CHK041 - Are all Edge Cases in spec.md mapped to either an FR (covered) or an explicit deferral (N-001 / N-002 / N-003 / N-005)? [Coverage, Edge Cases] _verified: every Edge Case bullet points either to an FR (FR-009, FR-013, FR-016) or to a deferral (N-002 self-addressing; N-003 chain integrity / replay; inherited 001/002 cases)._
- [x] CHK042 - Is the future-Message-variant story (when 004 / 005 / 008 / 010 add ConnectionHello / PeerSample / etc.) described enough in ADR 0010 to scope the receive-task pattern-match extension, without prematurely introducing variants in 003? [Coverage, ADR 0010] _verified: ADR 0010 Context section lists the anticipated future variants by feature; the Decision + spec FR-013 explicitly leave the catch-all arm to be added when those features land._

### Ambiguities & Conflicts (residual after Q1–Q6 + ADR 0010)

- [x] CHK043 - After ADR 0010, does any artifact still reference `Message::signed_bytes` (old shape) instead of `PlainMessage::signed_bytes` (new shape)? [Ambiguity, Conflict] _resolved (see CHK023 detail): ten drift sites fixed across spec.md / plan.md / research.md / data-model.md / contracts/library-api.md in this pass. The spec.md Input field (line ~11) preserves the original user prompt verbatim and is intentionally left as historical record._
- [x] CHK044 - After ADR 0010, does any artifact still describe `Signature::placeholder()` or a "set the signature back" step in the signing workflow? [Ambiguity, Conflict] _resolved: per the user's option-A decision in this pass, `Signature::placeholder()` is removed from the data-model.md §3 Signature entity's construction surface AND from the contracts/library-api.md Signature table; replaced with a note that US1 AS-3's deliberately-wrong-signature case uses `Signature::new(vec![0u8; 32])` directly. ADR 0010 + research.md §4 + quickstart.md tour were already on the no-placeholder workflow._
- [x] CHK045 - Does any artifact still describe `Message` as a struct (rather than an enum) after the ADR 0010 reshape? [Ambiguity, Conflict] _verified: every reference to `Message { topic, payload }` is in migration / historical context describing the OLD shape being replaced; no artifact currently describes Message as a struct prescriptively._
- [x] CHK046 - Does any artifact still use the 001 `Envelope` name (rather than `RoutingFrame`) in a post-ADR-0010 context, including the `pubsub_node::network::Envelope` path? [Ambiguity, Conflict] _resolved: data-model.md §0 opening prose updated in this pass (was still referring to "001-era Envelope" without the rename note). All other mentions are in renamed-to-RoutingFrame contexts (correct) or historical / migration prose (correct)._
- [x] CHK047 - Does any artifact's "publisher_id carrying random non-derived bytes" Edge Case still describe the publisher_id as a Message field (rather than a `PlainMessage` field)? [Consistency, Edge Cases] _resolved: spec.md Edge Cases bullet tightened in this pass to construct via `PublisherId::from(PublicKey::new(vec![0u8; 8]))` written into `plain.publisher_id`._
- [x] CHK048 - Are there any references to the term "envelope" in spec/plan/data-model/contracts/quickstart that mean the **PlainMessage** shape, contradicting the Assumptions terminology bullet's claim that prose-level "envelope" = whole signed message? [Ambiguity, Terminology] _resolved: contracts/library-api.md "Node::send" section had a "Message argument is now the 7-field envelope" residue (old shape) — updated in this pass to reflect the enum / inner-PlainMessage layering. All other prose uses of "envelope" mean the whole signed message (synthesis-aligned)._

### Traceability

- [x] CHK049 - Is ADR 0009 cited from every FR that depends on its decisions (crypto trait shape, no-Signer-on-Node, mock construction — FR-002/003/004/005/006/007/008/009/012/018)? [Traceability, ADR 0009] _verified: each of those FRs cites ADR 0009 directly or via an intermediate clarification bullet._
- [x] CHK050 - Is ADR 0010 cited from every FR that depends on its decisions (Message enum, SignedMessage / PlainMessage split, RoutingFrame rename, MessageHash content-anchored — FR-001/010/011/013/018/020)? [Traceability, ADR 0010] _verified: FR-001 / FR-010 / FR-011 / FR-013 / FR-018 / FR-020 all cite ADR 0010 explicitly._
- [x] CHK051 - Is N-005 cross-referenced from FR-011 (spec), ADR 0010 (Consequences), and quickstart.md (deferred-bits list)? [Traceability, N-005] _verified: N-005 appears in all three locations as required._
- [x] CHK052 - Is N-003 cross-referenced from FR-016 (signature-only validation deferral) and Edge Cases (chain-integrity deferred bullets)? [Traceability, N-003] _verified: FR-016 cites N-003 directly; Edge Cases first-message and replay bullets cite N-003._
- [x] CHK053 - Is N-004 cross-referenced from FR-010 (canonical encoding migration trigger) and quickstart.md (deferred-bits list)? [Traceability, N-004] _verified: FR-010 cites N-004 in two places (rustdoc-protocol-surface guidance + version-tag-omission rationale); quickstart.md deferred-bits list cites N-004._
- [x] CHK054 - Are all 002-FR cross-references in 003 (e.g., 002 FR-004 cited in 003 FR-013; 002 FR-006 cited in 003 FR-013 snapshot append) accurate after the architectural restructure? [Traceability] _resolved: 002 FR-006 (subscribe/unsubscribe mutator API) was incorrectly cited for snapshot-append behavior in spec.md FR-013, data-model.md §17 pipeline diagram, and contracts/library-api.md pipeline diagram; corrected in this pass to "001 FR-006 snapshot-append contract — extended by 002 FR-004 with the topic-filter precondition." All other 002-FR citations (FR-004, FR-005, FR-011, FR-014, FR-015) verified accurate._

### Pre-/speckit-tasks Readiness

- [x] CHK055 - Is the migration order in research.md §6 task-decomposable — each of the 8 steps shapes into a single coherent commit that leaves the crate green per the Constitution's green-checkpoints rule? [Readiness, research.md §6] _verified: each of the 8 steps maps to a single coherent commit-shape; step-3 RoutingFrame rename is mechanical, step-4 Message reshape is acknowledged as the largest single commit, every step leaves the crate green._
- [x] CHK056 - Is the `src/crypto/mod.rs` + `src/crypto/mock.rs` introduction broken into orderable sub-steps in research.md §6 (types/traits first, mock impls second) so `/speckit-tasks` can derive separate test-first task pairs? [Readiness, plan.md Project Structure] _verified: research.md §6 steps 1–2 split the crypto module introduction; step 1 (types + traits, no impls) compiles green with no callers, step 2 (mock impls) compiles green because the traits exist._
- [x] CHK057 - Is the Message-enum reshape (research.md §6 step 4) clearly flagged as the single largest commit, with the rationale that the multi-file edit MUST be coherent (no partial migration)? [Readiness, research.md §6] _verified: §6 step 4 explicitly says "this is the largest single-commit migration in 003" with the coherence rationale._
- [x] CHK058 - Are the four new test files (signed_message.rs, multi_publisher.rs, filter_composition.rs, mock_crypto_repro.rs) identified with their per-US FR coverage in research.md or quickstart.md so each can become a Phase task? [Readiness] _verified: plan.md Project Structure lists each test file with its US coverage; quickstart.md §§2–5 walk each file._
- [x] CHK059 - Is the property-based test (research.md §8) explicitly task-decomposable — separate from the example-driven acceptance-scenario tests — so the constitution's "property-based testing for critical properties" rule is task-trackable? [Readiness, Constitution Engineering Standards] _verified: research.md §8 names the proptest crate and the invariant; quickstart.md §5 lists it as a discrete test (`signature_binding_proptest`) alongside the example-driven tests._
- [x] CHK060 - Are the `Cargo.toml` dep additions (rand, rand_chacha, sha2 in `[dependencies]`; proptest in `[dev-dependencies]`) clearly task-shaped — a discrete dep-addition commit, separate from impl commits? [Readiness, research.md §5] _verified: research.md §5 specifies the version pins and placement; plan.md Technical Context names them explicitly. `/speckit-tasks` can schedule a discrete "add deps" task._
- [x] CHK061 - Is the Constitution's TDD trigger (signature authenticity tests BEFORE the verification-step implementation per Principle II + the "envelope handling, message verification" carve-out) clearly translatable to task ordering in `/speckit-tasks`? [Readiness, Constitution Principle II] _verified: plan.md Constitution Check explicitly names the TDD obligation and identifies the test-before-impl ordering for the receive-task verification step. `/speckit-tasks` has the artifacts it needs to schedule the test tasks first._
- [x] CHK062 - Is the 002 `topic_drop` → `message_dropped` + `cause = "topic_not_subscribed"` rename clearly identified as same-commit-as-invalid_signature-emitter per FR-015 + SC-007's atomicity criterion? [Readiness, Spec §FR-015, §SC-007] _verified: FR-015 says the rename happens "in the same commit that lands the FR-014 emitter"; SC-007 enforces the rename atomicity via the `grep -r "topic_drop"` criterion._
- [x] CHK063 - Is the test-support helper (`build_signed_message`, `build_signed_message_simple` in `tests/common/mod.rs` per research.md §4) clearly task-shaped — distinct from the user-story test tasks it serves? [Readiness, research.md §4] _verified: research.md §4 isolates the helper as a separate concern; research.md §6 step 5 schedules it as a discrete commit before the user-story test tasks land._

---

**Walk procedure** (mirroring 002's pattern recorded in commits `8299c00` and `8afc2b5`):

1. Walk each item top to bottom; for each, decide PASS (✓ requirement is well-written) / FAIL (✗ requirement needs editing) / DEFER (⊘ raise later).
2. For each FAIL or DEFER, record the resolution inline as an `_resolved_` or `_deferred_` note with the commit / artifact / decision that closed it.
3. Substantive edits (cascade drifts found, missing cross-references, ambiguities surfaced) land as separate commits per the green-checkpoints rule.
4. If pass-1 surfaces ≥1 substantive edit, schedule a pass-2 walk to catch cascade drift from those edits (per ROADMAP §4's convergence rule).
5. When all items close (or are deferred with explicit triggers), proceed to `/speckit-tasks`.

---

## Pass-1 walk closure (2026-06-04)

All 47 items closed in a single pass. Breakdown:

- **31 PASS (verified clean)**: CHK017–CHK022 (Message reshape consistency, except CHK022 which surfaced a small data-model.md §0 fix), CHK026, CHK028–CHK030, CHK031–CHK037, CHK038–CHK042, CHK045, CHK049–CHK053, CHK055–CHK063.
- **9 FIX-APPLIED (substantive edits landed in this pass)**:
  - **CHK023 + CHK043 cascade** — ten sites updated from `Message::signed_bytes` to `PlainMessage::signed_bytes` (or `&Message` → `&PlainMessage` on `MessageHash::of`) across spec.md (FR-004, Key Entities, Assumptions), plan.md, research.md §7, data-model.md §4, contracts/library-api.md (MessageHash + Signer::sign).
  - **CHK024 + CHK044** — `Signature::placeholder()` removed from the API per user's option-A decision: data-model.md §3 Signature entity, contracts/library-api.md Signature table. Replaced with note pointing US1 AS-3 to `Signature::new(vec![0u8; 32])`.
  - **CHK022 + CHK046** — data-model.md §0 opening prose updated to flag the 001 `Envelope` → `RoutingFrame` rename (was using the unrenamed 001-era name).
  - **CHK025** — data-model.md §4 `MessageHash::of` signature changed from `&Message` to `&PlainMessage`.
  - **CHK027** — data-model.md PublisherId entity `as_public_key` description: `msg.publisher_id` → `signed.plain.publisher_id` to match the post-ADR-0010 receive-task call site.
  - **CHK047** — spec.md Edge Cases "publisher_id carrying random non-derived bytes" bullet tightened to construct via `PublisherId::from(PublicKey::new(...))` written into the `plain.publisher_id` field.
  - **CHK048** — contracts/library-api.md `Node::send` section: "The Message argument is now the 7-field envelope rather than a 2-field one" updated to reflect the post-ADR-0010 enum / inner-PlainMessage layering.
  - **CHK054** — referential typo fixed: `002 FR-006` (subscribe/unsubscribe mutator API) was cited for snapshot-append in three sites (spec.md FR-013, data-model.md §17 pipeline, contracts/library-api.md pipeline). Replaced with "001 FR-006 snapshot-append contract — extended by 002 FR-004 with the topic-filter precondition."
- **0 DEFER**, **0 FAIL** remaining open.

**Pass-2 needed?** Originally judged "no" at pass-1 closure — but a follow-up walk surfaced three drift sites pass-1's greps had missed. Pass-2 was justified; see the pass-2 section below.

**Verdict (initial, superseded by pass-2)**: declared "ready for `/speckit-tasks`" prematurely. Pass-2 closes the remaining cascade drift.

---

## Pass-2 walk closure (2026-06-04)

Triggered by the user's "do another pass in case you missed something" request. Pass-2 generated 12 items (CHK064–CHK075) covering: (a) verification that pass-1's substantive edits stuck, (b) cascade drift pass-1 missed, (c) new-angle checks pass-1 didn't fully cover.

### Pass-1 fix verification

- [x] CHK064 - Did pass-1's CHK023 + CHK043 cascade fix (`Message::signed_bytes` → `PlainMessage::signed_bytes`) close cleanly across spec.md FR-004 / Key Entities / Assumptions / US1 Independent Test, plan.md green-checkpoints note, research.md §7, data-model.md §4, and contracts/library-api.md (MessageHash + Signer::sign rows)? [Consistency, pass-1 verification] _verified: greps confirm only the spec.md Input field (preserved verbatim as user-prompt history) retains the legacy `Message::signed_bytes` string — that's intentional historical record, not an active prescription. All eight production sites are clean._
- [x] CHK065 - Did pass-1's CHK044 `Signature::placeholder()` removal leave any active residues? [Consistency, pass-1 verification] _verified: three remaining mentions are NEGATIVE references ("no Signature::placeholder() constructor needed") in research.md §4, ADR 0010 Consequences, and the pass-1 walk notes themselves. No active API definition remains in data-model.md or contracts/library-api.md._
- [x] CHK066 - Did pass-1's CHK054 fix (002 FR-006 → 001 FR-006 for snapshot-append) close cleanly at the three originally-identified sites (spec.md FR-013, data-model.md §17 pipeline, contracts/library-api.md pipeline)? [Consistency, pass-1 verification] _verified at those three sites; but a fourth site (FR-019) was missed — see CHK068._

### Pass-2 newly-surfaced findings (pass-1 misses)

- [x] CHK067 - Does `IMPLEMENTATION_NOTES.md` N-004 still reference the pre-ADR-0010 `Message::signed_bytes` name in its working-answer body and its rustdoc-guidance bullet? [Ambiguity, Conflict] _resolved: two cascade drift sites in N-004 updated in this pass — line 64 (working-answer "exposed as a single helper") and line 87 (rustdoc-as-protocol-surface guidance). Both now reference `PlainMessage::signed_bytes`. Pass-1 grepped only inside the 003 feature directory + ADR 0010; missed the workstream-level IMPLEMENTATION_NOTES.md sweep._
- [x] CHK068 - Does spec.md FR-019 cite `002 FR-006` for "post-filter snapshot append" (an instance of the same typo CHK054 caught at three other sites)? [Conflict, pass-1 miss] _resolved: FR-019 "post-filter snapshot append (002 FR-006)" updated to "post-filter snapshot append (001 FR-006 — extended by 002 FR-004 with the topic-filter precondition)". The other "002 FR-006" cite in the same FR-019 sentence ("linearized order of subscription-set mutations") is correct — 002 FR-006 IS the subscription-mutation API, so that citation stays as-is._
- [x] CHK069 - Does plan.md Technical Context's "Primary Dependencies" list acknowledge `proptest = "1"` as a 4th dep (in `[dev-dependencies]`) alongside the three runtime deps, given that research.md §8 explicitly added it? [Completeness, Consistency, pass-1 miss] _resolved: plan.md "Three additions" rewritten as "Three runtime additions plus one test-only addition" with proptest listed; the Engineering-Standards property-based testing bullet also tightened from "`e.g. proptest or hand-rolled quickcheck-style`" to a definite commitment to `proptest = "1"` per research.md §8's framework decision._

### New-angle checks pass-1 didn't fully cover

- [x] CHK070 - Are the `SignedMessage` Future-methods anticipations in data-model.md §16a and contracts/library-api.md (the `verify(&self, verifier: &impl Verifier)` thin helper + `message_hash(&self) -> MessageHash`) consistent with ADR 0010's "Alternatives considered" treatment of `signed.verify(...)` ("Deferred — could be added later as a thin helper without changing the trait surface. Out of scope for this ADR.")? [Consistency, Forward-compat] _verified: all three locations agree — anticipated but not added in 003; the alternative-considered note in ADR 0010 explicitly says "could be added later as a thin helper", matching the data-model.md and contracts/library-api.md "Future methods (anticipated, not 003)" framing._
- [x] CHK071 - Is the spec.md Input field's preservation of the pre-ADR-0010 wording ("a single Message::signed_bytes seam", etc.) the **only** remaining residue of the old shape, and is that preservation intentional / documented as historical? [Consistency, Historical Record] _verified: the Input field stores the user's original `/speckit-specify` prompt verbatim, and is the only site where pre-restructure wording remains. Per Spec Kit convention, the Input field is a historical record of what the user typed (not an active prescription); leaving it as-is is the right choice. The Clarifications section's "Q (post-plan, surfaced during ADR 0010 drafting)" bullet records the restructure that supersedes the Input wording, so a reader walking from Input → Clarifications → FRs sees the evolution clearly._
- [x] CHK072 - Does ADR 0009 forward-reference ADR 0010 (e.g., as a "see also" Sources entry), or is the asymmetric reference (ADR 0010 → ADR 0009 only) intentional? [Traceability, Forward-link] _verified: the asymmetry is appropriate. ADR 0009 was authored 2026-06-01 (during pre-spec); ADR 0010 was authored 2026-06-03 (post-Phase-1). An ADR doesn't normally forward-reference a not-yet-authored ADR; ADR 0010 backward-references ADR 0009 (correct). If a reader walking from ADR 0009 needs to find ADR 0010, the `docs/decisions/` directory listing and CLAUDE.md's SPECKIT block both surface 0010 alongside 0009. No fix needed; recording the asymmetry as deliberate._
- [x] CHK073 - Are the four new test files (signed_message.rs, multi_publisher.rs, filter_composition.rs, mock_crypto_repro.rs) appropriately scheduled in research.md §6's 8-step migration order — i.e., do they land AFTER the substrate (crypto module + Message reshape + RoutingFrame rename + receive-task verification step) is in place? [Readiness, Ordering] _verified: research.md §6 step 7 "Add the per-user-story tests for 003 (US1, US2, US3, US4)" — explicitly the final step after steps 1–6 (crypto module, mock impls, RoutingFrame rename, Message reshape, test helper, shared TestVerifier, receive-task verification + topic_drop rename). Ordering is correct for green-checkpoint commits._
- [x] CHK074 - Are US2 / US3 / US4 Independent Test wordings post-ADR-0010-aligned (no residual references to the pre-restructure `Message::signed_bytes` workflow or `Signature::placeholder()` semantics)? [Consistency, post-ADR-0010] _verified: US2 describes "three keypairs from one MockCryptoScheme, sign one message per publisher" at a high level without committing to specific type-construction syntax; US3 describes "Build a KeyPair and a valid TestSigner for it. Send four messages from B to A" — also abstraction-level. US4 uses MockCryptoScheme directly. None drift to pre-restructure shape._
- [x] CHK075 - Does CLAUDE.md's SPECKIT block accurately reference both ADRs (0009 + 0010) and the four IMPLEMENTATION_NOTES entries (N-001 / N-002 / N-003 / N-004 / N-005)? [Traceability, agent-context] _verified: CLAUDE.md SPECKIT block lists ADR 0009 + ADR 0010 explicitly, plus N-001 / N-002 / N-003 / N-004 (the previously-updated post-plan edit). N-005 was added in the post-plan ADR 0010 commit but the SPECKIT block was not updated to include it._

_minor — CHK075_: CLAUDE.md SPECKIT block doesn't list N-005 alongside N-001/N-002/N-003/N-004. Minor traceability gap. Resolved in this pass: CLAUDE.md updated to include "N-005 (MessageHash content-anchored input revisit trigger)" in the IMPLEMENTATION_NOTES inventory bullet.

### Pass-2 substantive edits landed by this walk

- **CHK067 → IMPLEMENTATION_NOTES.md N-004**: two cascade drift sites fixed (working-answer body + rustdoc-guidance bullet). Pass-1's grep missed N-004 because it scoped only the 003 feature directory + ADR 0010, not the workstream-level IMPLEMENTATION_NOTES.md.
- **CHK068 → spec.md FR-019**: "002 FR-006" → "001 FR-006" for the snapshot-append citation (fourth instance of the typo that pass-1 caught at three other sites).
- **CHK069 → plan.md Technical Context + Engineering-Standards bullet**: proptest acknowledged as a 4th dep (in `[dev-dependencies]`); Engineering-Standards bullet committed to proptest specifically per research.md §8's decision.
- **CHK075 → CLAUDE.md SPECKIT block**: N-005 added to the IMPLEMENTATION_NOTES inventory.

### Pass-2 verdict

12 items closed: 9 PASS (verifications) + 3 FIX-APPLIED (CHK067 / CHK068 / CHK069) + 1 minor FIX (CHK075). Pass-2 surfaced real cascade drift that pass-1's narrower greps had missed — the convergence rule's value (ROADMAP §4) is empirically confirmed.

**Pass-3 needed?** Likely no. The pass-2 fixes are all consistency cleanups in low-traffic spots (one citation, one dep-list re-shape, two N-004 sentences, one CLAUDE.md bullet). None introduces new cross-references or restructures any section. But the round-5 audit of spec.md showed that fresh sweeps can still surface residue, so a quick pass-3 verification grep is cheap insurance before `/speckit-tasks`.

**Verdict**: 003 artifact set is consistent post-pass-2. A small pass-3 confirmation sweep is recommended before invoking `/speckit-tasks`; if it surfaces nothing, the spec / plan / supporting docs are tasks-ready.

---

## Pass-3 walk closure (2026-06-04)

Triggered by the user's "do another pass in case you missed something again" request — the third pass of the ROADMAP §4 convergence rule (which observes that pass-3 typically polishes; pass-4 confirms zero findings; per 001's 9 → 6 → 5 → 0 severity trajectory). Pass-3 generated 7 items (CHK076–CHK082) verifying pass-2's fixes stuck and probing for any further residual cascade drift.

### Pass-2 fix verification

- [x] CHK076 - Did pass-2's CHK067 fix (IMPLEMENTATION_NOTES.md N-004's `Message::signed_bytes` → `PlainMessage::signed_bytes` cascade at lines 64 + 87) close cleanly with no further residues in N-004? [Consistency, pass-2 verification] _verified: grep on N-004 finds only `PlainMessage::signed_bytes` now; both fix sites stuck._
- [x] CHK077 - Did pass-2's CHK068 fix (spec.md FR-019 `002 FR-006` → `001 FR-006` for snapshot-append) close cleanly, leaving only the correctly-used `002 FR-006` cite for subscription-set mutations? [Consistency, pass-2 verification] _verified: FR-019 now reads "post-filter snapshot append (001 FR-006 — extended by 002 FR-004…)" and "linearized order of subscription-set mutations (002 FR-006)" — both cites accurate._
- [x] CHK078 - Did pass-2's CHK069 fix (plan.md acknowledging proptest as a 4th dep) propagate to **all** sites in plan.md that enumerate the deps, not just the Technical Context section? [Consistency, Coverage, pass-2 follow-up] _resolved: pass-2 patched only the Technical Context. Two more sites still said "Three new dependencies": plan.md line 22 (Summary section's "Three new dependencies" bullet) and line 78 (Constitution Check "Justified dependencies" bullet). Both updated in this pass-3 to acknowledge proptest as a 4th dep in `[dev-dependencies]`. Pass-2's grep scoped only the Technical Context heading; the cascade extended further._
- [x] CHK079 - Did pass-2's CHK075 fix (CLAUDE.md SPECKIT block adding N-005 to the IMPLEMENTATION_NOTES inventory) close cleanly? [Traceability, pass-2 verification] _verified: CLAUDE.md SPECKIT block now lists all five N-entries (N-001 through N-005) with brief descriptions of each._

### New-angle convergence checks

- [x] CHK080 - Is quickstart.md's "Three new direct dependencies (`rand`, `rand_chacha`, `sha2`) are added automatically the first time `cargo build` runs" line contextually accurate (proptest is `[dev-dependencies]` and only pulled by `cargo test`, not `cargo build`), or does it need updating to acknowledge proptest? [Consistency, Contextual Accuracy] _verified: the quickstart wording is contextually accurate — `cargo build` does not pull proptest (it's pulled by `cargo test`). The bullet describes the build-time dep tree only. No fix needed; the wording stays as-is._
- [x] CHK081 - Does the ROADMAP.md §2 entry for 003 still describe pre-spec open questions and a "TDD trigger: YES, chain integrity" claim that has been superseded by spec.md FR-016 (signature-only validation, chain integrity deferred per N-003)? [Stale Documentation, Traceability] _observation, not resolved: ROADMAP.md §2 entry for 003 still carries the pre-spec preview wording (3 open questions, "TDD trigger YES for chain integrity"). Per ROADMAP.md's own §4 process notes ("This roadmap is a meta-spec, not a contract. Features may be re-shuffled, dropped, or restructured as architectural understanding evolves"), the ROADMAP is a working preview, not retroactively-updated authoritative documentation. spec.md, ADR 0009, ADR 0010, and IMPLEMENTATION_NOTES.md N-003 / N-004 / N-005 collectively supersede the ROADMAP's 003 preview. Updating the ROADMAP entry is OPTIONAL and not blocking for `/speckit-tasks`; deferred as future polish if the project wants ROADMAP entries to stay in sync with landed feature artifacts._
- [x] CHK082 - Final residual-drift sweep across the workstream: any other artifacts still using pre-ADR-0010 wording, stale dep counts, incorrect 002-FR cites, or pre-restructure type-construction syntax? [Convergence, Final Sweep] _verified: after CHK078 fix, broad greps across pubsub-node/specs/003-message-envelope-mock-crypto/, pubsub-node/specs/IMPLEMENTATION_NOTES.md, pubsub-node/docs/decisions/0009*, pubsub-node/docs/decisions/0010*, and pubsub-node/CLAUDE.md show no residual drift sites. The only remaining `Message::signed_bytes` mention is in spec.md's Input field (preserved verbatim as user-prompt history per CHK071's deliberate decision)._

### Pass-3 substantive edits landed

- **CHK078 → plan.md Summary bullet + Constitution-Check Justified-dependencies bullet**: two cascade-drift sites pass-2 missed (focused on Technical Context only). Both now acknowledge proptest as a 4th dep in `[dev-dependencies]`, with the Constitution exemption rationale ("project's chosen test framework") and the Engineering-Standards-rule justification (signature binding qualifies as a property-level claim warranting proptest).

### Pass-3 verdict

7 items closed: 6 PASS + 1 FIX-APPLIED (CHK078) + 1 OBSERVATION-ONLY (CHK081, ROADMAP entry staleness — deferred per the ROADMAP's "working document" stance). Pass-3 caught one further cascade drift pass-2 missed, confirming the 003 artifact set's convergence trajectory matches the 001 precedent (each pass narrows the residue; pass-4 typically confirms zero findings).

**Pass-4 needed?** Likely no. Pass-3's substantive edit (CHK078) is a two-site cascade of an already-known fix shape (dep-count update). The convergence rule's 001 precedent (severity 9 → 6 → 5 → 0 across 4 passes) suggests pass-4 would confirm zero findings. But the user has chosen multi-pass walks in 003 so far; if they want a confirmation pass-4, it's cheap.

**Verdict**: 003 artifact set is consistent post-pass-3. `/speckit-tasks` is unblocked. CHK081 (ROADMAP §2 003 entry staleness) is recorded as a deferred polish item if the project decides to keep ROADMAP entries synchronised with landed feature artifacts.

---

## Pass-4 walk closure (2026-06-04) — zero-finding convergence

Triggered by the user's "do another pass in case you missed something again" — the fourth pass of the ROADMAP §4 convergence rule (which observed in 001's lifecycle that pass-4 typically confirms zero findings after pass-1 / pass-2 / pass-3 progressively narrow the residue). Pass-4 generated 6 items (CHK083–CHK088), all PASS verifications. **Zero substantive findings.**

### Final-sweep verifications

- [x] CHK083 - Did pass-3's CHK078 fix (proptest acknowledgment at plan.md Summary bullet + Constitution-Check Justified-dependencies bullet) close cleanly with no remaining "Three new dependencies" / "Three new direct dependencies" residues in plan.md? [Consistency, pass-3 verification] _verified: grep on plan.md finds no remaining "Three new" / "All three live" residues; both pass-3 fix sites stuck. The two remaining "three new direct dependencies" mentions in quickstart.md (lines 10 + 20) are contextually accurate per CHK080 — they describe the cargo-build dep tree which legitimately excludes the proptest [dev-dependency]._
- [x] CHK084 - Is the only remaining `Message::signed_bytes` mention in spec.md the Input field (preserved verbatim as historical record per CHK071)? [Consistency, Convergence] _verified: greps across pubsub-node/specs/, pubsub-node/docs/decisions/, pubsub-node/CLAUDE.md find zero matches outside the spec.md Input field and the checklist's own resolution notes. All active prose uses `PlainMessage::signed_bytes`._
- [x] CHK085 - Is the only remaining `002 FR-006` cite in 003 artifacts the correctly-used one (subscription-set mutator API in spec.md FR-019's "linearized order of subscription-set mutations" clause)? [Consistency, Convergence] _verified: spec.md FR-019 is the sole remaining cite, and it's contextually correct — 002 FR-006 IS the subscribe/unsubscribe mutator API. No other site cites it for snapshot-append (which is correctly 001 FR-006 — extended by 002 FR-004)._
- [x] CHK086 - Are the only remaining `Signature::placeholder` references the absence-documenting ones in research.md §4 ("the placeholder-signature dance is gone") and ADR 0010 Consequences ("no placeholder-signature workflow")? [Consistency, Convergence] _verified: those two remaining mentions are NEGATIVE references explicitly documenting the post-ADR-0010 absence of the constructor. No active API definition or workflow step references it._
- [x] CHK087 - Are all remaining mentions of "001-era `Envelope`" in 003 artifacts contextualized as "renamed to `RoutingFrame`" or in historical-migration prose? [Consistency, Convergence] _verified: greps find no orphaned "001-era Envelope" or `pub struct Envelope` references outside rename-context. All sites correctly describe the rename or carry historical migration framing._
- [x] CHK088 - Are all remaining `MessageHash::of(...)` references invoked with `&PlainMessage` (or `&signed.plain`, `&prev.plain`, `&self.plain`) — i.e., never with `&Message` or `&SignedMessage`? [Consistency, Convergence] _verified: every call site uses the content-anchored input shape per ADR 0010 + FR-011 + N-005. SignedMessage's anticipated future `message_hash(&self)` helper delegates to `MessageHash::of(&self.plain)` — also content-anchored._

### Pass-4 verdict — zero-finding closure

**Six items, all PASS, no FIX-APPLIED, no DEFER.** Pass-4 surfaces no residual cascade drift; the post-ADR-0010 restructure is fully consistent across spec.md, plan.md, research.md, data-model.md, contracts/library-api.md, quickstart.md, ADRs 0009 / 0010, IMPLEMENTATION_NOTES.md N-003 / N-004 / N-005, and CLAUDE.md.

### Convergence trajectory

The 003 walk matches the 001 precedent recorded in ROADMAP §4 (severity trajectory 9 → 6 → 5 → 0 over four passes; pass-4 is the confirmation closure):

| Pass | Substantive findings | Notes |
|---|---|---|
| Pass 1 | 9 | CHK023/043 cascade (10 sites), CHK022/046, CHK024/044, CHK025, CHK027, CHK047, CHK048, CHK054 (3 sites), CHK046 |
| Pass 2 | 3 + 1 minor | CHK067 (N-004 cascade), CHK068 (FR-019 cite typo), CHK069 (plan.md proptest in Technical Context), CHK075 (CLAUDE.md N-005) |
| Pass 3 | 1 | CHK078 (plan.md Summary + Constitution-Check Justified-dependencies bullets for proptest cascade) + CHK081 deferred observation (ROADMAP entry staleness) |
| Pass 4 | **0** | Zero-finding closure — pass-3's CHK078 fix appears to be the last drift site |

### Final verdict for `/speckit-tasks`

**003 artifact set is consistent and `/speckit-tasks`-ready as of pass-4.** The walk procedure has reached the convergence-rule's expected terminal state.

The one deferred observation (CHK081, ROADMAP §2 003 entry staleness) remains open as future polish; the ROADMAP's own §4 notes treat it as a "working document, not authoritative" so retroactive sync is optional. Not blocking.

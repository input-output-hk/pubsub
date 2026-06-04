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

- [ ] CHK017 - Is the `Message` reshape (from struct to `#[non_exhaustive]` enum) consistently described across spec.md FR-001, plan.md Summary + Technical Approach, data-model.md §16, contracts/library-api.md Message table, quickstart.md type tour, and ADR 0010? [Consistency, Spec §FR-001, ADR 0010]
- [ ] CHK018 - Is the `Message::Signed(SignedMessage)` variant described with identical field surfaces in spec.md FR-001, data-model.md §16a, and contracts/library-api.md (no drift in field names or types)? [Consistency, Spec §FR-001]
- [ ] CHK019 - Are `SignedMessage`'s two fields (`plain: PlainMessage`, `signature: Signature`) defined identically in data-model.md §16a and contracts/library-api.md, and consistent with spec.md FR-001's prose description? [Consistency, Spec §FR-001]
- [ ] CHK020 - Is `PlainMessage`'s field set (topic, publisher_id, parent_hash, sequence, timestamp, payload — signature excluded) defined identically in spec.md FR-001, data-model.md §16b, and contracts/library-api.md? [Consistency, Spec §FR-001]
- [ ] CHK021 - Is `MessagePayload`'s preservation (unchanged from 002, `#[non_exhaustive]`, sole `Ping(u64)` variant) consistently described in spec.md FR-001, data-model.md §16c, plan.md Project Structure, and contracts/library-api.md? [Consistency, Spec §FR-001]
- [ ] CHK022 - Is the rename of 001's `Envelope` routing wrapper to `RoutingFrame` consistently mentioned in spec.md FR-001 + Assumptions terminology bullet, plan.md Project Structure + Summary, data-model.md §16d, contracts/library-api.md re-exports + What-does-NOT-change, quickstart.md, and ADR 0010? [Consistency, ADR 0010]
- [ ] CHK023 - Is `PlainMessage::signed_bytes` (not `Message::signed_bytes`) the canonical-encoding seam consistently across spec.md FR-010, data-model.md §16b, contracts/library-api.md PlainMessage table, quickstart.md tour, and IMPLEMENTATION_NOTES.md N-004? [Consistency, Spec §FR-010]
- [ ] CHK024 - Is the no-placeholder signing workflow ("construct PlainMessage → compute signed_bytes → sign → assemble SignedMessage → wrap as Message::Signed") consistently described across spec.md FR-010, research.md §4, contracts/library-api.md PlainMessage table, and quickstart.md type tour? [Consistency, Spec §FR-010]
- [ ] CHK025 - Is `MessageHash::of(&PlainMessage)` (not `&SignedMessage` or `&Message`) consistently specified across spec.md FR-011, ADR 0010 Consequences, contracts/library-api.md MessageHash table, IMPLEMENTATION_NOTES.md N-005, and research.md §2? [Consistency, Spec §FR-011]
- [ ] CHK026 - Is the content-anchored hash rationale (signature-malleability immunity, Cardano `tx_hash = blake2b(body)` alignment, content-addressing) consistently summarized across spec.md FR-011 + Clarifications, ADR 0010 Consequences, and IMPLEMENTATION_NOTES.md N-005? [Consistency]
- [ ] CHK027 - Is the receive-task pattern-match (`match frame.message { Message::Signed(signed) => { /* filter → verify → snapshot */ } }`) consistently described in spec.md FR-013, FR-020, data-model.md §17, and contracts/library-api.md Receive-task pipeline section? [Consistency, Spec §FR-013, §FR-020]
- [ ] CHK028 - Is the topic-filter-first ordering (Q6) consistently described in spec.md FR-013, US3 acceptance scenarios, data-model.md §17, and contracts/library-api.md? [Consistency, Spec §FR-013]
- [ ] CHK029 - Is the test-support helper (`build_signed_message`, `build_signed_message_simple`) returning `Message` (the enum, post-ADR-0010) consistent between research.md §4 and quickstart.md / plan.md Project Structure references? [Consistency, research.md §4]
- [ ] CHK030 - Is the migration order (research.md §6's eight steps) reflected accurately in plan.md Project Structure's commit-grouping prose and consistent with the Constitution's green-checkpoints rule? [Consistency, research.md §6]

### Requirement Clarity & Measurability

- [ ] CHK031 - Is the precise byte layout of `PlainMessage::signed_bytes` unambiguously specified in FR-010 (field order, u32-BE length-prefixes, the 32-byte fixed-width `parent_hash` slot, the `MessageHash::ZERO` sentinel, the `MessagePayload` variant tag values, endianness of sequence and timestamp)? [Clarity, Spec §FR-010]
- [ ] CHK032 - Is `MessagePayload::Ping`'s variant tag value (`0x00`) explicitly fixed in FR-010 and referenced consistently in research.md §7's variant-tag-stability mechanism? [Clarity, Spec §FR-010]
- [ ] CHK033 - Are the `Display` formats for `PublicKey`, `Signature`, `MessageHash`, `PublisherId` unambiguously specified as full lowercase hex in FR-003 + Clarifications Q4, with `PrivateKey` explicitly excluded? [Clarity, Spec §FR-003]
- [ ] CHK034 - Is the `PrivateKey` discipline (no derived `Debug`, no `Display`, no `Hash`, hand-written redacting `Debug` impl) unambiguously specified in FR-003 + Clarifications Q3? [Clarity, Spec §FR-003]
- [ ] CHK035 - Is the property-based signature-binding test described in research.md §8 measurable and verifiable (specific invariant + clear input space)? [Measurability, research.md §8]
- [ ] CHK036 - Is "operator-visible at default log level" in SC-008 measurable as written, and clearly separated from the test-anchored portion (`received_messages()` absence assertion)? [Measurability, Spec §SC-008]
- [ ] CHK037 - Is the rustdoc-as-protocol-surface contract clear about which artifacts trigger same-commit rustdoc updates (FR-010 for `PlainMessage::signed_bytes`; SC-006 for the MOCK warning's four locations)? [Clarity, Spec §FR-010, SC-006]

### Coverage

- [ ] CHK038 - Does every 003 FR (FR-001 through FR-020) have at least one entity entry in data-model.md and at least one contract clause in contracts/library-api.md? [Coverage, data-model.md §19]
- [ ] CHK039 - Does every US1–US4 acceptance scenario map to ≥1 FR for its test-anchored assertion (received_messages() presence/absence)? [Coverage]
- [ ] CHK040 - Are the four scenario classes (valid+on-topic, valid+off-topic, invalid+on-topic, invalid+off-topic) all covered by US1+US3 acceptance scenarios after the Q6 ordering update? [Coverage, Spec §US1, US3]
- [ ] CHK041 - Are all Edge Cases in spec.md mapped to either an FR (covered) or an explicit deferral (N-001 / N-002 / N-003 / N-005)? [Coverage, Edge Cases]
- [ ] CHK042 - Is the future-Message-variant story (when 004 / 005 / 008 / 010 add ConnectionHello / PeerSample / etc.) described enough in ADR 0010 to scope the receive-task pattern-match extension, without prematurely introducing variants in 003? [Coverage, ADR 0010]

### Ambiguities & Conflicts (residual after Q1–Q6 + ADR 0010)

- [ ] CHK043 - After ADR 0010, does any artifact still reference `Message::signed_bytes` (old shape) instead of `PlainMessage::signed_bytes` (new shape)? [Ambiguity, Conflict]
- [ ] CHK044 - After ADR 0010, does any artifact still describe `Signature::placeholder()` or a "set the signature back" step in the signing workflow? [Ambiguity, Conflict]
- [ ] CHK045 - Does any artifact still describe `Message` as a struct (rather than an enum) after the ADR 0010 reshape? [Ambiguity, Conflict]
- [ ] CHK046 - Does any artifact still use the 001 `Envelope` name (rather than `RoutingFrame`) in a post-ADR-0010 context, including the `pubsub_node::network::Envelope` path? [Ambiguity, Conflict]
- [ ] CHK047 - Does any artifact's "publisher_id carrying random non-derived bytes" Edge Case still describe the publisher_id as a Message field (rather than a `PlainMessage` field)? [Consistency, Edge Cases]
- [ ] CHK048 - Are there any references to the term "envelope" in spec/plan/data-model/contracts/quickstart that mean the **PlainMessage** shape, contradicting the Assumptions terminology bullet's claim that prose-level "envelope" = whole signed message? [Ambiguity, Terminology]

### Traceability

- [ ] CHK049 - Is ADR 0009 cited from every FR that depends on its decisions (crypto trait shape, no-Signer-on-Node, mock construction — FR-002/003/004/005/006/007/008/009/012/018)? [Traceability, ADR 0009]
- [ ] CHK050 - Is ADR 0010 cited from every FR that depends on its decisions (Message enum, SignedMessage / PlainMessage split, RoutingFrame rename, MessageHash content-anchored — FR-001/010/011/013/018/020)? [Traceability, ADR 0010]
- [ ] CHK051 - Is N-005 cross-referenced from FR-011 (spec), ADR 0010 (Consequences), and quickstart.md (deferred-bits list)? [Traceability, N-005]
- [ ] CHK052 - Is N-003 cross-referenced from FR-016 (signature-only validation deferral) and Edge Cases (chain-integrity deferred bullets)? [Traceability, N-003]
- [ ] CHK053 - Is N-004 cross-referenced from FR-010 (canonical encoding migration trigger) and quickstart.md (deferred-bits list)? [Traceability, N-004]
- [ ] CHK054 - Are all 002-FR cross-references in 003 (e.g., 002 FR-004 cited in 003 FR-013; 002 FR-006 cited in 003 FR-013 snapshot append) accurate after the architectural restructure? [Traceability]

### Pre-/speckit-tasks Readiness

- [ ] CHK055 - Is the migration order in research.md §6 task-decomposable — each of the 8 steps shapes into a single coherent commit that leaves the crate green per the Constitution's green-checkpoints rule? [Readiness, research.md §6]
- [ ] CHK056 - Is the `src/crypto/mod.rs` + `src/crypto/mock.rs` introduction broken into orderable sub-steps in research.md §6 (types/traits first, mock impls second) so `/speckit-tasks` can derive separate test-first task pairs? [Readiness, plan.md Project Structure]
- [ ] CHK057 - Is the Message-enum reshape (research.md §6 step 4) clearly flagged as the single largest commit, with the rationale that the multi-file edit MUST be coherent (no partial migration)? [Readiness, research.md §6]
- [ ] CHK058 - Are the four new test files (signed_message.rs, multi_publisher.rs, filter_composition.rs, mock_crypto_repro.rs) identified with their per-US FR coverage in research.md or quickstart.md so each can become a Phase task? [Readiness]
- [ ] CHK059 - Is the property-based test (research.md §8) explicitly task-decomposable — separate from the example-driven acceptance-scenario tests — so the constitution's "property-based testing for critical properties" rule is task-trackable? [Readiness, Constitution Engineering Standards]
- [ ] CHK060 - Are the `Cargo.toml` dep additions (rand, rand_chacha, sha2 in `[dependencies]`; proptest in `[dev-dependencies]`) clearly task-shaped — a discrete dep-addition commit, separate from impl commits? [Readiness, research.md §5]
- [ ] CHK061 - Is the Constitution's TDD trigger (signature authenticity tests BEFORE the verification-step implementation per Principle II + the "envelope handling, message verification" carve-out) clearly translatable to task ordering in `/speckit-tasks`? [Readiness, Constitution Principle II]
- [ ] CHK062 - Is the 002 `topic_drop` → `message_dropped` + `cause = "topic_not_subscribed"` rename clearly identified as same-commit-as-invalid_signature-emitter per FR-015 + SC-007's atomicity criterion? [Readiness, Spec §FR-015, §SC-007]
- [ ] CHK063 - Is the test-support helper (`build_signed_message`, `build_signed_message_simple` in `tests/common/mod.rs` per research.md §4) clearly task-shaped — distinct from the user-story test tasks it serves? [Readiness, research.md §4]

---

**Walk procedure** (mirroring 002's pattern recorded in commits `8299c00` and `8afc2b5`):

1. Walk each item top to bottom; for each, decide PASS (✓ requirement is well-written) / FAIL (✗ requirement needs editing) / DEFER (⊘ raise later).
2. For each FAIL or DEFER, record the resolution inline as an `_resolved_` or `_deferred_` note with the commit / artifact / decision that closed it.
3. Substantive edits (cascade drifts found, missing cross-references, ambiguities surfaced) land as separate commits per the green-checkpoints rule.
4. If pass-1 surfaces ≥1 substantive edit, schedule a pass-2 walk to catch cascade drift from those edits (per ROADMAP §4's convergence rule).
5. When all items close (or are deferred with explicit triggers), proceed to `/speckit-tasks`.

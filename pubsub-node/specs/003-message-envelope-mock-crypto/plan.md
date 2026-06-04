# Implementation Plan: Message Envelope + Mock Crypto

**Branch**: `003-message-envelope-mock-crypto` | **Date**: 2026-06-03 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/003-message-envelope-mock-crypto/spec.md`

## Summary

Layer the staged-design-synthesis §2.3 envelope onto 002's topic-filter substrate, and add signature verification on the receive path with a mock-crypto factory that mirrors the asymmetric-shape of real asymmetric crypto. Concretely: `Message` reshapes from a `{ topic, payload }` struct into a `#[non_exhaustive]` enum whose sole 003-era variant is `Message::Signed(SignedMessage)` (per ADR 0010 — future protocol-message variants for 004 / 005 / 008 / 010 / deferred replication land here as sibling variants). `SignedMessage` carries `plain: PlainMessage` (the signed-over content — `topic`, `publisher_id`, `parent_hash`, `sequence`, `timestamp`, `payload`) plus a `signature: Signature`. `MessagePayload` is preserved unchanged from 002 (`#[non_exhaustive]`, sole variant `Ping(u64)`) and lives inside `PlainMessage`. The 001-era `pubsub_node::network::Envelope { from, message }` routing-wrapper struct is renamed to `RoutingFrame` in the same commit (frees the term "envelope" for prose-level use matching the synthesis §2.3, where envelope = the whole signed message). A new `pubsub_node::crypto` module introduces concrete byte-newtype types (`PublicKey`, `PrivateKey`, `Signature`, `MessageHash`, `Timestamp`, `VerifyError`) and the `Signer` / `Verifier` trait pair; a new `pubsub_node::crypto::mock` submodule houses `MockCryptoScheme` (seeded ChaCha20 RNG, `KeyPair { public, private }` factory, asymmetric-shaped mock via `_public` byte-suffix derivation), `TestSigner`, and `TestVerifier`. `Node` gains an `Arc<dyn Verifier>` field at construction; the receive task pattern-matches on the `Message` variant — for `Message::Signed(signed)`, runs the existing 002 topic-subscription filter on `signed.plain.topic` **first**, then invokes signature verification on subscribed messages by calling `verifier.verify(signed.plain.publisher_id.as_public_key(), &signed.plain.signed_bytes(), &signed.signature)`. Both filter drops emit `event = "message_dropped"` with a `cause` field (`topic_not_subscribed` / `invalid_signature`) — the 002-era `event = "topic_drop"` is renamed in the same commit as the new emitter lands. Signing is caller-side: `Node::send` continues to take an already-built `Message`; the binary's `main.rs` does not publish in 003.

Technical approach (informed by ADR 0009, ADR 0010, and the clarifications recorded in `spec.md ## Clarifications` Session 2026-06-03):

- **Protocol-message type hierarchy** (ADR 0010): `Message` is a `#[non_exhaustive]` enum; `Message::Signed(SignedMessage)` is the sole 003 variant. `SignedMessage = { plain: PlainMessage, signature: Signature }`. `PlainMessage` carries the §2.3 fields minus the signature. The split eliminates the placeholder-signature workflow (signed_bytes lives on `PlainMessage`, signature isn't in scope). Future protocol-message variants (004's connection-control, 005 / 010's peer-sampling, 008's registry-lookup, deferred replication) slot in as sibling Message variants without touching dissemination semantics. The 001 `Envelope` routing wrapper is renamed `RoutingFrame` in the same commit.
- **Concrete crypto types, no generics on `Node`**: per ADR 0009 the crypto trait pair operates on concrete `PublicKey` / `Signature` byte-newtypes (rejecting the associated-types pattern). `Node` stores `Arc<dyn Verifier>` and remains type-parameter-free. This preserves 001's "trait-at-construction, concrete-at-storage" pattern for the network layer.
- **Asymmetric-shaped mock**: `MockCryptoScheme::generate_keypair` produces 32 random bytes for `private`; `derive_public(&PrivateKey)` appends the fixed `b"_public"` suffix to produce the `PublicKey`. `TestSigner::sign(msg) = Signature(sha256(private || msg))`. `TestVerifier::verify(public, msg, sig)` strips the `_public` suffix to recover the private bytes, recomputes the SHA-256, and compares byte-for-byte. The mock is loudly documented as "not unforgeable" in rustdoc at four sites (module, scheme, signer, verifier) per SC-006.
- **Canonical signing bytes via a single seam**: `PlainMessage::signed_bytes(&self) -> Vec<u8>` is the only place the encoding lives, per FR-010 and IMPLEMENTATION_NOTES.md N-004. Hand-rolled length-prefixed concatenation, no leading version-tag byte (resolved by Q1 in clarify). The rustdoc on `signed_bytes` is normatively part of the protocol surface — any change to the byte layout requires a rustdoc update in the same commit.
- **Content-anchored chain hash**: per ADR 0010 + IMPLEMENTATION_NOTES.md N-005, `MessageHash::of` consumes `&PlainMessage` (signature excluded from the hash input). Signature-malleability immunity, Cardano `tx_hash = hash(body)` alignment, and stability across signing-scheme changes (011's Ed25519 swap; any later) all flow from this choice. Revisit trigger fires when a downstream feature first operationally consumes the hash (chain-integrity validation in 008 / 012; future caching / dedup).
- **Receive-task ordering**: per Q6 in clarify, the 002 topic filter runs **before** signature verification (avoids paying crypto cost on off-topic traffic, preserves 002's pipeline structure; operator-visible consequence — an off-topic + invalid-sig message surfaces as `cause = "topic_not_subscribed"`). The receive task pattern-matches on `Message::Signed(signed)` to reach the dissemination pipeline; future variants get their own match arms.
- **Project-wide drop-event convention**: `event = "message_dropped"` with a snake_case `cause` field is the cross-feature shape (saved memory `feedback_message_dropped_event_convention`). The 002 `topic_drop` rename lands in the same commit as the new `invalid_signature` emitter (FR-015 + SC-007 enforce atomicity); 002's tests that filter on the legacy event name migrate in lockstep.
- **Tests don't check log content**: the test-anchored contract is `received_messages()` (presence vs absence). Log-stream mentions in the user-story acceptance scenarios' Then clauses are descriptive operator UX, not test assertions (matches 002's FR-011 / FR-014 convention; explicitly re-affirmed in 003's clarify Session 2026-06-03).
- **`PrivateKey` is secret-discipline-shaped from day one**: no derived `Debug`, no `Display`, no `Hash` — only a hand-written `Debug` impl that redacts the bytes. This carries the discipline forward to feature 011's Ed25519 swap (which will add `Zeroize` and constant-time comparison) without changing the trait-level shape.
- **Three runtime + one test-only dependency**: `rand`, `rand_chacha`, `sha2` in `[dependencies]` (the binary's `main.rs` constructs a `TestVerifier` at the prototype-stage iteration per FR-017, making `crypto::mock` reachable at runtime); `proptest = "1"` in `[dev-dependencies]` for the signature-binding property-based test per research.md §8. None of the four warrants a fresh ADR — the existing ADR 0009 covers the trait-shape decision that motivates the runtime deps; proptest is the project's chosen property-testing framework, exempt under the Constitution's "Justified dependencies" rule; the deps themselves are ubiquitous standard-Rust crates.
- **Two ADRs cover 003's structural decisions**: ADR 0009 (`crypto-trait-shape`) was authored during pre-spec and locks the crypto trait pair shape + the mock construction + the no-Signer-on-Node decision. ADR 0010 (`protocol-message-type-hierarchy`) was authored post-`/speckit-plan` Phase 1 when the user surfaced that the original Message-as-struct shape conflated the §2.3 dissemination envelope with "every protocol message"; ADR 0010 restructures `Message` to a `#[non_exhaustive]` enum, splits `SignedMessage` into `PlainMessage + signature` (eliminating the placeholder-signature workflow), renames 001's `Envelope` routing wrapper to `RoutingFrame`, and locks the `MessageHash::of(&PlainMessage)` content-anchored hash choice (with revisit trigger in IMPLEMENTATION_NOTES.md N-005). The receive-task verification step, the canonical-encoding seam, and the 002 emitter rename are tactical extensions of decisions locked in those two ADRs.

## Technical Context

**Language/Version**: Rust 1.75+ stable (edition 2021) — unchanged from 001 / 002. No new toolchain requirement.

**Primary Dependencies**: 002's set (`tokio`, `serde` + `toml`, `tracing` + `tracing-subscriber`, `clap`, `thiserror`) carries through unchanged. Three runtime additions plus one test-only addition in 003:

- `rand = "0.8"` (`[dependencies]`) — the standard idiomatic randomness facade (used as `rand_chacha`'s entry point and for `OsRng` in `MockCryptoScheme::from_entropy`).
- `rand_chacha = "0.3"` (`[dependencies]`) — provides `ChaCha20Rng`, the seeded PRNG underlying `MockCryptoScheme::with_seed(seed)`.
- `sha2 = "0.10"` (`[dependencies]`) — RustCrypto family SHA-256; consumed by `MessageHash::of` and by `TestSigner` / `TestVerifier`.
- `proptest = "1"` (`[dev-dependencies]`) — property-based testing framework used to assert the signature-binding invariant per the Constitution's Engineering Standards rule (research.md §8). Test-only; not reachable from production code.

All four are widely-used, vetted crates. The runtime-dep additions are justified by FR-007 / FR-011 normatively; the proptest dev-dep is justified by the property-level claim that the Engineering Standards specifically call out. No version-pin ADR is needed — these are tactical version choices consistent with current ecosystem default versions; reviewable via `cargo tree`.

**Storage**: N/A — single-process, in-memory only. The arrival log lives in memory for the Node's lifetime (per 002's contract); 003 does not change persistence semantics. Key material (`PrivateKey`) is held in process memory by `TestSigner` for the test's duration; the binary's `main.rs` does not load or persist key material in 003.

**Testing**: `cargo test` with `#[tokio::test]` for async integration tests, unchanged from 002. The 002 await-on-delivery primitive in `tests/common/mod.rs::await_delivery` is the canonical test seam — receive-task processing is asynchronous, so any "subsequently observable" assertion (signature-pass appends, signature-fail does not append) uses `await_delivery` to avoid races. 003's new tests are `tests/signed_message.rs`, `tests/multi_publisher.rs`, `tests/filter_composition.rs`, and `tests/mock_crypto_repro.rs`; the 002 tests migrate to the new envelope shape via the test-support helper introduced in `tests/common/mod.rs` (research.md §4).

**Target Platform**: Linux + macOS developer workstations — unchanged from 001 / 002.

**Project Type**: Single Cargo crate, library + binary — unchanged from 001 / 002. 003 deltas extend `src/` rather than restructure it; the only new directory is `src/crypto/` (module + mock submodule).

**Performance Goals**: None at this stage. Spec SC-001's 30-second wall-clock budget for the US1 signed-message demonstration is trivially satisfied (TestSigner's SHA-256 hash is sub-microsecond per message). The receive-task ordering decision (topic filter before verification) reflects an architectural lean toward future-Ed25519-cost-savings, not a 003 performance target.

**Constraints**:

- Single-process scope (inherits from 001 / 002).
- Single-version, single-codebase signing and verifying (inherits from 001 / 002). Cross-process / cross-language consumers are deferred to feature 009.
- Network unchanged (002 FR-005 propagates) — signature verification is strictly receive-side, not transport-side.
- Snapshot semantics: `received_messages()` (001 FR-006) returns the new `Message` shape (extended in-place per 003 FR-001); the snapshot contract is otherwise unchanged.
- Linearizability across topic-filter check, signature-verification step, mutator API, and snapshot getter (003 FR-019 extends 002 FR-015 to cover verification). The verifier is stateless; concurrent inbound messages can be verified in parallel without contention.
- Logging facility: same `tracing` stack as 001 / 002; 003 introduces one new info-level event (`event = "message_dropped"` / `cause = "invalid_signature"`) and renames 002's `topic_drop` emitter to the same `message_dropped` shape with `cause = "topic_not_subscribed"`. The 002 mutator-log events (`topic_subscribed`, `topic_unsubscribed`) are unchanged.
- Mock crypto is **not unforgeable**. The `_public` suffix derivation is reversible by anyone with read access to the module source. This is a deliberate property documented loudly in rustdoc and Assumptions; real authenticity arrives in feature 011.

**Scale/Scope**: 2 ≤ N ≤ 10 nodes per demonstration (inherits 001's bound). 2 ≤ K ≤ 5 publisher keypairs per demonstration (US2 exercises 5 distinct publishers across at least 50 messages). At least 4 messages spanning the on-topic/off-topic × valid-sig/invalid-sig matrix per US3. At least 10 successive `generate_keypair` calls per SC-005's reproducibility check. Envelope-field cardinality is fixed at the shape in FR-001 — no per-payload variations.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Evaluated against `.specify/memory/constitution.md` v1.0.0.

### Initial gate (before Phase 0)

- **I. Correctness Over Optimization** — ✅ **pass**. Every plan claim traces to a numbered FR in `spec.md`, a Q&A in `## Clarifications`, ADR 0009, or an entry in `IMPLEMENTATION_NOTES.md` (N-003 / N-004). No optimization-led decisions; the receive-task ordering choice (Q6) is justified architecturally, not by a performance target. The hand-rolled signed-bytes encoding is the cheapest correct primitive; CBOR migration is deferred to feature 009 per N-004 with an explicit revisit trigger.
- **II. Test-Driven for Correctness Claims** — ⚠ **at-risk → mitigated**. 003 carries a **protocol-behavior claim** — signature authenticity. Per the constitution's "envelope handling, message verification" carve-out, strict TDD applies for 003. Mitigation: `/speckit-tasks` will schedule the signature-authenticity test tasks (US1 acceptance scenarios 1–4, US2 acceptance scenarios 1–3) **before** the receive-task implementation tasks; each test must fail against an unimplemented verifier, then the implementation makes them pass. Mock-crypto reproducibility tests (US4) follow the same pattern. Chain-integrity tests do **not** appear in 003 because the corresponding behavior does not appear in 003 (FR-016 + N-003); strict TDD on chain integrity is deferred to features 008 / 012 along with the behavior. The 002 `topic_drop` rename is a re-naming under the existing 002 test coverage — no new test required; the existing 002 tests get migrated in lockstep with the rename.
- **III. Document Structural Decisions as ADRs** — ✅ **pass**. Two structural decisions are captured as ADRs for 003: **ADR 0009** (`docs/decisions/0009-crypto-trait-shape.md`, authored pre-spec, committed as `529bce0`) covers the crypto trait shape — concrete `PublicKey` / `PrivateKey` / `Signature` newtypes, no associated types, mock-crypto factory shape, no-Signer-on-Node. **ADR 0010** (`docs/decisions/0010-protocol-message-type-hierarchy.md`, authored post-Phase-1 when the user surfaced the type-conflation concern) covers the protocol-message type hierarchy — `Message` as a `#[non_exhaustive]` enum, the `SignedMessage` / `PlainMessage` split, the `MessageHash::of(&PlainMessage)` content-anchored hash choice (with revisit trigger in IMPLEMENTATION_NOTES.md N-005), and the rename of 001's `Envelope` routing wrapper to `RoutingFrame`. No further structural decisions arise: the canonical-encoding seam, the receive-task verification step, the 002 emitter rename, and the test-support helper layout are all tactical extensions of decisions captured in those two ADRs. The drop-event convention (project-wide; codified in saved memory) is not an ADR target — it's a cross-feature operator-UX convention, not a 003-specific structural choice.
- **IV. Specifications as Ambiguity Detectors** — ✅ **pass**. Six clarifications surfaced during five `/speckit-clarify` rounds (Q1 version_tag → removed; Q2 PublisherId derives; Q3 PublicKey / PrivateKey / Signature derives; Q4 Display format; Q5 VerifyError non_exhaustive; Q6 receive-task ordering) are recorded as Q/A bullets in `spec.md` Clarifications Session 2026-06-03 and encoded as normative FR text. A round-5 audit confirmed no further spec-level issues. Plan-level items the spec deferred (crypto module file layout, `MessageHash::of` exact shape, `Node::new` parameter ordering, test-support helper, dep version pins, `MessagePayload` variant-tag stability mechanism, migration pattern for existing tests) are addressed in `research.md` with explicit Decision / Rationale / Alternatives. None are silently resolved in code.
- **V. Specifications Are Read-Only** — ✅ **pass**. This plan does not propose edits to `../formal_spec/`, `../docs/`, or `../docs/extensions/`. Files touched: `specs/003-message-envelope-mock-crypto/{plan.md, research.md, data-model.md, contracts/*.md, quickstart.md}` (agent-editable Spec-Kit artifacts), `CLAUDE.md` (agent context, not a protocol specification), no new ADR (0009 already lives in `docs/decisions/`). The spec itself was edited only during `/speckit-specify` and the five `/speckit-clarify` rounds, all explicit spec-authoring phases.

Engineering Standards specifically engaged:

- *Property-based testing for critical properties.* 003 carries property-level claims: **signature binding** (a signature produced by `TestSigner(K_priv)` over `m` is accepted by `TestVerifier::verify(K_pub, m, sig)` for the matching `K_pub` and rejected for any other key, any other message, or any modified signature). `/speckit-tasks` will schedule at least one property-based test using `proptest = "1"` (per research.md §8's framework decision) covering the signature-binding invariant. Specific cases (US1 acceptance scenarios 1–4) remain as example-driven tests for regression pins and acceptance-scenario traceability.
- *Observable state transitions.* The receive task emits one structured tracing event per drop (FR-014) and zero events per acceptance (silent success — matches 001 / 002 convention). The event carries the receiver's self_id, the forwarding peer's from, the envelope's topic, and (for invalid_signature) the envelope's publisher_id — sufficient to reconstruct "which message, which peer, which decision, which outcome" per Engineering Standards.
- *Justified dependencies.* Four new direct dependencies: `rand`, `rand_chacha`, `sha2` in `[dependencies]` (runtime); `proptest` in `[dev-dependencies]` (test-only). FR-007 and FR-011 normatively require the runtime trio's functionality; ADR 0009 covers the structural choice (concrete byte-newtype types + asymmetric-shaped mock). proptest is justified by the Constitution's Engineering-Standards rule on property-based testing for critical properties (signature binding qualifies, per research.md §8). Per the Constitution's exemption clause ("standard language toolchain components and the project's chosen test framework are exempt"), `rand` family + `sha2` qualify as standard-Rust crypto-ecosystem crates, and `proptest` qualifies as the project's chosen property-testing framework. No additional ADR slot required.
- *Reproducible tests and simulations.* 003 introduces a seeded PRNG (`MockCryptoScheme::with_seed([u8; 32])`) — every test that asserts against key bytes pins a seed (US4 acceptance scenarios 1–5 cover this explicitly). No wall-clock dependencies are introduced. `Timestamp::now()` exists for production use, but acceptance scenarios all use `Timestamp::from_millis(known_value)` so test runs are deterministic.

Development Workflow specifically engaged:

- *Green checkpoints* / *Logical increments.* `/speckit-tasks` will order tasks so every closure leaves the crate compiling and `cargo test` green. Concrete ordering plan: introduce the `crypto::PublicKey` / `PrivateKey` / `Signature` newtypes (no behavior yet), then `crypto::Signer` / `Verifier` traits (no impls yet — the build still compiles because no code holds an `Arc<dyn Verifier>` yet), then `crypto::mock` impls (now the traits have callers), then `PlainMessage::signed_bytes` + the envelope-shape migration (tests still pass because verification isn't wired in), then the test-support helper in `tests/common/mod.rs`, then the receive-task verification step + the 002 `topic_drop` rename (now the new tests pass), then the per-user-story tests, then ADR 0009 verification / quickstart drift sweep.
- *Tracked skips.* No skipped or ignored tests are introduced by 003. If `/speckit-implement` discovers a test that can't be authored in TDD-first order (because, e.g., the test-support helper isn't yet in shape), it carries a tracking-issue or ADR reference per the constitution.

### Post-Phase-1 gate (re-evaluated after research.md, data-model.md, contracts/, quickstart.md)

All Phase 1 artifacts exist (`research.md`, `data-model.md`, `contracts/library-api.md`, `quickstart.md`; `contracts/node-config.toml.md` is intentionally omitted because the TOML schema is unchanged in 003 per FR-017 — 002's contract remains canonical). Re-running the gate against concrete content:

- **I. Correctness Over Optimization** — ✅ **pass**. Every entity in `data-model.md` and every contract clause in `contracts/library-api.md` traces to a numbered FR (matrix in `data-model.md §last`). No optimization-led decisions appear in the artifacts.
- **II. Test-Driven for Correctness Claims** — ✅ **pass** (mitigation applied). `quickstart.md` walks through running the new tests (US1 / US2 / US3 / US4) before the production-binary demonstration; `/speckit-tasks` will schedule those test tasks before implementation tasks per the constitution's protocol-behavior carve-out.
- **III. Document Structural Decisions as ADRs** — ✅ **pass**. Two ADRs cover 003: ADR 0009 (`crypto-trait-shape`, pre-spec) and ADR 0010 (`protocol-message-type-hierarchy`, post-Phase-1). Both exist under `docs/decisions/`. The Phase 1 artifacts surface no further structural choice — every plan-level decision in `research.md` is tactical (file layout, helper-function shape, dep version, etc.). The hash-input decision (`MessageHash::of(&PlainMessage)` vs `&SignedMessage`) surfaced during ADR 0010's drafting and is captured both in ADR 0010's Consequences section and as a revisit-trigger entry in IMPLEMENTATION_NOTES.md N-005.
- **IV. Specifications as Ambiguity Detectors** — ✅ **pass**. Every plan-level item that emerged during planning is recorded in `research.md` with Decision / Rationale / Alternatives. The "Open follow-ups" section records v2+ items that would otherwise be silently rediscovered.
- **V. Specifications Are Read-Only** — ✅ **pass**. Files touched by this plan run: `specs/003-message-envelope-mock-crypto/{plan.md, research.md, data-model.md, contracts/library-api.md, quickstart.md}` (agent-editable Spec-Kit artifacts), `CLAUDE.md` (agent context — the SPECKIT block was updated to point at 003's plan). No edits to `../formal_spec/`, `../docs/`, or `../docs/extensions/`.

**Gate verdict**: all five principles ✅ pass, no entries in Complexity Tracking. Plan is cleared for `/speckit-tasks`.

## Project Structure

### Documentation (this feature)

```text
specs/003-message-envelope-mock-crypto/
├── plan.md              # This file (/speckit-plan output)
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   └── library-api.md   # 003 deltas to the Rust public surface
├── checklists/
│   └── requirements.md  # Auto-generated by /speckit-specify
├── spec.md              # Feature spec (input)
└── tasks.md             # Phase 2 output (/speckit-tasks — NOT created here)
```

001's `contracts/cli.md` and 002's `contracts/node-config.toml.md` are **inherited unchanged** by 003 — no new CLI flags, no behavior change on existing flags, no new TOML field (FR-017). 003's `contracts/` therefore omits both `cli.md` and a `node-config.toml.md` companion; the 001 / 002 files remain canonical.

### Source Code (crate root: `pubsub-node/`)

```text
pubsub-node/
├── Cargo.toml                # extended: + rand, rand_chacha, sha2 in [dependencies]; + proptest in [dev-dependencies]
├── src/
│   ├── lib.rs                # re-exports extended: + crypto::{PublicKey, PrivateKey, Signature, MessageHash, Timestamp, VerifyError, Signer, Verifier},
│   │                         #                       + crypto::mock::{MockCryptoScheme, KeyPair, TestSigner, TestVerifier, derive_public},
│   │                         #                       + message::{Message, SignedMessage, PlainMessage, MessagePayload, PublisherId}.
│   │                         # Message is now a re-export of the enum (per ADR 0010), not the previous struct shape.
│   ├── peer.rs               # unchanged
│   ├── topic.rs              # unchanged
│   ├── network.rs            # renamed 001-era Envelope { from, message } to RoutingFrame { from, message } per ADR 0010;
│   │                         # behavior unchanged (FR-005 of 002 still holds; signature verification is receive-side,
│   │                         # not network-side). The rename is one struct + ~one grep-and-replace pass across callers.
│   ├── node.rs               # extended: + verifier: Arc<dyn Verifier> field; recv_task pattern-matches on the incoming
│   │                         # Message variant — Message::Signed(signed) arm runs the topic filter BEFORE signature
│   │                         # verification (FR-013, per Q6); topic_drop emitter renamed to event = "message_dropped"
│   │                         # / cause = "topic_not_subscribed" (FR-015) in the same commit as the new invalid_signature
│   │                         # emitter lands.
│   ├── message.rs            # extended: defines Message (#[non_exhaustive] enum), SignedMessage (struct), PlainMessage
│   │                         # (struct), PublisherId (newtype). MessagePayload (the 002 enum) is preserved unchanged and
│   │                         # lives as a field of PlainMessage. PlainMessage::signed_bytes(&self) -> Vec<u8> is the
│   │                         # canonical-encoding seam; MessageHash::of(&PlainMessage) -> MessageHash is content-anchored
│   │                         # (no signature in the hash input per ADR 0010 + N-005).
│   ├── received.rs           # unchanged (ReceivedDelivery wraps the new Message enum shape transitively)
│   ├── config.rs             # unchanged (FR-017 — TOML schema unchanged)
│   ├── error.rs              # unchanged (VerifyError lives in crypto::, not in the existing error module)
│   ├── crypto/               # NEW directory
│   │   ├── mod.rs            # NEW: PublicKey, PrivateKey, Signature, MessageHash, Timestamp newtype declarations;
│   │   │                     # VerifyError #[non_exhaustive] enum; Signer / Verifier trait declarations
│   │   └── mock.rs           # NEW: PUBLIC_SUFFIX constant; derive_public helper; MockCryptoScheme with seeded
│   │                         # ChaCha20Rng + KeyPair value type + generate_keypair / signer / verifier methods;
│   │                         # TestSigner / TestVerifier impls; module-level + per-struct "MOCK — not unforgeable"
│   │                         # rustdoc warnings (SC-006)
│   └── main.rs               # extended: constructs Arc::new(TestVerifier) (or equivalent via MockCryptoScheme)
│                             # and passes it to Node::new
├── tests/
│   ├── two_node_ping.rs      # MIGRATED: envelope shape (test-support helper builds signed messages)
│   ├── n_node_graph.rs       # MIGRATED: envelope shape; existing topic-filter assertions carry forward
│   ├── topic_filter.rs       # MIGRATED: log-event filter switches from "topic_drop" to ("message_dropped", "topic_not_subscribed")
│   ├── topic_runtime.rs      # MIGRATED: envelope shape; existing dynamic-subscription assertions carry forward
│   ├── config_loading.rs     # MIGRATED: envelope shape if any test constructs a Message; otherwise minimal touch
│   ├── signed_message.rs     # NEW: US1 acceptance scenarios (signature verification on receive path; 4 scenarios)
│   ├── multi_publisher.rs    # NEW: US2 acceptance scenarios (multi-publisher dispatching; 3 scenarios)
│   ├── filter_composition.rs # NEW: US3 acceptance scenarios (topic + signature filter composition; 5 scenarios)
│   ├── mock_crypto_repro.rs  # NEW: US4 acceptance scenarios (seeded reproducibility, derive_public invariant; 5 scenarios)
│   └── common/
│       └── mod.rs            # extended: + build_signed_message(&signer, topic, payload, …) helper that fills
│                             # in sensible defaults (sequence=0, parent_hash=None, timestamp=fixed) for tests
│                             # that don't care about chain semantics; existing fixture builders updated to take
│                             # a verifier parameter
├── docs/
│   └── decisions/
│       ├── 0001-…             # 001 ADRs (unchanged)
│       ├── …
│       ├── 0008-…             # 002 ADR (unchanged)
│       ├── 0009-crypto-trait-shape.md   # ALREADY AUTHORED (pre-spec, commit 529bce0); not re-created
│       └── 0010-protocol-message-type-hierarchy.md  # NEW (post-Phase-1 design discussion): Message enum,
│                                                    # SignedMessage / PlainMessage split, 001 Envelope rename to
│                                                    # RoutingFrame, MessageHash::of(&PlainMessage) content-anchored hash
└── specs/                    # this directory
```

**Structure Decision**: extend 001 / 002's single-Cargo-crate layout. The new `src/crypto/` directory introduces a clear module boundary for crypto types and impls; the mock submodule (`crypto::mock`) is `pub` at the crate boundary so test code and the binary's main can both reach it at this prototype-stage iteration. `src/network.rs` gains a one-struct rename (`Envelope` → `RoutingFrame`) per ADR 0010 but otherwise keeps 002's FR-005 ("network unchanged") behavior — 003 still layers verification on top of 002's filter at the receive task, not at the substrate. `src/message.rs` is the most extended file in 003: it gains the `Message` enum, `SignedMessage`, `PlainMessage`, `PublisherId`, and the `PlainMessage::signed_bytes` seam, while preserving `MessagePayload` from 002 unchanged. Existing 002 tests are migrated to the new layered type shape via a single helper added to `tests/common/mod.rs` (research.md §4); fresh tests for the four 003 user stories are added as new files. Two ADRs cover 003's structural decisions: ADR 0009 (crypto trait shape) authored during pre-spec, and ADR 0010 (protocol-message type hierarchy) authored post-Phase-1 when the type-shape concern surfaced. The receive-task verification step + the 002 emitter rename + the test-support helper are tactical extensions of decisions locked in those two ADRs.

## Complexity Tracking

*No Constitution Check violations require justification.*

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|--------------------------------------|
| *(none)* | — | — |

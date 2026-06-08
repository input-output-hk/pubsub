# Research: Message Envelope + Mock Crypto

**Feature**: 003-message-envelope-mock-crypto

**Created**: 2026-06-03

**Purpose**: capture plan-level design choices that the spec deferred or that emerged during planning, with Decision / Rationale / Alternatives entries for each. The spec resolved all `NEEDS CLARIFICATION` markers during the five `/speckit-clarify` rounds (Q1–Q6) recorded in `spec.md ## Clarifications` Session 2026-06-03; this file resolves the remaining plan-level questions and pins the shape of artifacts that downstream phases (`/speckit-tasks`, `/speckit-implement`) will assume.

---

## §1. Crypto module file layout: nested directory vs flat files

**Decision**: nested directory — `src/crypto/mod.rs` for the trait pair + concrete byte-newtype types + `VerifyError`; `src/crypto/mock.rs` for `MockCryptoScheme`, `KeyPair`, `TestSigner`, `TestVerifier`, and the `PUBLIC_SUFFIX` constant. `crypto::mock` is `pub` at the crate boundary so both test code and the binary's `main.rs` can construct a `TestVerifier` at this prototype-stage iteration.

**Rationale**:

- 001 / 002 have a flat `src/` layout (`peer.rs`, `topic.rs`, `network.rs`, `message.rs`, `node.rs`, `config.rs`, `error.rs`, `received.rs`). 003 introduces enough new public-API surface (5 newtype types, 2 traits, 1 error enum, 1 factory struct, 1 value-type struct, 2 impl structs, 1 constant) to make a flat `crypto.rs` file long and awkward to navigate. A nested module gives a clear two-file split: production-shape types and traits in `mod.rs`; mock-specific machinery in `mock.rs`.
- The future feature 011 (`Ed25519Signer` / `Ed25519Verifier`) will add `src/crypto/ed25519.rs` as a sibling to `mock.rs`, slotting in under the same `crypto::` namespace without restructuring. The directory makes this future addition mechanical.
- Keeping the mock in its own file makes the rustdoc-level "MOCK — not unforgeable" warning loud — the entire file's documentation can carry the warning at the top, and any future contributor opening `crypto/mock.rs` sees it before reading any code.

**Alternatives considered**:

- **Flat: `src/crypto.rs` + `src/crypto_mock.rs`**. Rejected: the flat layout doesn't compose as cleanly when feature 011 adds a third file (`crypto_ed25519.rs`?), and the `crypto::mock` path is more idiomatic Rust for a submodule.
- **Single file: `src/crypto.rs` containing everything**. Rejected: ~300 lines of trait declarations, type declarations, mock machinery, and rustdoc warnings in one file is harder to navigate than a two-file split; the mock-vs-real boundary is also less obvious.
- **`pubsub_node::crypto::test`** instead of `pubsub_node::crypto::mock`. Considered: `test` reads more accurately ("this is what tests use"), but `mock` matches the rustdoc convention used by the rest of the Rust ecosystem (`mockall`, `mockito`) and signals "fake implementation that mimics the shape of the real thing" more directly. Going with `mock`.
- **`#[cfg(any(test, feature = "mock-crypto"))]` gating**. Rejected for 003: FR-017 says the binary's `main.rs` uses `TestVerifier` at this prototype-stage iteration, which requires the mock to be reachable in non-test builds. The feature-gate could be added in 011 when `Ed25519CryptoScheme` becomes the default; until then it's premature.

## §2. `MessageHash::of` API shape and input type

**Decision**: associated function `MessageHash::of(plain: &PlainMessage) -> MessageHash`. Internally computes `MessageHash(sha256(plain.signed_bytes()).into())`. The input is `&PlainMessage`, not `&SignedMessage` and not `&Message` — see the input-type discussion below.

**Rationale (function placement)**:

- Associated function reads naturally at the call site: `let parent = MessageHash::of(&prev.plain);` — symmetric with constructor patterns like `MessageHash::ZERO`.
- The `MessageHash` type already owns the responsibility of being "the hash of a message", so the constructor function lives on the hash type rather than on the data type. The alternative — `PlainMessage::hash(&self) -> MessageHash` — bleeds hash semantics into the message-content type and would force `message.rs` to depend on `sha2` directly.
- Adding the function as an inherent impl avoids any trait-object overhead and keeps the call site cheap (one sha256 invocation, no trait dispatch).

**Rationale (input type — `&PlainMessage`, not `&SignedMessage`)**: surfaced during ADR 0010's drafting; the choice is structural, not stylistic. Hashing `PlainMessage` (content only) rather than `SignedMessage` (content plus signature) immunizes the parent-hash chain against signature malleability (the canonical Bitcoin pre-SegWit lesson), aligns with the Cardano `tx_hash = blake2b(tx_body)` convention, keeps the chain stable across signing-scheme changes (feature 011's Ed25519 swap; any later scheme migration), and makes `MessageHash` a clean content-addressing primitive. Full reasoning in ADR 0010's Consequences section and in spec.md FR-011; revisit trigger recorded in `specs/IMPLEMENTATION_NOTES.md` N-005 for when downstream features start operationally consuming the hash.

**Alternatives considered**:

- **`PlainMessage::hash(&self) -> MessageHash` (method on `PlainMessage`)**. Rejected: ties `PlainMessage` to the hash algorithm; would need updating if the hash function ever changes (it won't in 003 — assumption locked — but the hash function's home is more naturally on the hash type).
- **`MessageHash::of(&SignedMessage)` (hash includes signature)**. Rejected. Full reasoning in ADR 0010 — signature-malleability concerns, Cardano alignment, content-addressing cleanness, cross-scheme stability.
- **Free function `pubsub_node::crypto::message_hash(plain: &PlainMessage) -> MessageHash`**. Rejected: an associated function on `MessageHash` is more discoverable via IDE autocomplete than a sibling free function; also keeps the `crypto` module's surface tighter.
- **`From<&PlainMessage> for MessageHash`**. Considered: idiomatic Rust, but `From` implementations are often instinctively read as "cheap conversion" rather than "compute a hash"; the explicit `MessageHash::of` name flags the cost (one SHA-256) more honestly. Skipping.

## §3. `Node::new` parameter ordering with the new `verifier` arg

**Decision**: append `verifier: Arc<dyn Verifier>` at the end of the existing parameter list:

```rust
pub async fn new<N: Network>(
    self_id: PeerId,
    config: NodeConfig,
    initial_subscriptions: HashSet<TopicId>,
    network: Arc<N>,
    verifier: Arc<dyn Verifier>,
) -> Result<Self, NodeError>
```

**Rationale**:

- "Append at end" is the universal convention for adding required parameters to a constructor in Rust — minimises ambiguity at call sites that read positionally.
- Existing 001 / 002 call sites need updating (every test fixture builder + `main.rs`); the append-at-end choice keeps the diff small at each call site (one new argument at the tail).
- The argument is `Arc<dyn Verifier>` (trait object) rather than `Arc<TestVerifier>` (concrete) so the call site can pin the verifier impl explicitly; this matches the ADR 0009 decision that `Node` itself stays type-parameter-free.

**Alternatives considered**:

- **Insert `verifier` between `config` and `initial_subscriptions`** (grouping verifier with other "behavioral inputs" like the subscription set). Rejected: the spec's clarifications and the 002 lifecycle both established `initial_subscriptions` as the third positional argument; reordering would force every existing test to update its argument list non-mechanically. Append-at-end is mechanical.
- **Builder pattern (`Node::builder().with_verifier(...).build().await?`)**. Rejected: would be a structural change to `Node::new`'s public API surface and is overkill for adding one required argument. If a future feature accumulates 5+ optional parameters, the builder shift becomes worth it — not now.
- **Optional via `Option<Arc<dyn Verifier>>`** with a default `TestVerifier`. Rejected by FR-012: "The parameter is required (not optional): a Node MUST be constructed with a non-None verifier, since unsigned messages have no place in the 003 envelope shape."

## §4. Test-support helper for building signed envelopes

**Decision**: add a small helper to `tests/common/mod.rs` that returns a fully-wrapped `Message`:

```rust
pub fn build_signed_message(
    signer: &impl Signer,
    topic: TopicId,
    payload: MessagePayload,
    sequence: u64,
    parent_hash: Option<MessageHash>,
    timestamp: Timestamp,
) -> Message {
    let plain = PlainMessage {
        topic,
        publisher_id: PublisherId::from(signer.public_key()),
        parent_hash,
        sequence,
        timestamp,
        payload,
    };
    let signature = signer.sign(&plain.signed_bytes());
    Message::Signed(SignedMessage { plain, signature })
}
```

Plus a convenience wrapper for tests that don't care about sequence / parent_hash / timestamp specifics:

```rust
pub fn build_signed_message_simple(
    signer: &impl Signer,
    topic: TopicId,
    payload: MessagePayload,
) -> Message {
    build_signed_message(signer, topic, payload, 0, None, Timestamp::from_millis(0))
}
```

Tests that need to manipulate the `SignedMessage` or `PlainMessage` directly (e.g., to swap the `publisher_id` after signing for US1 AS-4, or to construct a malformed-public-key edge case) skip the helper and assemble the layered types by hand.

**Rationale**:

- US1 / US2 / US3 acceptance scenarios each describe a multi-step "construct PlainMessage → compute signed_bytes → sign → assemble SignedMessage → wrap in Message::Signed" workflow per FR-010 (post-ADR 0010). Without the helper, every test would repeat that pattern. With the helper, each test reads as "build a signed message with these inputs, then send it" — much closer to the natural-language scenario.
- **The placeholder-signature dance is gone.** Because `signed_bytes` lives on `PlainMessage` (post-ADR 0010), the signature isn't a field in scope when the bytes are computed; there is no `Signature::placeholder()` constructor needed and no "set the signature back into the message" step. The helper reads cleanly top-to-bottom.
- The `_simple` variant absorbs the test fixtures that exist for migration (`tests/two_node_ping.rs`, `tests/n_node_graph.rs`, etc.) — those tests don't care about chain semantics; they need a Message that verifies cleanly under whatever shared TestVerifier the test suite uses.
- Keeping the helper in `tests/common/mod.rs` (not `src/`) avoids polluting the production-surface API. Tests already share `common::` via the standard Rust integration-test pattern.

**Alternatives considered**:

- **Add a `MessageBuilder` type in `src/message.rs`**. Rejected for 003: a builder is overkill for the prototype scale; the helper function is 15 lines and lives entirely in test-support code where it belongs.
- **Force each test to hand-construct the envelope manually**. Rejected: ~20 acceptance scenarios across US1–US4, plus the migration of existing 002 tests; multiplying the per-test boilerplate by ~30 sites produces unreadable test code and makes future schema changes much more painful.
- **Add the helper as a `pub` test-utility on `MockCryptoScheme`** (e.g. `scheme.sign_message(topic, payload, ...) -> Message`). Considered: ergonomic but couples the helper to a specific scheme, which is awkward for tests that want to use a stand-alone `TestSigner` from a `KeyPair` they've already split. Free-function-in-`tests/common/` stays neutral.

## §5. Cargo.toml dep version pins and placement

**Decision**:

```toml
[dependencies]
rand = "0.8"
rand_chacha = "0.3"
sha2 = "0.10"
```

All three in `[dependencies]` (not `[dev-dependencies]`). Versions pinned to current major.minor as of 2026-06; patch ranges left wildcarded per existing 001 / 002 convention.

**Rationale**:

- FR-017 says the binary's `main.rs` constructs a `TestVerifier` (via `MockCryptoScheme`) at this prototype-stage iteration. `MockCryptoScheme::with_seed` uses `rand_chacha::ChaCha20Rng`; `from_entropy` uses `rand::rngs::OsRng` via the `rand` facade; `TestSigner` / `TestVerifier` use `sha2::Sha256`. All three crates are therefore reachable from the production binary — they cannot live in `[dev-dependencies]`.
- Version pins match the current stable releases of the RustCrypto and rand-rs ecosystems. None of the three carries a 1.0 release commitment, but all are widely used, well-tested, and security-vetted; the major-minor pin is enough.
- No existing 001 / 002 dependency is bumped or removed; the dependency tree grows additively.

**Alternatives considered**:

- **Pin to exact patch versions** (e.g., `rand = "=0.8.5"`). Rejected: 001 / 002 use loose patch ranges; consistency wins. Pinning patches creates dependabot churn without proportional safety.
- **Use `getrandom` directly instead of `rand` + `rand_chacha`**. Rejected: the standard idiomatic Rust pattern is `rand::SeedableRng::from_entropy()` via `rand` + `rand_chacha`; dropping `rand` would force re-implementing the trait surface and lose ecosystem familiarity for new contributors.
- **Use `blake3` instead of `sha2`**. Considered: blake3 is faster than SHA-256 and has a cleaner API. Rejected: FR-011 explicitly names SHA-256, which matches the staged-design-synthesis §2.3 hash convention and the Cardano ecosystem precedent. Future hash-function changes are out of scope per the spec's Assumptions.
- **Roll a hand-written SHA-256**. Rejected at the question level during pre-spec discussion (the user explicitly asked for "the standard Rust option").

## §6. Migration strategy for existing 001 / 002 tests

**Decision**: migrate every 001 / 002 test that constructs a `Message` (directly or via `tests/common/mod.rs::build_ping`) to use the new `build_signed_message_simple` helper from §4 above. The migration is a single search-and-replace per construction site, performed in lockstep with the receive-task verification step (so the build stays green at every commit).

Migration order (encoded in `/speckit-tasks`):

1. Introduce the new `crypto` module (types + traits, no impls). Build green; no behavior change.
2. Introduce `crypto::mock` impls. Build green; no behavior change (no callers yet).
3. Rename 001's `Envelope` routing wrapper to `RoutingFrame` per ADR 0010. Single struct + one grep-and-replace pass across the network layer and any test that pattern-matches on the type name. Build green; no behavior change.
4. Reshape `Message` per ADR 0010: convert from struct to `#[non_exhaustive]` enum with the sole `Signed(SignedMessage)` variant; introduce `SignedMessage` and `PlainMessage` structs; preserve `MessagePayload` from 002 unchanged. Build green only if every construction site is updated *in the same commit* — this is the largest single-commit migration in 003, replacing `Message { topic, payload: MessagePayload::Ping(n) }` constructions with `Message::Signed(SignedMessage { plain: PlainMessage { topic, publisher_id, parent_hash: None, sequence: 0, timestamp: Timestamp::from_millis(0), payload: MessagePayload::Ping(n) }, signature })`. Initially every test fixture uses a shared `TestSigner` so signatures verify cleanly.
5. Add the test-support helper (`build_signed_message`, `build_signed_message_simple`) to `tests/common/mod.rs`. The helper absorbs the multi-layer construction from step 4 so the 002-era tests can compress back to one-line constructions.
6. Add a shared `TestVerifier` to the test fixtures so existing 002 tests have a default verifier without restructuring.
7. Add the receive-task pattern-match wrapper + verification step + the `topic_drop` rename to `src/node.rs`. Tests still pass because the new `TestVerifier` accepts every message built via `build_signed_message_simple`.
8. Add the per-user-story tests for 003 (US1, US2, US3, US4).

**Rationale**: each step leaves the crate green per the Constitution's "green checkpoints" rule. The Message-shape migration in step 3 is the only commit that requires a coordinated multi-file edit; the helper from step 4 absorbs all the existing call sites without requiring per-test reasoning about envelope fields.

**Alternatives considered**:

- **Migrate tests incrementally, with `#[ignore]` markers for the unmigrated ones**. Rejected: contradicts the Constitution's "Tracked skips" rule (no `#[ignore]` without a tracking issue) and the "Green checkpoints" rule (the unmigrated tests would fail rather than skip). Single-commit migration is simpler.
- **Add a `Message::ping_legacy(topic, n)` wrapper that builds a Message with a default-zero signature and accept-everything-mode TestVerifier**. Rejected: dual-mode verification (sometimes-strict, sometimes-permissive) violates FR-013 and creates two divergent code paths; the helper-function approach is cleaner.

## §7. `MessagePayload` variant-tag stability mechanism

**Decision**: explicit `match` in `PlainMessage::signed_bytes` that hand-writes each variant's tag:

```rust
match &self.payload {
    MessagePayload::Ping(n) => {
        out.push(0x00); // explicit Ping tag
        out.extend_from_slice(&n.to_be_bytes());
    }
}
```

NOT `(variant as u8)` or any pattern that depends on Rust's declaration-order tagging.

**Rationale**:

- `MessagePayload` is `#[non_exhaustive]` (inherited from 002). Adding a variant in a future feature is non-breaking *at the type level*, but if `signed_bytes` derived the tag from declaration order (`as u8`), reordering variants alphabetically would silently change the byte encoding and invalidate every existing signature.
- Explicit `match` arms with hard-coded tag bytes make the tag-to-variant mapping a normative property of the codebase. A test (added in `/speckit-tasks`) pins each tag value: `assert_eq!(signed_bytes_of_ping(0).first(), Some(&0x00))`.
- FR-010 says "future variants will append new tag values without altering the existing tags." The explicit-match pattern is the mechanism that enforces this.

**Alternatives considered**:

- **`#[repr(u8)]` on `MessagePayload` with explicit discriminants** (e.g., `Ping(u64) = 0x00`). Considered: would also enforce tag stability via the type system. Rejected because Rust requires `repr` enums to be fieldless or fully-explicitly-tagged, which limits the variant-shape flexibility (a future variant carrying a struct couldn't use `#[repr]` cleanly).
- **A `#[derive(SignedTag)]` macro**. Rejected: overkill for a single enum at v1 scale; introduces a build-time dependency without proportional benefit.
- **`variant as u8` cast**. Rejected per the rationale above.

## §8. Property-based test framework choice

**Decision**: `proptest = "1"` added under `[dev-dependencies]`. Used to author at least one property-level test asserting **signature binding**: for any `(seed, msg)` pair, the verifier accepts the signature produced by the matching signer on `msg`, and rejects any modified `(key, msg, sig)` triple.

**Rationale**:

- The Engineering Standards section of the constitution names "signature binding" as a property-level claim that prefers property-based tests over single-case unit tests. `proptest` is the idiomatic Rust answer (used by the Cardano-rust ecosystem, ed25519_dalek's own test suite, and the RustCrypto family).
- The single test file `tests/mock_crypto_repro.rs` (US4) is a natural home for the property-based test alongside the seed-determinism scenarios.
- Adding `proptest` as a `[dev-dependencies]` entry has no production-side cost; the crate is widely used and well-vetted.

**Alternatives considered**:

- **`quickcheck`**. Rejected: less actively maintained than `proptest`, smaller user base, less ergonomic shrinking on failure.
- **Hand-rolled property tests** (build N random `(key, msg)` pairs in a loop). Rejected: loses the automatic shrinking and reproducibility-from-seed benefits of `proptest`; also makes failures harder to investigate.
- **No property-based tests in 003**. Rejected: the constitution's Engineering Standards rule specifically calls out signature binding as a property-level claim; honouring it from 003 onward sets the discipline for 011's real-crypto tests.

## §9. ADR slot inventory for 003

**Decision**: two ADRs cover 003's structural decisions:

- **ADR 0009 — Crypto trait shape** (`docs/decisions/0009-crypto-trait-shape.md`): authored during pre-spec, committed to the 003 branch as `529bce0`. Captures the concrete `PublicKey` / `PrivateKey` / `Signature` newtypes choice (rejecting associated types), the asymmetric-shaped mock construction, the no-Signer-on-Node decision, and the `PublisherId(PublicKey)` newtype shape.
- **ADR 0010 — Protocol-message type hierarchy** (`docs/decisions/0010-protocol-message-type-hierarchy.md`): authored post-`/speckit-plan` Phase 1, after the user surfaced that the original Message-as-struct shape conflated the §2.3 dissemination envelope with "every protocol message." Captures the `Message` enum reshape (with `#[non_exhaustive]`), the `SignedMessage` / `PlainMessage` split (which eliminates the placeholder-signature workflow), the `MessagePayload` preservation, the `MessageHash::of(&PlainMessage)` content-anchored hash decision, and the 001 `Envelope` → `RoutingFrame` rename.

**Rationale**: every other 003 delta is a tactical extension of patterns these two ADRs establish (or that 001 / 002 established earlier):

- Receive-task verification step + pattern-match wrapper: extends ADR 0006 (receive-task and registration) by adding a step inside the loop and a pattern-match around it. The loop's structural shape — single async task per Node, mpsc receiver, snapshot append on success, drop+log on miss — is unchanged.
- Canonical-encoding seam (`PlainMessage::signed_bytes`): a single hand-written function on `PlainMessage`. The interface is one method; no trait, no abstraction, no public-API rippling.
- Drop-event convention (`event = "message_dropped"` + `cause`): a cross-feature convention recorded in saved memory `feedback_message_dropped_event_convention.md`. It applies workstream-wide, not 003-specific; no ADR slot.
- Test-support helper (`build_signed_message_simple`): lives in `tests/common/`, not in `src/`; not part of the public API surface; no ADR needed.

**Alternatives considered**:

- **A third ADR covering "receive-task pipeline ordering"** (the topic-filter-first decision from Q6). Rejected: the decision is one sentence, the trade-off space is small (verify-first vs filter-first vs implementer-choice), and the rationale fits comfortably in the Clarifications bullet plus the FR-013 body. An ADR would be ceremonial without informational gain.
- **An ADR covering "canonical encoding format" (hand-rolled vs CBOR)**. Rejected: the decision and its forward-compat plan are already captured in `IMPLEMENTATION_NOTES.md` N-004, which is the project's deferred-revisit ledger. N-004 carries the revisit trigger (feature 009 / first cross-language consumer); an ADR would duplicate without adding.
- **A separate ADR for the `MessageHash::of(&PlainMessage)` content-anchored hash decision**. Rejected: the choice is intertwined with the type-hierarchy split (the `PlainMessage` / `SignedMessage` separation is what *enables* hashing the content alone), so the rationale belongs in ADR 0010's Consequences section rather than its own ADR. A revisit trigger is recorded in IMPLEMENTATION_NOTES.md N-005 for when downstream features start operationally consuming the hash.

## §10. Open follow-ups (deferred to later features)

These are plan-level items the 003 plan acknowledges but explicitly defers. Each carries a re-visit trigger so a future session does not silently rediscover the same trade-offs.

1. **`Cargo.toml` dep gating for `crypto::mock`** (feature flag). At 003 the mock module is unconditionally `pub`; the binary uses it. When feature 011's `Ed25519CryptoScheme` lands and the binary's default verifier shifts to real crypto, a `#[cfg(feature = "mock-crypto")]` gate on the mock module becomes attractive — operators in production no longer pay for the mock surface. Revisit trigger: feature 011, when the binary's default verifier swaps.
2. **`MessagePayload` variant additions**. 003 keeps `Ping(u64)` as the only variant (inherited from 002). When subsequent features add variants (`Heartbeat`, `Announce`, etc.), the explicit-tag pattern from §7 carries forward; the test that pins existing tags (added in `/speckit-tasks`) ensures stability. Revisit trigger: the first feature that adds a `MessagePayload` variant.
3. **Persistence of `KeyPair`s on disk**. `MockCryptoScheme` is in-memory; tests construct fresh schemes per run. The future publisher CLI (post-008) will need to persist a long-term keypair somewhere. Revisit trigger: the publisher-CLI feature that introduces it (likely between 008 and 011).
4. **Constant-time comparison for `PrivateKey`**. 003 derives `Eq + PartialEq` on `PrivateKey`. Real Ed25519 / BLS work would prefer `subtle::ConstantTimeEq`. Revisit trigger: feature 011 alongside the `Zeroize` derive on `PrivateKey`.
5. **`Verifier` variant errors beyond `Invalid`**. FR-005's `VerifyError` is `#[non_exhaustive]` and v1 ships with one variant. The first verifier that distinguishes failure modes (key format invalid, algorithm mismatch, signature format invalid) adds variants without breaking callers. Revisit trigger: feature 011 + cross-language consumers in feature 009.
6. **Verification performance characterisation**. 003's `TestVerifier::verify` is a SHA-256 (sub-microsecond per message). Real Ed25519 verify is ~50–100 µs per message on modern hardware. The receive-task ordering decision (topic filter first per Q6) was made partly in anticipation of this. Revisit trigger: feature 011 — at that point a microbenchmark or `criterion` harness for the receive task is worth its setup cost.
7. **Wire-level hash / dedup primitive distinct from `MessageHash`**. Per ADR 0010 + IMPLEMENTATION_NOTES.md N-005, `MessageHash::of` hashes `PlainMessage` only (content); two `SignedMessage`s with identical content but different signatures hash to the same `MessageHash`. If a future caching, retransmission, or dedup layer needs to distinguish those wire-different forms, a sibling `WireHash` over `SignedMessage` becomes warranted. The two coexist; neither replaces the other. Revisit trigger: the first feature that introduces wire-level dedup (likely 009 TCP transport or a 006-era fan-out optimisation).

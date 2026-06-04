---

description: "Tasks: Message Envelope + Mock Crypto"
---

# Tasks: Message Envelope + Mock Crypto

**Input**: Design documents from `/specs/003-message-envelope-mock-crypto/`

**Prerequisites**: plan.md ✓, spec.md ✓, research.md ✓, data-model.md ✓, contracts/library-api.md ✓, quickstart.md ✓ (002's `contracts/node-config.toml.md` is inherited unchanged per FR-017 — no 003 contract file for TOML; 001's `contracts/cli.md` is also inherited unchanged)

**Tests**: Tests are **MANDATORY** for 003 — the feature carries a protocol-behavior claim (signature authenticity) per Constitution Principle II's "envelope handling, message verification" carve-out. **Strict red-green TDD applies for User Story 1**: the US1 acceptance-scenario tests (signature verification on the receive path) MUST be authored BEFORE the receive-task implementation lands; each must fail against an unimplemented verifier, then the implementation makes them pass. US2 / US3 / US4 tests verify the same mechanism in different configurations; they may be authored after US1's implementation is in place (they will pass without further implementation work because the substrate is shared). The drop event payloads (FR-014's `event="message_dropped"` / `cause="invalid_signature"`, FR-015's `cause="topic_not_subscribed"`) are deliberately **not** test-anchored — they are operator UX per the tests-don't-check-logs convention (Clarifications Q4-fallout + 002 FR-011 / FR-014 precedent). Tests assert on `received_messages()` snapshot presence/absence only.

**ADRs**: Two structural decisions are captured as ADRs for 003: **ADR 0009** (`docs/decisions/0009-crypto-trait-shape.md`, authored pre-spec, committed `529bce0`) covers the crypto trait shape. **ADR 0010** (`docs/decisions/0010-protocol-message-type-hierarchy.md`, authored post-Phase-1, committed `78e061b`) covers the protocol-message type hierarchy + the `MessageHash::of(&PlainMessage)` content-anchored hash decision + the 001 `Envelope` → `RoutingFrame` rename. No new ADRs are authored during `/speckit-implement`; the receive-task verification step, the canonical-encoding seam, the 002 emitter rename, and the test-support helper layout are all tactical extensions of decisions already locked.

**Organization**: Tasks are grouped by user story. The 003 substrate (crypto module + traits + mock impls + Message reshape + RoutingFrame rename + test helper + shared TestVerifier fixture) is **foundational** — all four user stories depend on it. US1 is the MVP and carries the protocol-behavior implementation work (the receive-task verification step). US2 / US3 / US4 are predominantly test-only phases that exercise the substrate from new configurations.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Different files, no incomplete dependencies — can run in parallel
- **[Story]**: US1 / US2 / US3 / US4 — user-story phase tasks only
- File paths are absolute from the crate root (`pubsub-node/`)

## Path Conventions

- **Single Cargo crate** (lib + bin) per `plan.md` "Project Structure" — unchanged from 001 / 002
- Source: `src/`
- Integration tests: `tests/`
- ADRs: `docs/decisions/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: 003 inherits the Cargo crate, lint configuration, and most of the dependency set from 001 / 002 (002 added no new deps; 001 established the baseline). Phase 1 confirms the 003 branch starts green and adds the three runtime deps + one test-only dep that 003 requires.

- [ ] T001 Verify the 003 branch baseline is green before touching any code: run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo build`, and `cargo test` from `pubsub-node/`. All four MUST pass (the 003 branch was cut from main `b7fa9e5` which inherits 001 / 002's green checkpoints; this task guards against unexpected drift on the branch). No code edits in this task — observation only. Per the saved feedback memory `feedback_cargo_fmt_per_commit`.

- [ ] T002 Add the three runtime dependencies and one test-only dependency to `Cargo.toml` (per `plan.md` Technical Context + `research.md §5`): under `[dependencies]` add `rand = "0.8"`, `rand_chacha = "0.3"`, `sha2 = "0.10"`; under `[dev-dependencies]` add `proptest = "1"`. Run `cargo build` and `cargo test` to confirm the deps resolve and the crate still compiles green (no callers exist for the new deps yet, so behavior is unchanged). Per FR-007 (rand + rand_chacha), FR-011 (sha2), and the Constitution Engineering Standards rule on property-based testing (proptest, exercised in T023). NOT parallelizable with T001 (T001 must complete first to establish a clean baseline).

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: All shared substrate every user story depends on — the new `pubsub_node::crypto` module + traits + mock impls + the `PublisherId` newtype + the `RoutingFrame` rename of the 001 `Envelope` + the `Message` reshape into a `#[non_exhaustive]` enum carrying `SignedMessage { plain: PlainMessage, signature: Signature }` + the test-support helpers + a shared `TestVerifier` test fixture.

**⚠️ CRITICAL**: No user-story phase work can begin until this phase completes. Several tasks in this phase are **breaking changes** that update many call sites in a single commit to preserve the green-checkpoint invariant (Constitution §"Development Workflow"); they are explicitly NOT parallelizable.

- [ ] T003 Scaffold the `src/crypto/` module directory: create `src/crypto/mod.rs` (empty stub) and add `pub mod crypto;` to `src/lib.rs`. Add a top-of-file rustdoc to `src/crypto/mod.rs` summarising the module's purpose ("Crypto trait pair + concrete byte-newtype types per ADR 0009; mock impls live in `mock`"). The file is otherwise empty at this task; subsequent tasks populate it. Verify `cargo build` is green. Per `plan.md` Project Structure + `research.md §1`.

- [ ] T004 Define the six concrete byte-newtype types in `src/crypto/mod.rs` per FR-003 + ADR 0009 + Clarifications Q3 / Q4: `pub struct PublicKey(Vec<u8>)` with `derive(Clone, Debug, Eq, PartialEq, Hash)`, accessor `pub fn new(bytes: Vec<u8>) -> Self` + `pub fn as_bytes(&self) -> &[u8]`, and `impl Display` rendering full lowercase hex; `pub struct PrivateKey(Vec<u8>)` with `derive(Clone, Eq, PartialEq)` (NO `Hash`, NO derived `Debug`) plus a hand-written `impl Debug` printing `"PrivateKey([REDACTED])"`, accessor `pub fn new(bytes: Vec<u8>) -> Self` + `pub fn as_bytes(&self) -> &[u8]`, NO `Display` impl; `pub struct Signature(Vec<u8>)` with `derive(Clone, Debug, Eq, PartialEq)` (NO `Hash`), accessor `pub fn new(bytes: Vec<u8>) -> Self` + `pub fn as_bytes(&self) -> &[u8]`, and `impl Display` rendering full lowercase hex (no `placeholder()` constructor per ADR 0010 + Clarifications Q3 + checklist CHK044); `pub struct MessageHash([u8; 32])` with `derive(Clone, Debug, Eq, PartialEq, Hash)`, `pub const ZERO: MessageHash = MessageHash([0u8; 32])`, accessor `pub fn new(bytes: [u8; 32]) -> Self` + `pub fn as_bytes(&self) -> &[u8; 32]`, and `impl Display` rendering 64 lowercase hex chars; `pub struct Timestamp(u64)` with `derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)`, constructors `pub fn now() -> Self` (resolving `SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64`) + `pub fn from_millis(ms: u64) -> Self`, and `pub fn as_millis(&self) -> u64`. Per `data-model.md §§1, 2, 3, 4, 5` + `contracts/library-api.md` `pubsub_node::crypto` section. Verify `cargo build` + `cargo clippy` green. Unit-test the `PrivateKey` redacting-Debug formatter (`assert_eq!(format!("{:?}", PrivateKey::new(vec![1,2,3])), "PrivateKey([REDACTED])")`) to lock SC-006's secret-discipline property at the type level.

- [ ] T005 Define `pub enum VerifyError` in `src/crypto/mod.rs` per FR-005 + Clarifications Q5: marked `#[non_exhaustive]`, derives `Debug` + `thiserror::Error`, sole v1 variant `#[error("invalid signature")] Invalid`. Per `data-model.md §7` + `contracts/library-api.md` `VerifyError` row. Verify `cargo build` + `cargo clippy` green.

- [ ] T006 Define the `Signer` and `Verifier` trait pair in `src/crypto/mod.rs` per FR-004 + FR-005 + ADR 0009. `pub trait Signer: Send + Sync { fn public_key(&self) -> PublicKey; fn sign(&self, msg: &[u8]) -> Signature; }` and `pub trait Verifier: Send + Sync { fn verify(&self, key: &PublicKey, msg: &[u8], sig: &Signature) -> Result<(), VerifyError>; }`. Both traits MUST take no type parameters and define no associated types (ADR 0009's no-associated-types decision). Per `data-model.md §§8, 9` + `contracts/library-api.md` `Signer trait` + `Verifier trait` rows. Verify `cargo build` + `cargo clippy` green; the traits have no impls yet — the build still compiles because no code holds an `Arc<dyn Verifier>` yet.

- [ ] T007 Scaffold the `src/crypto/mock.rs` submodule: create the file, add `pub mod mock;` to `src/crypto/mod.rs`. Add a prominent module-level rustdoc carrying the **"MOCK — not unforgeable"** warning per SC-006 (verbatim from the spec's wording: "The mock implementations below are designed to mirror the *shape* of real asymmetric crypto without imposing its security properties. Anyone with read access to this module's source can produce a forged signature that `TestVerifier::verify` accepts. The mock exists solely to differentiate correct-vs-incorrect key+message bindings in tests. Real authenticity arrives in feature 011."). Verify `cargo build` green. Per `data-model.md §§10-15` + `contracts/library-api.md` `pubsub_node::crypto::mock module` section.

- [ ] T008 In `src/crypto/mock.rs`, define the shared building blocks per FR-009 + Clarifications Q3: `pub(crate) const PUBLIC_SUFFIX: &[u8] = b"_public";` (the byte sequence shared between `derive_public` and `TestVerifier::verify` — declared once so the two operations stay byte-symmetric by construction); `pub fn derive_public(private: &PrivateKey) -> PublicKey` returning `PublicKey::new(private.as_bytes().to_vec() concatenated with PUBLIC_SUFFIX)`; `pub struct KeyPair { pub public: PublicKey, pub private: PrivateKey }` with `derive(Clone, Debug)` (Debug delegates to `PrivateKey`'s redacting impl, so `format!("{:?}", kp)` yields `KeyPair { public: PublicKey(…), private: PrivateKey([REDACTED]) }`). Per `data-model.md §§11, 14, 15`. Verify `cargo build` green.

- [ ] T009 In `src/crypto/mock.rs`, implement `MockCryptoScheme` per FR-006 + FR-007 + FR-008 + Clarifications Q4: `pub struct MockCryptoScheme { rng: rand_chacha::ChaCha20Rng }`; constructors `pub fn with_seed(seed: [u8; 32]) -> Self` (uses `rand::SeedableRng::from_seed(seed)`) and `pub fn from_entropy() -> Self` (uses `rand_chacha::ChaCha20Rng::from_entropy()`); `pub fn generate_keypair(&mut self) -> KeyPair` draws 32 fresh bytes from the internal RNG into a `private: PrivateKey` then computes `public = derive_public(&private)` and returns the `KeyPair`; `pub fn signer(&self, private: PrivateKey) -> TestSigner` (returns `TestSigner::new(private)` — `&self`, does not advance the RNG); `pub fn verifier(&self) -> TestVerifier` (returns `TestVerifier` — `&self`, does not advance the RNG; verifier is a unit struct). Add struct-level rustdoc carrying the **"MOCK — not unforgeable"** warning per SC-006. Per `data-model.md §10`.

- [ ] T010 In `src/crypto/mock.rs`, implement `TestSigner` and `TestVerifier` per FR-009 + ADR 0009. `pub struct TestSigner { private: PrivateKey }` with `pub fn new(private: PrivateKey) -> Self`; `impl Signer for TestSigner { fn public_key(&self) -> PublicKey { derive_public(&self.private) }; fn sign(&self, msg: &[u8]) -> Signature { let mut input = self.private.as_bytes().to_vec(); input.extend_from_slice(msg); Signature::new(sha2::Sha256::digest(&input).to_vec()) } }`. `pub struct TestVerifier;` (unit struct, no fields — stateless) with `impl Verifier for TestVerifier { fn verify(&self, key: &PublicKey, msg: &[u8], sig: &Signature) -> Result<(), VerifyError> { let stripped = key.as_bytes().strip_suffix(PUBLIC_SUFFIX).ok_or(VerifyError::Invalid)?; let mut input = stripped.to_vec(); input.extend_from_slice(msg); let expected = sha2::Sha256::digest(&input); if sig.as_bytes() == expected.as_slice() { Ok(()) } else { Err(VerifyError::Invalid) } } }`. Add per-struct rustdoc carrying the **"MOCK — not unforgeable"** warning per SC-006 on each of `TestSigner` and `TestVerifier`. Per `data-model.md §§12, 13`. Verify `cargo build` + `cargo clippy` green; the mock impls are now usable from tests but no production code holds them yet.

- [ ] T011 [P] In `src/message.rs`, define `pub struct PublisherId(PublicKey)` per FR-002 + Clarifications Q2: `derive(Clone, Debug, Eq, PartialEq, Hash)`, `pub fn new(public: PublicKey) -> Self`, `impl From<PublicKey> for PublisherId`, `pub fn as_public_key(&self) -> &PublicKey`, `impl Display` delegating to the inner `PublicKey`'s `Display`. Per `data-model.md §6` + `contracts/library-api.md` `pubsub_node::PublisherId` section. Independent of T009 / T010 (different file, only depends on `PublicKey` from T004). Verify `cargo build` green.

- [ ] T012 Rename the 001-era `Envelope` routing wrapper to `RoutingFrame` in `src/network.rs` per FR-001 + ADR 0010 (the rename frees "envelope" for prose-level use matching the synthesis §2.3): rename `pub struct Envelope { from: PeerId, message: Message }` to `pub struct RoutingFrame { from: PeerId, message: Message }`. Update every caller in the crate **in the same commit** to preserve the green-checkpoint invariant: `src/network.rs` (any `impl` blocks, any internal references to the type name), `src/node.rs` (the recv-task variable is currently named `env: Envelope` — rename the variable to `frame: RoutingFrame` for clarity; the type rename forces the variable type-binding update), any other callers under `src/`. Then update `tests/common/mod.rs` and `tests/two_node_ping.rs` / `tests/n_node_graph.rs` / `tests/topic_filter.rs` / `tests/topic_runtime.rs` / `tests/config_loading.rs` if any of them pattern-match on or reference the `Envelope` type name (most do not, since the network layer abstracts it from test code). Update `src/lib.rs` re-export `pub use network::Envelope;` to `pub use network::RoutingFrame;`. Per `data-model.md §16d` + `contracts/library-api.md` `pubsub_node::network::RoutingFrame` row. NOT parallelizable — touches every file that names the old identifier.

- [ ] T013 Reshape `Message` and introduce `SignedMessage` + `PlainMessage` in `src/message.rs` per FR-001 + FR-010 + ADR 0010. Replace the 002-era `pub struct Message { pub topic: TopicId, pub payload: MessagePayload }` with a `#[non_exhaustive] #[derive(Clone, Debug, Eq, PartialEq)] pub enum Message { Signed(SignedMessage) }` (sole 003 variant; future variants land here per ADR 0010). Define `#[derive(Clone, Debug, Eq, PartialEq)] pub struct SignedMessage { pub plain: PlainMessage, pub signature: Signature }`. Define `#[derive(Clone, Debug, Eq, PartialEq)] pub struct PlainMessage { pub topic: TopicId, pub publisher_id: PublisherId, pub parent_hash: Option<MessageHash>, pub sequence: u64, pub timestamp: Timestamp, pub payload: MessagePayload }`. The 002-era `#[non_exhaustive] pub enum MessagePayload { Ping(u64) }` is preserved unchanged and lives as a field of `PlainMessage` (per `data-model.md §16c`). Implement `pub fn signed_bytes(&self) -> Vec<u8>` on `PlainMessage` with the FR-010 byte layout: `u32_be(topic.as_str().len()) || topic.as_str().as_bytes() || u32_be(publisher_id.as_public_key().as_bytes().len()) || publisher_id.as_public_key().as_bytes() || parent_hash.as_ref().map(|h| h.as_bytes()).unwrap_or(&[0u8; 32]) (32 bytes) || sequence.to_be_bytes() (8 bytes) || timestamp.as_millis().to_be_bytes() (8 bytes) || u32_be(payload_encoded.len()) || payload_encoded`. For `MessagePayload::Ping(n)`, `payload_encoded = [0x00] || n.to_be_bytes()` (1 byte variant tag `0x00` per FR-010 + research.md §7's explicit-match stability mechanism). Author the byte-layout rustdoc on `PlainMessage::signed_bytes` in full per FR-010's protocol-surface obligation (field order, widths, endianness, the `MessageHash::ZERO` sentinel for absent `parent_hash`, the `MessagePayload` variant tag values, the no-version-tag rationale). Also implement `MessageHash::of(plain: &PlainMessage) -> MessageHash` in `src/crypto/mod.rs` (compute `sha2::Sha256::digest(&plain.signed_bytes())` and wrap in `MessageHash`) — content-anchored per FR-011 + Clarifications hash-input bullet + N-005. **Breaking change** — every Message-construction call site MUST be updated in the same commit to preserve the green-checkpoint invariant: `tests/common/mod.rs` fixture builders (the 002-era helper that builds bare `Message { topic, payload }` updates to construct via `Message::Signed(SignedMessage { plain: PlainMessage { topic, publisher_id, parent_hash: None, sequence: 0, timestamp: Timestamp::from_millis(0), payload }, signature })` — using a sentinel TestSigner provided by the fixture; the actual `build_signed_message_simple` helper API lands in T015), `tests/two_node_ping.rs`, `tests/n_node_graph.rs`, `tests/topic_filter.rs`, `tests/topic_runtime.rs`, `tests/config_loading.rs` (any test that constructs a `Message`). Per `data-model.md §§16-16d` + `contracts/library-api.md` `Message` / `SignedMessage` / `PlainMessage` rows + research.md §6 step 4 + N-004. NOT parallelizable — this is the largest single-commit migration in 003.

- [ ] T014 Add the 003 re-exports to `src/lib.rs` (depends on T004, T005, T006, T008, T009, T010, T011, T013). Add: `pub use crypto::{PublicKey, PrivateKey, Signature, MessageHash, Timestamp, VerifyError, Signer, Verifier};` and `pub use crypto::mock::{MockCryptoScheme, KeyPair, TestSigner, TestVerifier, derive_public};` (note `PUBLIC_SUFFIX` is `pub(crate)` and NOT re-exported). Add: `pub use message::{Message, SignedMessage, PlainMessage, PublisherId};` (the existing `pub use message::Message;` line from 002 — `Message`'s public name survives unchanged from 002 to 003 even though the underlying shape changed from struct to enum). The existing `pub use message::MessagePayload;` line from 002 is unchanged. The `pub use network::RoutingFrame;` line (post-T012 form) is unchanged by this task. Verify `cargo build` is green and that downstream consumers can name every type listed in `contracts/library-api.md` "Re-exports" via the flat `pubsub_node::…` namespace. Per `contracts/library-api.md` "Re-exports from `pubsub_node` — additions and reshapings" section.

- [ ] T015 Extend `tests/common/mod.rs` with the test-support helpers per `research.md §4`. Add `pub fn build_signed_message(signer: &impl Signer, topic: TopicId, payload: MessagePayload, sequence: u64, parent_hash: Option<MessageHash>, timestamp: Timestamp) -> Message`: constructs a `PlainMessage` with the inputs (publisher_id derived from `signer.public_key()` wrapped in `PublisherId::from`), computes `plain.signed_bytes()`, signs the bytes with the provided signer, assembles `SignedMessage { plain, signature }`, returns `Message::Signed(signed)`. Add convenience wrapper `pub fn build_signed_message_simple(signer: &impl Signer, topic: TopicId, payload: MessagePayload) -> Message` defaulting `sequence = 0`, `parent_hash = None`, `timestamp = Timestamp::from_millis(0)`. Also add the shared `TestVerifier` fixture: `pub fn shared_test_verifier() -> Arc<dyn Verifier> { Arc::new(TestVerifier) }` (or analogous static `OnceLock<Arc<dyn Verifier>>` if the test suite already uses lazy-static patterns; consistent with the 002 fixture style). Update the existing fixture builders (`two_node_fixture` and any sibling helpers from 001 / 002) to take an `Arc<dyn Verifier>` parameter (default to the shared `TestVerifier` when callers don't override) and to pass it to `Node::new`; default callers continue to behave correctly because the fixture's internal `build_signed_message_simple` helper signs with a fixture-owned `TestSigner` whose `derive_public` output matches what `TestVerifier::verify` will reconstruct. Per `data-model.md §10-15` + `research.md §4`.

---

## Phase 3: User Story 1 — Signature Verification on the Receive Path (Priority: P1) 🎯 MVP

**Goal**: signature-valid messages flow into the existing 002 arrival log unchanged; signature-invalid messages are silently dropped with a structured `event = "message_dropped"` / `cause = "invalid_signature"` tracing entry. This is the irreducible protocol-behavior claim of 003 and the trigger for strict TDD per Constitution Principle II's "envelope handling, message verification" carve-out.

**Independent Test**: per the spec's US1 Independent Test — spawn two nodes A and B sharing an InMemory network and a shared `Arc<TestVerifier>`; construct a signed `Message::Signed(SignedMessage { plain, signature })` via `build_signed_message_simple`; send from B to A; observe A's `received_messages()` snapshot. Then construct a tampered variant (one altered payload byte without re-signing) and confirm it does NOT appear in A's snapshot.

### Tests for User Story 1 (MANDATORY — Constitution Principle II, strict TDD) ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before T017–T020 implementation lands. The tests fail initially because Node has no `verifier` field yet — the build fails to compile against the new Node constructor signature, which is the canonical TDD-red state.**

- [ ] T016 [US1] Implement integration tests in `tests/signed_message.rs` covering all four US1 acceptance scenarios per `spec.md` US1 AS-1 through AS-4. Each test is `#[tokio::test]`, each uses `mod common;` to access the extended fixture from T015. Required tests: **`valid_signature_message_retained`** (US1 AS-1): construct a signed Message via `build_signed_message_simple(&signer, topic, MessagePayload::Ping(42))` where `(signer, _public)` comes from `MockCryptoScheme::with_seed([0u8; 32]).generate_keypair()`; send from B to A subscribed to that topic; assert A's `received_messages()` contains exactly one delivery with the matching `Message::Signed(SignedMessage { plain: PlainMessage { publisher_id, .. }, signature })` value. **`payload_tampered_after_signing_dropped`** (US1 AS-2): construct a valid signed Message, then clone-and-mutate the `payload` field (e.g. `Ping(42)` → `Ping(43)`) without re-signing; assert the tampered Message does NOT appear in A's `received_messages()`. **`bogus_signature_dropped`** (US1 AS-3): construct a Message with a valid publisher_id but `signature = Signature::new(vec![0u8; 32])` (32 zero bytes — explicitly NOT using a `Signature::placeholder()` constructor which does not exist per ADR 0010 + checklist CHK044); assert the bogus-signature Message does NOT appear in the snapshot. **`publisher_id_mismatched_with_signing_key_dropped`** (US1 AS-4): build two keypairs (X and Y) from one scheme, sign with `TestSigner::new(kp_x.private)` but construct the envelope with `publisher_id = PublisherId::from(kp_y.public)`; assert the mismatched Message does NOT appear in the snapshot. The log-content assertions described in `spec.md` US1's Then clauses are DESCRIPTIVE (operator UX) — tests assert only on `received_messages()` per the tests-don't-check-logs convention (Clarifications Q4 + FR-014). Per `spec.md` US1 AS-1 through AS-4. Verify `cargo test --test signed_message` FAILS at this point (because the Node::new signature does not yet take a verifier; the tests won't compile).

### Implementation for User Story 1

- [ ] T017 [US1] Add the `verifier` field to `Node` and update `Node::new` to accept it (per FR-012 + `research.md §3`). In `src/node.rs`: add `verifier: Arc<dyn Verifier>` field to the `pub struct Node` definition (alongside the existing 002 fields). Update the constructor signature to `pub async fn new<N: Network>(self_id: PeerId, config: NodeConfig, initial_subscriptions: HashSet<TopicId>, network: Arc<N>, verifier: Arc<dyn Verifier>) -> Result<Self, NodeError>` (the `verifier` parameter is appended at the end of the existing 002 parameter list per `research.md §3`'s decision). In the constructor body, store the provided `verifier` on the struct; the recv-task code that will use it is implemented in T018. The Node MUST NOT add a `signer` field at this iteration per FR-012 + ADR 0009. Per `data-model.md §17` + `contracts/library-api.md` `Node::new` row. **Also**: while updating call sites for the new 5-parameter signature, sweep `src/network.rs` lines 125–129 — the `InMemoryNetwork` rustdoc carries a `Node::new` example inside a ` ```ignore ` fence (001-era debt, last hand-edited in commit `b5ed478` when 002 renamed `peer_list` → `config`). The `ignore` fence silently lets the example rot as `Node::new`'s signature evolves. Convert it to `no_run` with a hidden async-fn wrapper so future signature drift becomes a `cargo test` compile error rather than silent doc rot — concretely: change the opening fence from ` ```ignore ` to ` ```no_run `, prepend `# async fn run() -> Result<(), Box<dyn std::error::Error>> {` (with the `#` prefix that hides the line from rendered HTML but compiles), append `# Ok(()) }` similarly, and update the body to the new 5-parameter `Node::new(self_id, config, initial_subscriptions, network.clone(), verifier)` call. After this edit, `cargo test` runs the example as a compile-only doc-test (no execution side effects, but signature mismatches break the build). Per Constitution Principle I (Correctness Over Optimization — accurate docs are correctness).

- [ ] T018 [US1] Implement the receive-task pattern-match + verification step + the atomic same-commit `topic_drop` → `message_dropped` rename in `src/node.rs` per FR-013 + FR-014 + FR-015 + SC-007. Update the recv-task body to: `while let Some(frame) = rx.recv().await { match frame.message { Message::Signed(signed) => { /* per-FR-013 pipeline below */ } } }` — the `frame: RoutingFrame` variable uses the post-T012 name. Inside the `Signed(signed)` arm: (1) acquire the `subscriptions` mutex briefly and check `subscriptions.contains(&signed.plain.topic)`; on miss, drop the message and emit `tracing::info!(target: "pubsub_node::node", event = "message_dropped", cause = "topic_not_subscribed", self_id = %self.self_id, from = %frame.from, topic = %signed.plain.topic)` (this REPLACES the 002-era `event = "topic_drop"` emission in the same commit per FR-015 — no source line in the crate emits `"topic_drop"` after this task lands; SC-007 enforces atomicity via `grep -r "topic_drop"` returning no production-code matches). (2) On topic-filter hit: compute `signed.plain.signed_bytes()` and call `self.verifier.verify(signed.plain.publisher_id.as_public_key(), &signed.plain.signed_bytes(), &signed.signature)`. (3) On `Ok(())`: push the `ReceivedDelivery { from: frame.from, message: Message::Signed(signed) }` into the `received` mutex per the 001 FR-006 snapshot-append contract (extended by 002 FR-004 with the topic-filter precondition). (4) On `Err(VerifyError::Invalid)` (or any future variant per FR-005's `#[non_exhaustive]`): drop the message and emit `tracing::info!(target: "pubsub_node::node", event = "message_dropped", cause = "invalid_signature", self_id = %self.self_id, from = %frame.from, topic = %signed.plain.topic, publisher_id = %signed.plain.publisher_id)` per FR-014. The verifier call MUST be synchronous (returns `Result<(), VerifyError>` directly); it MUST NOT introduce an `await` point per FR-020. The topic-filter-first ordering avoids signature-verification cost on off-topic traffic (Q6) and preserves the 002 receive-task pipeline structure. **Operator-facing-string convention**: the log entries MUST NOT cite any FR identifier or spec section in the entry text (per the saved feedback memory `feedback_no_fr_citations_in_operator_strings`). Per `data-model.md §17` + `contracts/library-api.md` Receive-task pipeline section. NOT parallelizable — touches the receive-task body atomically.

- [ ] T019 [US1] Update `src/main.rs` to construct a verifier and thread it through to `Node::new` per FR-017. After the existing `load_node_config` + `initial_subscriptions` setup, construct `let verifier: Arc<dyn Verifier> = Arc::new(TestVerifier);` (at this prototype-stage iteration the binary unconditionally uses the mock verifier per FR-017; when feature 011 lands the construction switches to `Ed25519CryptoScheme::*.verifier()` per FR-018 with no other public-API changes). Pass `verifier` as the fifth argument to `Node::new(self_id, config, initial_subscriptions, network, verifier).await?`. Verify `cargo build` is green and `cargo run -- --self-id node-a --config <path>` still starts a Node successfully (no behavior change from an operator's perspective at startup; the verifier is consulted only when inbound messages arrive). Per `contracts/library-api.md` `Node::new` row + FR-017.

- [ ] T020 [US1] Migrate any 002-era test that previously filtered log capture on `event == "topic_drop"` to the new `event == "message_dropped"` AND `cause == "topic_not_subscribed"` shape (per FR-015 + SC-007). Concretely: search `tests/` for any test that asserts on the legacy event name (most 002 tests do NOT assert on log content per the tests-don't-check-logs convention; this migration is bounded). Update each such site in the same commit as T018, per FR-015's MUST-same-commit atomicity requirement. In practice this task is typically a no-op because the tests-don't-check-logs convention (FR-014 + 002 FR-011 / FR-014) means few or no 002 tests filter on `event == "topic_drop"` in the first place; if the migration set turns out to be empty, T020 closes as a no-op verification rather than a follow-up commit. After this task, `grep -r "topic_drop"` across `pubsub-node/` MUST return zero matches in production code paths; the legacy event name MUST NOT appear in any production emitter call site or in any test in code (per FR-014's tightened tests-don't-check-logs convention — no automated test validates log emission, including via source-grep). Verify `cargo test` green — all 002 tests pass under the migration, and the new T016 tests (signed_message.rs) now pass because the receive-task verification step is in place.

**Checkpoint**: At this point, US1 is complete. The MVP demonstrates signature verification on the receive path with the new structured `message_dropped` event covering both drop causes. /speckit-implement can stop here and validate the MVP independently before proceeding to US2 / US3 / US4.

---

## Phase 4: User Story 2 — Multi-Publisher Verification (Priority: P2)

**Goal**: A's receive path uses each message's own `publisher_id` to select the verification key — there is no single per-node publisher key. Messages signed by their declared publisher land in the snapshot; messages whose declared publisher does not match the signing key are dropped + logged.

**Independent Test**: per the spec's US2 Independent Test — build three keypairs (Alice / Bob / Carol) from one `MockCryptoScheme`; sign one message per publisher tagged with the same topic; have B deliver all three to A subscribed to that topic; assert A's snapshot contains all three deliveries with the correct publisher_ids. Then alter one message's publisher_id to a different publisher's public key (without re-signing) and confirm only the two unaltered messages land in the snapshot.

**No new implementation needed** — the receive-task verification step implemented in T018 already dispatches verification using each message's own `publisher_id` per FR-013. US2 tests should pass on their own after T018 lands.

### Tests for User Story 2

- [ ] T021 [P] [US2] Implement integration tests in `tests/multi_publisher.rs` covering all three US2 acceptance scenarios per `spec.md` US2 AS-1 through AS-3. Each test is `#[tokio::test]`, each uses `mod common;`. Required tests: **`three_publishers_all_accepted`** (US2 AS-1): build three keypairs (alice/bob/carol) from `MockCryptoScheme::with_seed([0u8; 32])`; build a `TestSigner` for each; construct three signed messages (one per publisher) tagged with the same topic `T1`; send all three from B to A subscribed to `{T1}`; assert A's `received_messages()` contains all three deliveries in arrival order, each carrying the correct `publisher_id`. **`mismatched_publisher_id_rejected`**: like AS-1 but with Alice's message's `publisher_id` swapped to Bob's public key (signature unchanged — still Alice's signature over the original bytes); assert A's snapshot contains only Bob's and Carol's deliveries. **`interleaved_50_messages_5_publishers`** (US2 AS-3): build five keypairs, sign 50 messages total drawn from this pool (e.g., 10 per publisher in round-robin order), have B deliver them interleaved; assert A's snapshot contains exactly the 50 messages, the per-sender FIFO arrival ordering is preserved as inherited from 001 (the InMemoryNetwork's contract) and 002 (the receive-task contract). Per `spec.md` US2 AS-1, AS-2, AS-3. Verify `cargo test --test multi_publisher` passes.

**Checkpoint**: User Stories 1 AND 2 should both work independently.

---

## Phase 5: User Story 3 — Filter Composition with Topic Subscription (Priority: P3)

**Goal**: The 002 topic-subscription filter and the new signature filter compose. A message reaches A's snapshot only if (a) A subscribes to the message's topic AND (b) the signature verifies. Both filters drop with the same `event = "message_dropped"` structured tracing event, distinguished by the `cause` field. The 002-era `event = "topic_drop"` emission is REMOVED in the same commit as the new `invalid_signature` emitter (T018); no production code emits the legacy name.

**Independent Test**: per the spec's US3 Independent Test — A subscribed to `{T1}`, holding a `TestVerifier`; send four messages from B to A spanning the on-topic/off-topic × valid-signature/invalid-signature matrix; observe A's snapshot contains exactly the valid-on-topic message.

**No new implementation needed** — both filters are implemented in T018 (FR-013's topic-filter-first ordering per Q6). US3 tests verify the composition.

### Tests for User Story 3

- [ ] T022 [P] [US3] Implement integration tests in `tests/filter_composition.rs` covering US3 acceptance scenarios AS-1 through AS-4 per `spec.md` (US3 AS-5 — no legacy `event = "topic_drop"` in the log stream — is operator-UX-only per FR-014's tightened tests-don't-check-logs convention; SC-007 is verified via T028's polish-phase agent-run grep plus manual log inspection during the SC-001 / SC-002 demonstration, NOT via a test in code). Each test is `#[tokio::test]`, each uses `mod common;`. Required tests: **`valid_on_topic_message_appears_in_snapshot`** (US3 AS-1): A subscribed to `{T1}`; build a keypair + TestSigner; B sends a signed message tagged `T1`; assert A's snapshot contains exactly that delivery. **`valid_off_topic_message_dropped_with_cause_topic_not_subscribed`** (US3 AS-2): same setup; B sends a valid-signature message tagged `T2` (NOT in A's subscription set); assert A's snapshot does NOT contain it. **`invalid_on_topic_message_dropped_with_cause_invalid_signature`** (US3 AS-3): same setup; B sends an invalid-signature message tagged `T1`; assert A's snapshot does NOT contain it. **`invalid_off_topic_message_dropped_with_cause_topic_not_subscribed`** (US3 AS-4 — Q6 ordering): same setup; B sends a message that is both off-topic (`T2`) AND has an invalid signature; assert A's snapshot does NOT contain it (per FR-013's topic-filter-first ordering, the topic filter rejects before the verifier runs; the invalid signature is never observed for this message). The log-content assertions in `spec.md` US3 AS-1 through AS-4's Then clauses are DESCRIPTIVE (operator UX) — tests assert only on `received_messages()` per the tests-don't-check-logs convention. Verify `cargo test --test filter_composition` passes (4 tests).

**Checkpoint**: User Stories 1, 2, AND 3 should all work independently.

---

## Phase 6: User Story 4 — Reproducible Mock-Crypto Keypairs from a Seed (Priority: P4)

**Goal**: `MockCryptoScheme::with_seed(s)` produces deterministic keypairs across runs; `from_entropy()` produces non-deterministic keypairs. The `_public`-suffix derivation invariant holds for every generated `KeyPair`. The signature-binding property (the matching `(signer, public)` pair verifies; mismatched pairs reject) is exercised as a `proptest`-based property test per the Constitution's Engineering Standards rule.

**Independent Test**: per the spec's US4 Independent Test — build two `MockCryptoScheme` instances from the same 32-byte seed; generate three keypairs from each; assert byte-identical pairs. Build a third scheme from a different seed; assert its first keypair differs. Build a fourth scheme via `from_entropy()`; assert (statistically) its first keypair differs from the seeded schemes.

**No new implementation needed** — the mock crypto module is fully implemented in T007–T010. US4 tests verify its observable properties.

### Tests for User Story 4

- [ ] T023 [US4] Implement integration tests in `tests/mock_crypto_repro.rs` covering the five US4 acceptance scenarios per `spec.md` US4 AS-1 through AS-5 PLUS the proptest-based signature-binding property test per `research.md §8`. Each test is `#[tokio::test]` (or `#[test]` for the synchronous ones), each uses `mod common;` only where it needs shared fixtures (most US4 tests are scheme-construction-and-comparison and don't need any Node fixture). Required tests: **`same_seed_yields_byte_identical_keypair_sequences`** (US4 AS-1): construct two `MockCryptoScheme::with_seed([0u8; 32])` instances; for each, call `generate_keypair()` ten times collecting the `KeyPair` sequence; assert `scheme_1.kp_sequence == scheme_2.kp_sequence` byte-for-byte across all ten pairs (comparing `public.as_bytes()` and `private.as_bytes()`). **`different_seeds_yield_differing_keypairs`** (US4 AS-2): construct two schemes from different seeds (e.g., `[0u8; 32]` and `[1u8; 32]`); assert the first generated keypair differs in at least one byte of `public` or `private`. **`derive_public_invariant_holds_on_generated_keypairs`** (US4 AS-3): for any keypair from a `MockCryptoScheme`, assert `derive_public(&kp.private) == kp.public`. **`test_verifier_accepts_test_signer_signatures`** (US4 AS-4): generate a keypair; build a `TestSigner` from `kp.private`; sign arbitrary bytes; assert `TestVerifier.verify(&kp.public, &msg, &sig).is_ok()`. **`test_verifier_rejects_keys_without_public_suffix`** (US4 AS-5): construct a `PublicKey::new(vec![0xAB, 0xCD, 0xEF])` (bytes that do NOT end in `b"_public"`); call `TestVerifier.verify(&that_public, &any_msg, &any_sig)`; assert `Err(VerifyError::Invalid)`. **`signature_binding_proptest`** (per `research.md §8`): use `proptest! { #[test] fn signature_binding(seed in any::<[u8; 32]>(), msg in any::<Vec<u8>>()) { ... } }` to exercise the property "for any seeded keypair and arbitrary message, the matching signer's signature verifies under the matching public key; AND modifying any of (key, msg, sig) yields verifier rejection". Test fails initial assumptions are violated; shrinks to minimal failing cases per proptest's standard behavior. Per `spec.md` US4 AS-1 through AS-5 + `research.md §8` + Constitution Engineering Standards. Verify `cargo test --test mock_crypto_repro` passes (all six tests green).

**Checkpoint**: All four user stories are independently functional. /speckit-implement can declare 003's MVP-plus-extensions complete.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final quality sweeps — green-checkpoint, rustdoc audit, quickstart walkthrough, FR coverage spot-check, SC-006 + SC-007 verifications. Each task in this phase is independently runnable and verifies an invariant the rest of the implementation should already satisfy; failures here surface drift that earlier tasks missed.

- [ ] T024 Run the green-checkpoint sweep from `pubsub-node/`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo build`, `cargo test` (all integration suites + unit tests, no `--release` needed). All four MUST pass. Address any drift inline (formatting, clippy warnings, build errors, test failures) before committing the polish phase. Per the saved feedback memory `feedback_cargo_fmt_per_commit` and Constitution §"Green checkpoints".

- [ ] T025 Rustdoc audit on the new 003 public surface per the saved feedback memory `feedback_pubsub_node_doc_audiences`: ensure every newly-public item has a top-of-item `///` doc comment describing behavior in stable, audience-appropriate terms. Items to audit (per `contracts/library-api.md` "Re-exports" section): `PublicKey`, `PrivateKey` (including the no-Display, redacting-Debug discipline note for downstream consumers), `Signature`, `MessageHash` (including `MessageHash::ZERO` + `MessageHash::of`), `Timestamp` (including `now()` + `from_millis()`), `VerifyError` (including the `#[non_exhaustive]` callout for pattern-match call sites), `Signer` + `Verifier` traits, `MockCryptoScheme` (including the seeded-vs-entropy distinction), `KeyPair`, `TestSigner`, `TestVerifier`, `derive_public`, `PublisherId`, `Message` (the enum and the `Signed` variant's role), `SignedMessage`, `PlainMessage` (and its `signed_bytes` rustdoc which is normatively part of the protocol surface per FR-010 — verify the byte layout is documented in full per FR-010's content), `RoutingFrame` (the rename note pointing at ADR 0010). **Operator-facing-string convention applies**: no FR identifier or spec-section citations in rustdoc text (per `feedback_no_fr_citations_in_operator_strings`). Source `//` line comments + this `tasks.md` + `data-model.md` MAY cite FRs freely. Run `cargo doc --no-deps --document-private-items` and visually inspect the rendered HTML for completeness. Verify SC-006's "MOCK — not unforgeable" warning appears at the module level on `pubsub_node::crypto::mock`, AND on the struct rustdocs for `MockCryptoScheme`, `TestSigner`, `TestVerifier` (four sites total per SC-006).

- [ ] T026 Walk `quickstart.md` end-to-end manually following SC-004's ≤1-hour budget for a fresh contributor. Execute every command in §§1–6 against a clean checkout of the 003 branch (or in an isolated worktree). Specifically: (a) `cargo build` and confirm rand / rand_chacha / sha2 pull in; (b) `cargo test --test signed_message` passes (US1 — T016); (c) `cargo test --test multi_publisher` passes (US2 — T021); (d) `cargo test --test filter_composition` passes (US3 — T022); (e) `cargo test --test mock_crypto_repro` passes (US4 — T023); (f) `cargo test` (full suite) passes; (g) walk the §7 "Tour of the new types" code example in a scratch `examples/` file or in a transient unit test — verify the construction syntax compiles and the resulting `Message::Signed(SignedMessage { plain, signature })` verifies under `TestVerifier::verify`; (h) `cargo doc --open --no-deps` and inspect the generated rustdoc for the four "MOCK — not unforgeable" warning locations per SC-006. Update `quickstart.md` in-place if any command, expected output, or test name has drifted from the implementation reality. Per the SC-004 + checklist CHK037 cohesion rule.

- [ ] T027 FR coverage spot-check against `data-model.md §19`. For each FR row in the matrix, verify the cited entity entry / contract clause / test exists and asserts what the matrix claims. Specifically confirm: (a) every type listed in §19's "Entity / Surface" column corresponds to actual Rust code under `src/`; (b) every test name in the per-US sections of this `tasks.md` corresponds to a `#[tokio::test]` or `#[test]` function actually present in the relevant test file; (c) the tests-don't-check-logs convention is preserved — no test under `tests/` asserts on `tracing` log content for the `message_dropped` events. Update `data-model.md §19` if any of these mismatch — the matrix is the canonical traceability surface that `/speckit-analyze` (the next phase) consumes, so it MUST be accurate before that phase runs.

- [ ] T028 SC-007 grep verification: from `pubsub-node/`, run `grep -r "topic_drop" src/ tests/ examples/ 2>/dev/null | grep -v 'topic_not_subscribed'` and confirm zero production-code matches; the legacy literal event name MUST NOT appear in any emitter call site or in any test in code (per FR-014's tightened tests-don't-check-logs convention — no automated test validates log emission, including via source-grep). If any production-code match surfaces, address inline by completing T020's migration. Per SC-007 + FR-015.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — T001 (baseline check) must run first; T002 (deps) depends on T001's green baseline.
- **Phase 2 (Foundational)**: Depends on Phase 1. T003–T015 form the substrate — every user-story phase blocks on this phase.
- **Phase 3 (US1)**: Depends on Phase 2. T016 (tests) lands FIRST and is expected to FAIL (TDD); T017–T020 implement the receive-task verification and turn the tests green. T020 also migrates 002 tests that filter on the legacy event name.
- **Phase 4 (US2)**: Depends on Phase 3 (the verification mechanism implemented in T018 is what US2 exercises). T021 is test-only.
- **Phase 5 (US3)**: Depends on Phase 3 + Phase 4 conceptually (US3 exercises the same mechanism in composition with the 002 topic filter). T022 is test-only.
- **Phase 6 (US4)**: Depends on Phase 2 only (US4 exercises the mock crypto module directly, not the receive-task verification — it could in principle run after Phase 2 without any of Phases 3–5). T023 is test-only.
- **Phase 7 (Polish)**: Depends on all desired user stories being complete.

### Within Each User Story

- Tests (T016 for US1) MUST be written and FAIL before T017–T020 implementation per Constitution Principle II's TDD trigger.
- US2 / US3 / US4 tests may be authored after US1's implementation lands (they pass without new implementation).
- The atomic-same-commit rule between T018 (FR-014 emitter) and the topic_drop rename (FR-015) MUST be respected — T020's 002-test migration cleanly closes the rename per SC-007.

### Parallel Opportunities

- T011 (PublisherId in src/message.rs) is `[P]` — independent of the crypto-mock work in T007–T010 because it only depends on `PublicKey` from T004.
- T021, T022, T023 are all `[P]` — each writes to a different test file (multi_publisher.rs / filter_composition.rs / mock_crypto_repro.rs).
- Tasks under Phase 7 (T024–T028) are independent verifications; T024 should run before T025–T028 to land on a green baseline first; T025–T028 can run in parallel afterwards.

### Within Phase 2 (Foundational)

The bulk of Phase 2 is sequential because tasks edit overlapping files (`src/crypto/mod.rs` for T004–T006; `src/crypto/mock.rs` for T007–T010; `src/message.rs` for T011 + T013). T012 (RoutingFrame rename) and T013 (Message reshape) are both global breaking-change commits — they cannot be parallelized with each other or with the prior substrate tasks. T015 (test helpers) depends on T011 + T013 + T014 being in place.

---

## Parallel Example: User Story 2 + User Story 4

```bash
# After Phase 3 (US1) lands, US2 and US4 can run in parallel by different developers:
Task: "Integration tests in tests/multi_publisher.rs covering three US2 scenarios (T021)"
Task: "Integration tests + proptest in tests/mock_crypto_repro.rs covering six US4 scenarios (T023)"
```

US3 (T022) similarly fits this pattern but conceptually integrates US1 + US2 + the 002 topic-filter, so it's often easier to author after US2 lands.

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001–T002).
2. Complete Phase 2: Foundational (T003–T015).
3. Complete Phase 3: User Story 1 (T016–T020). T016 lands first as failing tests (TDD red); T017–T020 turn them green.
4. **STOP and VALIDATE**: run `cargo test --test signed_message` independently; demonstrate the MVP per SC-001 + SC-002 (under 30s wall-clock per scenario).
5. Deploy/demo if ready.

### Incremental Delivery

1. Setup + Foundational → substrate ready.
2. US1 → MVP (signature verification on the receive path).
3. US2 → multi-publisher verification (no new impl beyond US1).
4. US3 → filter composition with the 002 topic-filter (no new impl beyond US1; T020 closes the topic_drop rename).
5. US4 → mock crypto reproducibility (no new impl beyond Phase 2; proptest property-based signature-binding test per Constitution Engineering Standards).
6. Polish → green-checkpoint, rustdoc audit, quickstart walkthrough, FR coverage, SC-006 + SC-007 grep verification.

### Parallel Team Strategy

With multiple developers, after Phase 3 (US1) lands:

- Developer A: User Story 2 (T021)
- Developer B: User Story 3 (T022)
- Developer C: User Story 4 + the proptest property-based test (T023)

All three user stories land independently; Phase 7 polish runs once all three are green.

---

## Notes

- [P] tasks = different files, no incomplete dependencies
- [Story] label maps task to specific user story for traceability (US1 / US2 / US3 / US4)
- Each user story is independently completable and testable
- Verify US1 tests fail before implementing T017–T020 (Constitution Principle II strict TDD)
- Commit after each task or logical group; the Phase 2 breaking-change tasks (T012, T013) MUST be single coherent commits per the green-checkpoint rule
- Stop at any checkpoint to validate story independently
- Avoid: vague tasks, same-file conflicts that violate the green-checkpoint rule, cross-story dependencies that break independence
- The `analysis.md` file is NOT generated by `/speckit-tasks` — it's the destination for `/speckit-analyze`'s findings (per saved feedback memory `feedback_speckit_analysis_file`); it lands during the post-tasks Spec Kit phase that mirrors 002's `f2a373f` pass-4 zero-finding closure.

# Quickstart — Message Envelope + Mock Crypto

**Feature**: 003-message-envelope-mock-crypto
**Goal**: Reproduce the four-message filter-composition demonstration (US3 / SC-003) in under one hour, per SC-004, without consulting any document outside this feature directory.

This quickstart layers on top of 002's substrate. If you have not yet reproduced 002's multi-topic walkthrough, run `../002-topic-subscription-filtering/quickstart.md` first (it builds on 001's two-node Ping demo).

## Prerequisites

Same as 001 / 002: Rust stable ≥ 1.75, a POSIX shell, this repo checked out, working directory `pubsub-node/`. Three new direct dependencies (`rand`, `rand_chacha`, `sha2`) are added automatically the first time `cargo build` runs — no per-developer setup beyond `cargo` itself.

> **Shell note for fish users.** Several examples below use POSIX heredoc syntax (`cat > file <<'EOF' … EOF`) which fish does not parse. If your default shell is fish, drop into bash for the duration of this walkthrough: run `bash`, paste the examples verbatim, then `exit` when you're done.

## 1 — Build

```sh
cargo build
```

First post-002 build pulls in three new dependencies (`rand`, `rand_chacha`, `sha2`) and recompiles the crate. Subsequent builds are incremental.

## 2 — Run the signed-message integration test (003 US1)

```sh
cargo test --test signed_message
```

Expected:

```text
running 4 tests
test valid_signature_message_retained ... ok
test payload_tampered_after_signing_dropped ... ok
test bogus_signature_dropped ... ok
test publisher_id_mismatched_with_signing_key_dropped ... ok

test result: ok. 4 passed; 0 failed; 0 ignored
```

Each test:

- Spawns two Nodes `A` and `B` sharing an `InMemoryNetwork` and a shared `Arc<TestVerifier>`.
- Constructs a signed `Message` using `MockCryptoScheme::with_seed([0u8; 32])` (deterministic) → `generate_keypair()` → `TestSigner::new(kp.private)`.
- Sends the message from `B` to `A`.
- Asserts the receiver's `received_messages()` snapshot contains the delivery (for `valid_signature_message_retained`) or does NOT contain it (for the three tamper / mismatch scenarios).

What you can verify manually at the same time: pipe `cargo test` output through `tail` or open the test runner's log capture, and you'll see one `event="message_dropped" cause="invalid_signature"` info-level entry per rejection. The tests themselves do NOT assert on log content (FR-014's convention); the log entries are operator UX for inspecting drops manually.

## 3 — Run the multi-publisher integration test (003 US2)

```sh
cargo test --test multi_publisher
```

Expected:

```text
running 3 tests
test three_publishers_all_accepted ... ok
test mismatched_publisher_id_rejected ... ok
test interleaved_50_messages_5_publishers ... ok

test result: ok. 3 passed; 0 failed; 0 ignored
```

The `interleaved_50_messages_5_publishers` test demonstrates the protocol's core multi-publisher property: the receiver uses each message's *own* `publisher_id` to select the verification key, not a single per-node key. Verifier dispatch is keyed off the envelope, per FR-013.

## 4 — Run the filter-composition integration test (003 US3 / SC-003)

```sh
cargo test --test filter_composition
```

Expected:

```text
running 4 tests
test valid_on_topic_message_appears_in_snapshot ... ok
test valid_off_topic_message_dropped_with_cause_topic_not_subscribed ... ok
test invalid_on_topic_message_dropped_with_cause_invalid_signature ... ok
test invalid_off_topic_message_dropped_with_cause_topic_not_subscribed ... ok

test result: ok. 4 passed; 0 failed; 0 ignored
```

US3 AS-5 (no legacy `event = "topic_drop"` in the log stream after the rename) is operator-UX-only per FR-014's tightened tests-don't-check-logs convention — there is no Rust test for it. Verify SC-007's rename atomicity manually by running `grep -r "topic_drop" src/` (should return zero matches in production code) or by running the demonstration and confirming the log stream emits only `event = "message_dropped"` entries. T028's polish-phase agent-run grep covers the same verification at the end of `/speckit-implement`.

The fourth test (`invalid_off_topic_message_dropped_with_cause_topic_not_subscribed`) verifies FR-013's topic-filter-first ordering: even though the message has both an invalid signature AND a topic not in the subscription set, the receive task drops it with `cause = "topic_not_subscribed"` because the topic filter rejects before the verifier runs. The invalid signature is never observed for this message.

## 5 — Run the mock-crypto reproducibility test (003 US4 / SC-005)

```sh
cargo test --test mock_crypto_repro
```

Expected:

```text
running 6 tests
test same_seed_yields_byte_identical_keypair_sequences ... ok
test different_seeds_yield_differing_keypairs ... ok
test derive_public_invariant_holds_on_generated_keypairs ... ok
test test_verifier_accepts_test_signer_signatures ... ok
test test_verifier_rejects_keys_without_public_suffix ... ok
test signature_binding_proptest ... ok

test result: ok. 6 passed; 0 failed; 0 ignored
```

The sixth test is a `proptest`-based property check (research.md §8): for any `(seed, msg)` pair, the verifier accepts the signer's signature on `msg` under the matching public key, and rejects any modified `(key, msg, sig)` triple. The property check exercises the signature-binding invariant per the Constitution's Engineering Standards rule on property-based testing.

## 6 — Re-run the inherited 001 / 002 tests

```sh
cargo test
```

All previous tests (`two_node_ping`, `n_node_graph`, `topic_filter`, `topic_runtime`, `config_loading`) pass under the new envelope shape. The migration is mechanical — every test that constructs a `Message` now uses the `tests/common/mod.rs::build_signed_message_simple` helper which fills in sensible defaults (sequence=0, parent_hash=None, timestamp=fixed) and signs with a shared test-only signer/verifier pair.

If you scan the diff for `tests/two_node_ping.rs`, `tests/n_node_graph.rs`, etc., you should see exactly one kind of change: every `Message { topic, payload }` construction is replaced by `build_signed_message_simple(&signer, topic, payload)`. The receive-side assertions (snapshot contains / does not contain) are unchanged.

## 7 — Tour of the new types

A signed message can be constructed by hand in a few lines (per ADR 0010 — no placeholder-signature dance, because `signed_bytes` lives on `PlainMessage`):

```rust
use pubsub_node::{MockCryptoScheme, TestSigner, TestVerifier, Signer};
use pubsub_node::{Message, SignedMessage, PlainMessage, MessagePayload, PublisherId, Timestamp};

// 1. Build a scheme with a known seed (reproducible across runs).
let mut scheme = MockCryptoScheme::with_seed([0u8; 32]);

// 2. Generate a keypair from the scheme.
let kp = scheme.generate_keypair();

// 3. Wrap the private half in a Signer.
let signer = TestSigner::new(kp.private);

// 4. Construct the PlainMessage (the signed-over content).
let topic = "my-topic".parse().unwrap();
let plain = PlainMessage {
    topic,
    publisher_id: PublisherId::from(kp.public),  // kp.public matches signer's derived public_key
    parent_hash: None,                            // first message on this topic
    sequence: 0,
    timestamp: Timestamp::from_millis(0),
    payload: MessagePayload::Ping(42),
};

// 5. Sign the canonical bytes and assemble.
let signature = signer.sign(&plain.signed_bytes());
let signed = SignedMessage { plain, signature };
let msg = Message::Signed(signed);
```

Every test in 003 follows this pattern (compressed into the `build_signed_message` / `build_signed_message_simple` helper, which returns a fully-wrapped `Message::Signed(SignedMessage { plain, signature })` value).

A receiver-side Node is constructed with an `Arc<dyn Verifier>`:

```rust
use std::sync::Arc;
use pubsub_node::{Node, TestVerifier, Verifier};

let verifier: Arc<dyn Verifier> = Arc::new(TestVerifier);
let node = Node::new(self_id, config, subscriptions, network, verifier).await?;
```

`TestVerifier` is stateless — a single `Arc` can be shared across every Node in a test (or every Node in production at this prototype-stage iteration).

## 8 — Inspect the new types in your editor

The new types live at:

- `src/crypto/mod.rs` — `PublicKey`, `PrivateKey`, `Signature`, `MessageHash`, `Timestamp`, `VerifyError`, `Signer` trait, `Verifier` trait.
- `src/crypto/mock.rs` — `MockCryptoScheme`, `KeyPair`, `TestSigner`, `TestVerifier`, `derive_public`, `PUBLIC_SUFFIX`.
- `src/message.rs` — `Message` (#[non_exhaustive] enum, per ADR 0010), `SignedMessage`, `PlainMessage`, `PublisherId`, the preserved `MessagePayload`, and the `PlainMessage::signed_bytes` method whose rustdoc is treated as part of the protocol surface (per FR-010 + IMPLEMENTATION_NOTES.md N-004).
- `src/network.rs` — the 001-era `Envelope` routing wrapper is renamed to `RoutingFrame` (per ADR 0010), freeing the term "envelope" for prose-level use matching the synthesis §2.3.

Run `cargo doc --open --no-deps` to read the generated HTML. The `crypto::mock` module's docs carry a prominent **"MOCK — not unforgeable"** warning paragraph at the module, scheme, signer, and verifier levels per SC-006.

## 9 — What 003 does NOT do (and where the deferred bits live)

- **No chain-integrity validation**. A message with a `parent_hash` that doesn't link to anything in the arrival log is accepted (only its signature is checked). Gap detection, equivocation reporting, and sequence-monotonicity checks are all deferred to features 008 / 012 per `../IMPLEMENTATION_NOTES.md` N-003.
- **No publisher-key registry**. A message signed by an unauthorized publisher (cryptographically valid signature, but the publisher isn't supposed to publish on this topic) is accepted. Authorization checks layer on top of signature verification in feature 008 (mock registry) and 012 (real on-chain registry).
- **No replay detection**. A byte-identical signed envelope received twice is accepted twice (appears twice in the snapshot). Replay detection consumes the same chain state as integrity validation and is similarly deferred.
- **No real cryptographic authenticity**. The mock's `TestVerifier` can derive a `PrivateKey` from any `PublicKey` it sees (by stripping the `_public` suffix), so anyone with read access to the mock's source can forge a signature. Real Ed25519 lands in feature 011.
- **No new CLI flag**. The binary uses `Arc::new(TestVerifier)` at the prototype-stage iteration per FR-017. When real Ed25519 lands in 011, a choice surfaces about how operators select the verifier impl; in 003 it is hard-coded.
- **No new TOML field**. The 002-era `subscribed_topics` field, the `[[peers]]` entries, and the strict-unknown-field policy all remain unchanged (FR-017).

## 10 — Where to look for protocol-level deeper context

- `spec.md` — the normative requirements (FR-001 through FR-020), the four user stories, and the Q1–Q6 clarifications.
- `data-model.md` — every new entity (PublicKey, PrivateKey, Signature, MessageHash, Timestamp, PublisherId, VerifyError, Signer, Verifier, MockCryptoScheme, KeyPair, TestSigner, TestVerifier) with field-by-field traceability to FRs.
- `contracts/library-api.md` — the public Rust API delta this feature adds.
- `research.md` — plan-level decisions (file layout, helper-function shape, dep version pins, migration strategy, property-test framework choice) with Decision / Rationale / Alternatives entries.
- `../../docs/decisions/0009-crypto-trait-shape.md` — the ADR for the crypto trait shape (the rejected associated-types alternative, the concrete-type decision, the asymmetric-shaped mock construction).
- `../../docs/decisions/0010-protocol-message-type-hierarchy.md` — the ADR for the `Message` enum + `SignedMessage` / `PlainMessage` split + the 001 `Envelope` → `RoutingFrame` rename + the `MessageHash::of(&PlainMessage)` content-anchored hash choice.
- `../IMPLEMENTATION_NOTES.md` — N-003 (deferred chain integrity, revisit at 008 / 012), N-004 (deferred CBOR canonical, revisit at 009), N-005 (`MessageHash` input revisit when downstream features first consume the hash operationally).
- `../../docs/staged-design-synthesis.md` §2.3 — the original envelope-shape rationale and the three protocol properties it enables (sequence ordering, parent-hash chain extension, signature scope binding).

If you change anything in `spec.md` that affects this walkthrough (FRs governing the envelope, the Signer / Verifier trait surfaces, the `MockCryptoScheme` factory API, the `message_dropped` event shape, or the test names referenced in this quickstart), update `quickstart.md` in the same commit per SC-004's spec-quickstart cohesion rule.

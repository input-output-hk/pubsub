# Data Model: Message Envelope + Mock Crypto

**Feature**: 003-message-envelope-mock-crypto

**Created**: 2026-06-03

**Purpose**: enumerate the entities introduced or extended by 003, with fields, derives, validation rules, state-transition semantics, relationships, and an FR cross-reference. Entities unchanged by 003 (`PeerId`, `TopicId`, `MessagePayload` enum shape, `PeerDescriptor`, `BasicPeerDescriptor`, `Network`, `NetworkHandle`, `InMemoryNetwork`, `Envelope` routing wrapper, `NodeError`, `NetworkError`, `ConfigError`, `SubscribeOutcome`, `UnsubscribeOutcome`, `ReceivedDelivery`'s containing-Vec semantics, `NodeConfig` shape) are not duplicated here — the canonical references are `../001-minimal-node-scaffold/data-model.md` and `../002-topic-subscription-filtering/data-model.md`.

A reminder on terminology that the spec's Assumptions section formalises: throughout this file, "envelope" / "message envelope" refers to the §2.3 shape carried on `pubsub_node::Message` (extended in §11 below). The 001-era `Envelope { from: PeerId, message: Message }` Rust type is the network-layer routing wrapper and is **not** what this feature is about.

---

## 1. `PublicKey` — new entity

**Source**: `src/crypto/mod.rs` (new file, see plan.md §"Project Structure").

**Definition**: opaque byte-newtype around an owned `Vec<u8>`.

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PublicKey(Vec<u8>);
```

**Validation rules**: none at the type level. The bytes are opaque; concrete `Signer` / `Verifier` impls choose how to interpret them (per FR-003 + ADR 0009). The mock impl pair (§13 / §14 below) interprets the bytes as `private_bytes || PUBLIC_SUFFIX` (`b"_public"`); the future Ed25519 impl will interpret them as a 32-byte curve-encoded key.

**Construction surface**:

- `pub fn new(bytes: Vec<u8>) -> Self` — direct constructor for tests that need to fabricate keys with specific bytes (e.g., US1 AS-4, US4 AS-5).
- `impl From<Vec<u8>> for PublicKey` — same as `new`; idiomatic Rust.

**Accessors**:

- `pub fn as_bytes(&self) -> &[u8]` — borrows the underlying bytes for hashing / suffix-stripping / cross-impl comparison.
- `impl Display for PublicKey` — full lowercase hex of the underlying bytes (FR-003 Display bullet; resolved by Q4 in clarify).

**FR trace**: FR-003 (newtype, derives, Display).

## 2. `PrivateKey` — new entity

**Source**: `src/crypto/mod.rs`.

**Definition**: opaque byte-newtype around an owned `Vec<u8>`, secret-discipline-shaped.

```rust
#[derive(Clone, Eq, PartialEq)]
pub struct PrivateKey(Vec<u8>);
// NOTE: deliberately no derived `Debug`, no `Hash`, no `Display`.
impl std::fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PrivateKey([REDACTED])")
    }
}
```

**Validation rules**: none at the type level. Mock impl uses 32 random bytes per FR-008.

**Construction surface**:

- `pub fn new(bytes: Vec<u8>) -> Self` — direct constructor for tests (analogue of `PublicKey::new`). Tests that fabricate a `KeyPair` without going through `MockCryptoScheme::generate_keypair` use this.

**Accessors**:

- `pub fn as_bytes(&self) -> &[u8]` — borrows the underlying bytes. Used by `TestSigner::sign` to compute `sha256(private || msg)`.

**Secret-discipline-shaped properties** (FR-003 + clarification Q3 refinement):

- **No derived `Debug`**: the hand-written impl above redacts the byte content. Panics and test-failure messages including `PrivateKey` print `PrivateKey([REDACTED])`, never the bytes.
- **No `Hash`**: secrets in `HashMap` keys is a footgun (correlation / side-channel concerns). Even though the mock's secret is recoverable from the public, the discipline carries forward to feature 011's Ed25519 swap.
- **No `Display`**: there is no operator-facing use case for printing a private key. Production code that tries `format!("{}", private_key)` fails to compile.

**FR trace**: FR-003 (newtype, derives, redacting Debug, no Display).

## 3. `Signature` — new entity

**Source**: `src/crypto/mod.rs`.

**Definition**: opaque byte-newtype around an owned `Vec<u8>`.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Signature(Vec<u8>);
// Note: deliberately no `Hash` derive (FR-003 + clarification Q3).
```

**Validation rules**: none at the type level. Mock impl stores 32 bytes (SHA-256 output); future Ed25519 impl will store 64 bytes (Ed25519 signature format).

**Construction surface**:

- `pub fn new(bytes: Vec<u8>) -> Self` — direct constructor.
- `pub fn placeholder() -> Self` — returns an empty `Signature(Vec::new())` used by the test-support helper `build_signed_message` during the construct-then-sign workflow (per FR-010's last sentence: "construct the message with a placeholder signature, compute signed_bytes, sign those bytes, replace the placeholder with the produced signature"). The placeholder value is arbitrary — `signed_bytes` excludes the signature field, so any value works; `Vec::new()` is the cheapest.

**Accessors**:

- `pub fn as_bytes(&self) -> &[u8]` — borrows the underlying bytes for byte-for-byte comparison during `TestVerifier::verify`.
- `impl Display for Signature` — full lowercase hex (FR-003 Display bullet).

**FR trace**: FR-003 (newtype, derives, no Hash, Display).

## 4. `MessageHash` — new entity

**Source**: `src/crypto/mod.rs`.

**Definition**: fixed-width 32-byte newtype.

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct MessageHash([u8; 32]);
```

**Validation rules**: structural — always 32 bytes by virtue of the array type.

**Construction surface**:

- `pub const ZERO: MessageHash = MessageHash([0u8; 32])` — the absent-`parent_hash` sentinel used by `Message::signed_bytes` encoding (FR-003 + FR-010 + clarification Q1).
- `pub fn of(message: &Message) -> MessageHash` — computes `MessageHash(sha256(message.signed_bytes()).into())`. The canonical hash function — applied to the canonical signing bytes — produces this hash. Used by test fixtures building a chain of messages and (in deferred-future features) by replication / catch-up logic.
- `pub fn new(bytes: [u8; 32]) -> Self` — direct constructor for tests that fabricate hashes with specific bytes.

**Accessors**:

- `pub fn as_bytes(&self) -> &[u8; 32]` — borrows the 32-byte array for use in `signed_bytes` encoding (32 raw bytes for the `parent_hash` field) and for byte-for-byte comparison.
- `impl Display for MessageHash` — full lowercase hex (64 hex chars; FR-003 Display bullet).

**FR trace**: FR-003 (newtype, derives, ZERO constant, Display) + FR-011 (SHA-256 of signed_bytes, `of` constructor).

## 5. `Timestamp` — new entity

**Source**: `src/crypto/mod.rs`. *(Lives in `crypto::` rather than a sibling module because it's used exclusively as an envelope field and bundles naturally with the other crypto-adjacent types; it has no logical home in `peer.rs`, `topic.rs`, or `message.rs`.)*

**Definition**: newtype around `u64` carrying Unix epoch milliseconds.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Timestamp(u64);
```

**Validation rules**: none at the type level — every `u64` is a valid timestamp.

**Construction surface**:

- `pub fn now() -> Self` — resolves the system clock via `SystemTime::now().duration_since(UNIX_EPOCH)`; on the (impossible-in-practice) failure case (system clock before 1970), returns `Timestamp::from_millis(0)`. Used by production publishers (none in 003) and any future publisher CLI.
- `pub fn from_millis(ms: u64) -> Self` — direct constructor used by tests for deterministic timestamps (US1–US4 acceptance scenarios all use this).

**Accessors**:

- `pub fn as_millis(&self) -> u64` — borrows the underlying value for `signed_bytes` encoding (8 bytes big-endian per FR-010) and for any future timestamp-based logic.

**Semantic note**: advisory only (synthesis §2.3 + spec Assumptions). The receive-path verifier does not interpret the timestamp; its only role is to be included in the canonical signing bytes so the publisher's signature commits to a publication time the publisher chose to assert.

**FR trace**: FR-003 (newtype, derives, constructors).

## 6. `PublisherId` — new entity

**Source**: `src/message.rs` (alongside the extended `Message` struct).

**Definition**: thin newtype around `PublicKey`, distinct at the type level from `PeerId`.

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PublisherId(PublicKey);
```

**Validation rules**: inherits `PublicKey`'s (none at the type level).

**Construction surface**:

- `pub fn new(public: PublicKey) -> Self` — direct constructor.
- `impl From<PublicKey> for PublisherId` — same as `new`; idiomatic Rust.

**Accessors**:

- `pub fn as_public_key(&self) -> &PublicKey` — borrows the inner `PublicKey` for verifier dispatching (the receive task calls `self.verifier.verify(msg.publisher_id.as_public_key(), …)`).
- `impl Display for PublisherId` — delegates to the inner `PublicKey`'s `Display` (FR-003 + Q4).

**Role distinction** (FR-002): `PublisherId` identifies the entity whose private key produced the envelope's signature (the message originator). `PeerId` identifies the network neighbor that forwarded the message. Even when both happen to wrap the same `PublicKey` byte representation under a future iteration, the compiler distinguishes them; accidental cross-role use is a compile error.

**FR trace**: FR-002 (newtype, derives, role distinction).

## 7. `VerifyError` — new entity

**Source**: `src/crypto/mod.rs`.

**Definition**: typed enum returned by `Verifier::verify`.

```rust
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("invalid signature")]
    Invalid,
}
```

**Variants (v1)**: only `Invalid`. Future variants (`KeyFormatInvalid`, `AlgorithmMismatch`, etc.) anticipated per FR-014's anticipation; per Q5 the enum is `#[non_exhaustive]` so adding variants in 011+ is a non-breaking change for downstream callers.

**Implementations**: `Debug` (derived), `Display` (via `thiserror::Error`), `std::error::Error` (via `thiserror::Error`). Per the crate-wide typed-errors convention (ADR 0005 lineage).

**Semantic note**: 003's receive task treats every `Err` variant uniformly — drop the message with `cause = "invalid_signature"`. Future verifier impls that emit more specific variants (in 011+) can refine the drop-event's `cause` field at that point; for 003 the spec collapses every error variant to one cause value (FR-013).

**FR trace**: FR-005 (typed enum, variant set, `#[non_exhaustive]`).

## 8. `Signer` trait — new entity

**Source**: `src/crypto/mod.rs`.

**Definition**:

```rust
pub trait Signer: Send + Sync {
    fn public_key(&self) -> PublicKey;
    fn sign(&self, msg: &[u8]) -> Signature;
}
```

**Properties**:

- `Send + Sync` (FR-004) — concrete impls must be threadable. Future Ed25519Signer satisfies this; TestSigner satisfies it trivially (no interior mutability).
- No type parameters and no associated types (FR-004 + ADR 0009) — `msg` is a raw `&[u8]`; the return type is the concrete `Signature` newtype.
- Infallible (`sign` returns `Signature`, not `Result<Signature, _>`). Real Ed25519 signing is infallible; the trait does not need to anticipate a fallible variant in 003.

**Consumers in 003**: test-support code (`tests/common/mod.rs::build_signed_message`); no production-code consumer because `Node` does not sign (FR-012).

**FR trace**: FR-004.

## 9. `Verifier` trait — new entity

**Source**: `src/crypto/mod.rs`.

**Definition**:

```rust
pub trait Verifier: Send + Sync {
    fn verify(&self, key: &PublicKey, msg: &[u8], sig: &Signature) -> Result<(), VerifyError>;
}
```

**Properties**:

- `Send + Sync` (FR-005) — required for storage as `Arc<dyn Verifier>` on `Node`.
- No type parameters and no associated types (FR-005 + ADR 0009).
- Synchronous (returns `Result<(), VerifyError>` directly, not a `Future`). The receive task calls verify without inserting an `await` point (FR-020).

**Consumers in 003**: `Node` (stores `Arc<dyn Verifier>` and calls verify on each inbound message that passes the topic filter — FR-013).

**FR trace**: FR-005 + FR-019 + FR-020.

## 10. `MockCryptoScheme` — new entity

**Source**: `src/crypto/mock.rs` (new file).

**Definition**: factory struct that owns a seeded RNG.

```rust
pub struct MockCryptoScheme {
    rng: rand_chacha::ChaCha20Rng,
}
```

**Construction surface**:

- `pub fn with_seed(seed: [u8; 32]) -> Self` — constructs the scheme with a deterministic ChaCha20 PRNG seeded by the caller-supplied bytes (FR-007). Tests that depend on key bytes use this to pin reproducibility.
- `pub fn from_entropy() -> Self` — constructs the scheme with a PRNG seeded from OS entropy (FR-007). Used by tests that want non-determinism and (eventually) by any future publisher CLI that wants a fresh key on first run.

**Methods**:

- `pub fn generate_keypair(&mut self) -> KeyPair` — draws 32 fresh random bytes from the internal RNG for the private key, derives the public key by appending `PUBLIC_SUFFIX`, returns a `KeyPair { public, private }` (FR-008).
- `pub fn signer(&self, private: PrivateKey) -> TestSigner` — constructs a `TestSigner` wrapping the given private key. Stateless on the scheme — does not advance the RNG.
- `pub fn verifier(&self) -> TestVerifier` — returns a fresh `TestVerifier`. Stateless on the scheme; the returned verifier carries no key, only the algorithm.

**Rustdoc**: the module-level rustdoc on `crypto::mock` plus the struct-level rustdoc on `MockCryptoScheme` carry a prominent **"MOCK — not unforgeable"** warning paragraph per SC-006.

**FR trace**: FR-006 + FR-007 + FR-008.

## 11. `KeyPair` — new entity

**Source**: `src/crypto/mock.rs`.

**Definition**: value-type struct returned by `MockCryptoScheme::generate_keypair`.

```rust
pub struct KeyPair {
    pub public: PublicKey,
    pub private: PrivateKey,
}
```

**Properties**:

- Public fields by design — tests destructure freely (`let KeyPair { public, private } = scheme.generate_keypair();`).
- Implements `Clone` (inherits from the field types). Does NOT implement `Hash` or `Display` (because `PrivateKey` doesn't). Does NOT implement `Debug` derive (because `PrivateKey` has a hand-written redacting `Debug`); `KeyPair` derives `Debug` via the redacted `PrivateKey::Debug`, which means `format!("{:?}", kp)` prints `KeyPair { public: PublicKey([…]), private: PrivateKey([REDACTED]) }`.

**Invariant** (FR-008 + FR-009): `derive_public(&kp.private) == kp.public` for any `KeyPair` produced by `MockCryptoScheme::generate_keypair`. US4 AS-3 pins this invariant as an acceptance scenario.

**FR trace**: FR-006 + FR-008.

## 12. `TestSigner` — new entity

**Source**: `src/crypto/mock.rs`.

**Definition**:

```rust
pub struct TestSigner {
    private: PrivateKey,
}

impl Signer for TestSigner {
    fn public_key(&self) -> PublicKey {
        derive_public(&self.private)
    }

    fn sign(&self, msg: &[u8]) -> Signature {
        let mut input = self.private.as_bytes().to_vec();
        input.extend_from_slice(msg);
        Signature::new(sha2::Sha256::digest(&input).to_vec())
    }
}
```

**Construction surface**:

- `pub fn new(private: PrivateKey) -> Self` — constructs the signer wrapping the given private key. Used directly by tests that have a `PrivateKey` (often from `MockCryptoScheme::generate_keypair`'s `KeyPair`).

**Rustdoc**: carries a prominent **"MOCK — not unforgeable"** warning per SC-006.

**FR trace**: FR-006 + FR-009.

## 13. `TestVerifier` — new entity

**Source**: `src/crypto/mock.rs`.

**Definition**: a zero-sized unit struct.

```rust
pub struct TestVerifier;

impl Verifier for TestVerifier {
    fn verify(&self, key: &PublicKey, msg: &[u8], sig: &Signature) -> Result<(), VerifyError> {
        let stripped = key
            .as_bytes()
            .strip_suffix(PUBLIC_SUFFIX)
            .ok_or(VerifyError::Invalid)?;
        let mut input = stripped.to_vec();
        input.extend_from_slice(msg);
        let expected = sha2::Sha256::digest(&input);
        if sig.as_bytes() == expected.as_slice() {
            Ok(())
        } else {
            Err(VerifyError::Invalid)
        }
    }
}
```

**Properties**:

- Stateless (a zero-sized type). Concurrent verification across multiple inbound messages is trivially safe; the verifier never contends with itself.
- The `PUBLIC_SUFFIX` constant is shared with `derive_public` (in the same module); the strip-suffix and append-suffix operations stay byte-symmetric by construction (FR-009).

**Rustdoc**: carries a prominent **"MOCK — not unforgeable"** warning per SC-006.

**FR trace**: FR-006 + FR-009.

## 14. `PUBLIC_SUFFIX` constant — new entity

**Source**: `src/crypto/mock.rs`.

**Definition**:

```rust
pub(crate) const PUBLIC_SUFFIX: &[u8] = b"_public";
```

**Visibility**: `pub(crate)`. Used internally by `derive_public` and by `TestVerifier::verify`. Tests that need to construct a "public key that doesn't end in the suffix" (US4 AS-5) can do so by simply passing arbitrary bytes via `PublicKey::new` — they don't need direct access to the constant.

**FR trace**: FR-009.

## 15. `derive_public` helper — new function

**Source**: `src/crypto/mock.rs`.

**Definition**:

```rust
pub fn derive_public(private: &PrivateKey) -> PublicKey {
    let mut bytes = private.as_bytes().to_vec();
    bytes.extend_from_slice(PUBLIC_SUFFIX);
    PublicKey::new(bytes)
}
```

**Visibility**: `pub` — exposed at `pubsub_node::crypto::mock::derive_public` so US4 AS-3 can call it directly to assert the invariant `derive_public(&keypair.private) == keypair.public`.

**Property**: deterministic — same input always yields the same output. The function is total (no failure mode).

**FR trace**: FR-009.

## 16. `Message` — reshaped entity (was struct in 002; now enum per ADR 0010)

**Source**: `src/message.rs` (reshaped).

**Definition** (post-003):

```rust
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Message {
    Signed(SignedMessage),
    // Future variants (per ADR 0010):
    //   ConnectionHello(ConnectionHello),     // 004
    //   ConnectionAccept(ConnectionAccept),   // 004
    //   PeerSample(PeerSampleMessage),        // 005 / 010
    //   CatchUpRequest(CatchUpRequest),       // deferred replication
    //   CatchUpBatch(CatchUpBatch),           // deferred replication
    //   …
}
```

**Migration from 002**: the 002-era `Message { topic, payload }` struct shape is replaced by the enum above. Every 001 / 002 construction site (every test fixture, every `tests/common/mod.rs::build_ping`-style helper) updates to `Message::Signed(SignedMessage { plain: PlainMessage { topic, publisher_id, parent_hash: None, sequence: 0, timestamp: Timestamp::from_millis(0), payload: MessagePayload::Ping(N) }, signature })` — typically compressed via the `build_signed_message_simple` helper (research.md §4 + §6). The migration is a single coherent commit per the green-checkpoints rule (research.md §6 step 4).

**Derives**: `Clone, Debug, Eq, PartialEq` (preserved from 002). No `Hash` derive — `Message` isn't keyed in a HashMap.

**`#[non_exhaustive]`**: future variants for 004 / 005 / 008 / 010 / deferred replication are non-breaking additions. Pattern-matches in external code MUST include a catch-all `_ =>` arm; internal receive-task matches in 003 cover only the `Signed` arm because that's the sole variant the receive task can handle.

**FR trace**: FR-001 (enum + variant set) + ADR 0010 (structural decision).

## 16a. `SignedMessage` — new entity (introduced by 003 per ADR 0010)

**Source**: `src/message.rs`.

**Definition**:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedMessage {
    pub plain: PlainMessage,
    pub signature: Signature,
}
```

**Role**: wraps a `PlainMessage` (the signed-over content) and a `Signature` together to form a complete signed dissemination message. Carried as `Message::Signed(SignedMessage)` at the protocol layer; corresponds to the staged-design-synthesis §2.3 "envelope" (signature included) in prose.

**Derives**: `Clone, Debug, Eq, PartialEq`. Public fields by design — callers destructure freely.

**Future methods** (not in 003 but anticipated): `verify(&self, verifier: &impl Verifier) -> Result<(), VerifyError>` thin helper, `message_hash(&self) -> MessageHash` (delegates to `MessageHash::of(&self.plain)`). Adding these is non-breaking and can happen incrementally.

**FR trace**: FR-001 (struct shape) + ADR 0010.

## 16b. `PlainMessage` — new entity (introduced by 003 per ADR 0010)

**Source**: `src/message.rs`.

**Definition**:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlainMessage {
    pub topic: TopicId,
    pub publisher_id: PublisherId,
    pub parent_hash: Option<MessageHash>,
    pub sequence: u64,
    pub timestamp: Timestamp,
    pub payload: MessagePayload,
}
```

**Role**: carries the §2.3 envelope fields **excluding the signature**. The canonical-encoding seam (`signed_bytes`) lives here, which means the signing workflow never needs a placeholder signature value — `PlainMessage` is constructed in full, its bytes are signed, and the resulting signature is wrapped alongside it into a `SignedMessage`.

**Derives**: `Clone, Debug, Eq, PartialEq`. Public fields by design.

**Methods**:

- `pub fn signed_bytes(&self) -> Vec<u8>` — the single canonical-encoding seam. Produces the hand-rolled length-prefixed byte layout described in FR-010. The rustdoc on this method documents the byte layout in full (field order, widths, endianness, the `MessageHash::ZERO` sentinel for absent `parent_hash`, the `MessagePayload` variant tags) — treated as part of the protocol surface per FR-010 + IMPLEMENTATION_NOTES.md N-004.

**Signing workflow** (FR-010 + helper from research.md §4, post-ADR 0010 — no placeholder):

1. Construct `PlainMessage { topic, publisher_id, parent_hash, sequence, timestamp, payload }`.
2. Compute `plain.signed_bytes()` (signature field doesn't exist on this type — no placeholder needed).
3. Sign: `let signature = signer.sign(&plain.signed_bytes());`.
4. Assemble: `Message::Signed(SignedMessage { plain, signature })`.

**FR trace**: FR-001 (field set) + FR-010 (signed_bytes method, byte layout) + ADR 0010 (structural rationale for the PlainMessage / SignedMessage split).

## 16c. `MessagePayload` — preserved entity (002, unchanged in 003)

**Source**: `src/message.rs` (002-era declaration, carried unchanged).

**Definition**:

```rust
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MessagePayload {
    Ping(u64),
}
```

**Role in 003**: lives as a field of `PlainMessage` (the dissemination application content). The 002 anticipation — "different application content sharing one envelope shape" — is preserved at this layer; `Ping(u64)` remains the sole variant for 003 (inherited from 001's connection-probe semantics), and future application content variants (governance updates, DeFi intents, SPO alerts, …) extend this enum without touching the envelope structure.

**Reasoning for preservation rather than rename to `Payload`**: discussed during ADR 0010's drafting. The 002-era name is intelligible; renaming to `Payload` would touch every 001 / 002 test and every spec artifact, with negligible clarity benefit. Skipping.

**FR trace**: FR-010 (variant-tag stability and encoding shape) + 002 FR-001 (original introduction).

## 16d. `RoutingFrame` — renamed entity (was `Envelope` in 001; renamed by ADR 0010 in 003)

**Source**: `src/network.rs` (renamed).

**Definition**:

```rust
pub struct RoutingFrame {
    pub from: PeerId,
    pub message: Message,
}
```

**Role**: the network-layer routing wrapper carrying a `Message` from a sender's `PeerId` to a receiver's mailbox. The InMemoryNetwork's tokio mpsc channel transports `RoutingFrame` values per the existing 001 / 002 contract; behavior is unchanged from 001.

**Rename rationale (ADR 0010)**: freeing the name "envelope" for prose-level use matching the staged-design-synthesis §2.3 (where "envelope" = the whole signed message). The `RoutingFrame` name more precisely describes the type's job (routing a Message between Nodes).

**Migration**: a single struct rename in `src/network.rs` plus a search-and-replace pass across any test that pattern-matches on the type name. Mechanical, single-commit.

**FR trace**: FR-001 (rename context) + ADR 0010.

## 17. `Node` — extended entity

**Source**: `src/node.rs` (extended).

**Delta**: gains an `Arc<dyn Verifier>` field at construction; the receive task gains a signature-verification step **after** the existing topic-subscription filter (per FR-013 + Q6).

**New constructor parameter** (research.md §3 — appended at end):

```rust
pub async fn new<N: Network>(
    self_id: PeerId,
    config: NodeConfig,
    initial_subscriptions: HashSet<TopicId>,
    network: Arc<N>,
    verifier: Arc<dyn Verifier>,
) -> Result<Self, NodeError>
```

**New field**: `verifier: Arc<dyn Verifier>` (FR-012). Stored on the Node for its lifetime. No `Signer` field is added (signing is caller-side per FR-012 + ADR 0009).

**Receive-task pipeline** (FR-013 + Q6, with the ADR 0010 pattern-match wrapper):

```text
recv RoutingFrame { from, message }
  └─ match message {
       Message::Signed(signed) →
         topic-filter check on signed.plain.topic
           ├─ [miss] drop + emit message_dropped/topic_not_subscribed (FR-015)
           └─ [hit]  signature verification:
                       verifier.verify(&signed.plain.publisher_id.as_public_key(),
                                       &signed.plain.signed_bytes(),
                                       &signed.signature)
                       ├─ [Ok]  snapshot append (the full Message::Signed(signed) value;
                       │         002 FR-006 contract preserved transitively)
                       └─ [Err] drop + emit message_dropped/invalid_signature (FR-014)
       // Future variants of Message get their own match arms when those features land.
     }
```

**Linearizability extension** (FR-019): the verification step is stateless and synchronous (no `await`). The existing 002 FR-015 linearizability contract (subscription-set mutators, snapshot getter, receive-path filter check) extends unchanged to cover the verification step — verification's behavior is a pure function of `(verifier, key, msg, sig)`, so it has no concurrency hazard of its own.

**`Node::send` shape**: unchanged from 002 (takes an already-built `Message` per FR-012). The 002-era `Node::send(to, message)` signature compiles unchanged after the Message migration; the only delta is that the `Message` argument is now a 7-field envelope rather than a 2-field one.

**FR trace**: FR-012 (verifier field, no Signer field, send unchanged) + FR-013 (receive-task ordering + verification call site) + FR-019 (linearizability extension) + FR-020 (confined to receive task, synchronous verify).

## 18. Tracing event shape — extended observability surface

**Source**: `src/node.rs` (the receive task's drop emitters).

**Delta**: both filter-drop emissions in the receive task use a unified `event = "message_dropped"` + `cause` field shape, distinguished by cause value. This replaces 002's `event = "topic_drop"` (FR-015 rename, same-commit-as-new-emitter).

**Schema**:

```text
event = "message_dropped"  (stable string, same value for every drop cause)
cause = "<snake_case_reason>"  (per-cause discriminator)
self_id    (receiving Node's peer id, %Display)
from       (forwarding peer's logical peer id, %Display)
topic      (envelope's topic, %Display)
publisher_id (envelope's publisher id, %Display — included only for cause = "invalid_signature")
```

**Cause values established by 003**:

- `topic_not_subscribed` — message arrived for a topic not in the subscription set (the 002 `topic_drop` event renamed under the new convention; FR-015).
- `invalid_signature` — message signature failed `Verifier::verify` (FR-014).

**Future cause values** (post-N-003 / per the saved drop-event convention): chain inconsistencies, format errors, expired messages — each adds a new `cause` value, NOT a new event name.

**Test-anchored contract**: NONE. Tests do not assert on log content. The log entry is operator UX only (FR-014's convention reminder).

**FR trace**: FR-014 (invalid_signature emitter) + FR-015 (002 rename).

## 19. FR cross-reference matrix

| FR | Entity / Surface | Where it lives |
|---|---|---|
| FR-001 (Message enum + SignedMessage + PlainMessage + MessagePayload + RoutingFrame rename) | `Message` / `SignedMessage` / `PlainMessage` / `MessagePayload` / `RoutingFrame` | §§16, 16a, 16b, 16c, 16d |
| FR-002 (PublisherId newtype) | `PublisherId` | §6 |
| FR-003 (newtype types + Display) | `PublicKey` / `PrivateKey` / `Signature` / `MessageHash` / `Timestamp` | §§1, 2, 3, 4, 5 |
| FR-004 (Signer trait) | `Signer` | §8 |
| FR-005 (Verifier trait, VerifyError) | `Verifier` / `VerifyError` | §§7, 9 |
| FR-006 (crypto::mock module) | `MockCryptoScheme` / `KeyPair` / `TestSigner` / `TestVerifier` | §§10, 11, 12, 13 |
| FR-007 (with_seed / from_entropy) | `MockCryptoScheme::with_seed` / `::from_entropy` | §10 |
| FR-008 (generate_keypair) | `MockCryptoScheme::generate_keypair` | §10 |
| FR-009 (mock algorithm) | `TestSigner::sign` + `TestVerifier::verify` + `PUBLIC_SUFFIX` + `derive_public` | §§12, 13, 14, 15 |
| FR-010 (signed_bytes seam on PlainMessage) | `PlainMessage::signed_bytes` + payload-tag mechanism | §16b + research.md §7 |
| FR-011 (MessageHash::of consumes PlainMessage) | `MessageHash::of` constructor | §4 (with content-anchored rationale referenced from §16b + ADR 0010) |
| FR-012 (Node verifier field) | `Node` (extended) | §17 |
| FR-013 (receive-task pattern-match + ordering) | `Node` (extended) | §17 |
| FR-014 (invalid_signature event) | tracing event shape | §18 |
| FR-015 (002 topic_drop rename) | tracing event shape | §18 |
| FR-016 (signature-only validation) | `Node` (extended) | §17 (negative claim — no chain-integrity logic added) |
| FR-017 (TOML unchanged) | inherited from 002 | (no entity) |
| FR-018 (011 swap-readiness, trait surfaces + Message enum shape preserved) | Signer / Verifier / type newtypes / Message enum | §§1, 2, 3, 8, 9, 16 |
| FR-019 (linearizability) | `Node` (extended) | §17 |
| FR-020 (receive-task confinement + pattern-match wrapper) | `Node` (extended) | §17 |

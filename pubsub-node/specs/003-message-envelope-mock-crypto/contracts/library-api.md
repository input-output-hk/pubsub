# Library API Contract — 003 Deltas

**Feature**: 003-message-envelope-mock-crypto

**Source of truth**: `src/lib.rs` re-exports + the per-module public surface

**Spec trace**: FR-001, FR-002, FR-003, FR-004, FR-005, FR-006, FR-007, FR-008, FR-009, FR-010, FR-011, FR-012, FR-013, FR-014, FR-015, FR-018, FR-019, FR-020 (full matrix in `data-model.md §19`)

This contract documents **only what 003 adds or changes**. The 001 contract at `../001-minimal-node-scaffold/contracts/library-api.md` and the 002 contract at `../002-topic-subscription-filtering/contracts/library-api.md` remain the canonical references for everything else (`PeerId`, `TopicId`, `PeerDescriptor`, `Network`, `NetworkHandle`, `InMemoryNetwork`, `ReceivedDelivery`, `MessagePayload`, `NodeConfig`, `SubscribeOutcome`, `UnsubscribeOutcome`, all error types other than `crypto::VerifyError`). Items unchanged by 003 are not re-described here.

---

## Re-exports from `pubsub_node` — additions and reshapings

```rust
// new in 003 — crypto types and traits (re-exported from src/crypto/mod.rs):
pub use crypto::{
    PublicKey, PrivateKey, Signature, MessageHash, Timestamp,
    VerifyError, Signer, Verifier,
};

// new in 003 — mock crypto factory + impls (re-exported from src/crypto/mock.rs):
pub use crypto::mock::{
    MockCryptoScheme, KeyPair, TestSigner, TestVerifier,
    derive_public, // public so US4 AS-3 can call it directly
    // PUBLIC_SUFFIX is pub(crate); NOT re-exported.
};

// new in 003 — protocol-message type hierarchy (per ADR 0010):
pub use message::{Message, SignedMessage, PlainMessage, PublisherId};
// `Message` is re-exported as before, but its shape has changed from struct
// (002-era `{ topic, payload }`) to `#[non_exhaustive]` enum (sole 003 variant
// `Message::Signed(SignedMessage)`). Downstream pattern-matches on `Message`
// must include a catch-all `_ =>` arm.

// preserved unchanged from 002:
pub use message::MessagePayload;  // #[non_exhaustive] enum, still Ping(u64); now lives inside PlainMessage.

// renamed by ADR 0010 (was 001-era `Envelope`):
pub use network::RoutingFrame;
// The 001-era `pub use network::Envelope;` is removed. Test code and any
// external pattern-matches on the type name update in the same commit.
```

The `crypto::` and `crypto::mock::` paths are also navigable directly (`pubsub_node::crypto::Signer`, `pubsub_node::crypto::mock::MockCryptoScheme`). The flat re-exports above are convenience aliases consistent with 001 / 002's "everything at the crate root" convention; the nested paths are the canonical homes.

## `pubsub_node::crypto` module

### `PublicKey`

| Item | Contract |
|------|----------|
| `pub fn new(bytes: Vec<u8>) -> Self` | Direct constructor; accepts any byte sequence. No length check at construction (impl-defined: 32-byte block from `MockCryptoScheme::generate_keypair`, but the type tolerates other lengths for tests like US4 AS-5 that fabricate keys with arbitrary bytes). |
| `impl From<Vec<u8>> for PublicKey` | Same as `new`. |
| `pub fn as_bytes(&self) -> &[u8]` | Borrows the underlying bytes for hashing, suffix-stripping, comparison. |
| `impl Display for PublicKey` | Full lowercase hex of the underlying bytes (FR-003 Display bullet, resolved by clarify Q4). E.g., bytes `[0x70, 0x75]` render as `"7075"`. Carries forward unchanged when Ed25519's fixed-length 32-byte keys arrive in feature 011. |
| `derive(Clone, Debug, Eq, PartialEq, Hash)` | Per FR-003. Hash supports `HashMap<PublicKey, _>` for future-feature use (publisher registries in 008). |

### `PrivateKey`

| Item | Contract |
|------|----------|
| `pub fn new(bytes: Vec<u8>) -> Self` | Direct constructor; accepts any byte sequence. |
| `pub fn as_bytes(&self) -> &[u8]` | Borrows the underlying bytes for `TestSigner::sign` to compute `sha256(private \|\| msg)`. |
| `derive(Clone, Eq, PartialEq)` | Per FR-003. **No `Hash` derive** (secret-discipline; per clarify Q3). |
| Hand-written `impl Debug for PrivateKey` | Outputs `PrivateKey([REDACTED])` — does NOT print the underlying bytes. Per FR-003 + clarify Q3 refinement. |
| `Display` | **Not implemented.** Production code that tries `format!("{}", private_key)` fails to compile. |

### `Signature`

| Item | Contract |
|------|----------|
| `pub fn new(bytes: Vec<u8>) -> Self` | Direct constructor; accepts any byte sequence. No `placeholder()` constructor — post-ADR-0010 the signing workflow assembles `SignedMessage { plain, signature }` in one step from a real signature (`signer.sign(&plain.signed_bytes())`), so no placeholder is needed. Tests that need a deliberately-wrong signature (e.g., US1 AS-3's 32-zero-bytes case) construct one directly via `Signature::new(vec![0u8; 32])`. |
| `pub fn as_bytes(&self) -> &[u8]` | Borrows the underlying bytes for byte-for-byte comparison during verification. |
| `impl Display for Signature` | Full lowercase hex (FR-003 Display bullet). |
| `derive(Clone, Debug, Eq, PartialEq)` | Per FR-003. **No `Hash` derive** (per clarify Q3 — signatures are not natural map keys). |

### `MessageHash`

| Item | Contract |
|------|----------|
| `pub const ZERO: MessageHash` | All-zero 32-byte hash sentinel. Used by `PlainMessage::signed_bytes` as the encoding for absent `parent_hash` (FR-010 + clarify Q1). |
| `pub fn new(bytes: [u8; 32]) -> Self` | Direct constructor for tests that fabricate hashes with specific bytes. |
| `pub fn of(plain: &PlainMessage) -> MessageHash` | Computes `MessageHash(sha256(plain.signed_bytes()).into())`. The hash is taken over the signed-over content only — signature is **not** in the hash input (per ADR 0010's content-anchored decision + FR-011 + IMPLEMENTATION_NOTES.md N-005). This is the function downstream callers use to derive the hash that becomes the next message's `parent_hash`. Callers holding a `SignedMessage` reach it via `MessageHash::of(&signed.plain)`. |
| `pub fn as_bytes(&self) -> &[u8; 32]` | Borrows the 32-byte array for `signed_bytes` encoding and comparison. |
| `impl Display for MessageHash` | Full lowercase hex (64 hex chars; FR-003 Display bullet). |
| `derive(Clone, Debug, Eq, PartialEq, Hash)` | Per FR-003. Hash supports future-feature use (e.g., per-`(topic, publisher)` chain-head maps once N-003 is revisited). |

### `Timestamp`

| Item | Contract |
|------|----------|
| `pub fn now() -> Self` | Wraps `SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64`. Returns `Timestamp::from_millis(0)` if the system clock is before 1970 (impossible in practice). Used by production publishers; no production publisher exists in 003. |
| `pub fn from_millis(ms: u64) -> Self` | Direct constructor. Used by every 003 test for deterministic timestamps. |
| `pub fn as_millis(&self) -> u64` | Borrows the underlying value for `signed_bytes` encoding (8 bytes big-endian per FR-010). |
| `derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)` | Per FR-003. `Copy` because the type is a single `u64`. `Ord`/`PartialOrd` for completeness (no test depends on ordering yet, but future replication / catch-up may). |

### `VerifyError`

| Item | Contract |
|------|----------|
| `Invalid` variant | Single v1 variant. Represents "signature does not match key+msg" for the mock; future verifier impls may distinguish (`KeyFormatInvalid`, `AlgorithmMismatch`, etc.) per FR-014's anticipation. |
| `#[non_exhaustive]` | Per FR-005 + clarify Q5. Adding variants in 011+ is a non-breaking change for downstream callers. Pattern-matching on `VerifyError` MUST include a `_ =>` arm at every call site. |
| `Debug` + `Display` (via thiserror) | `Display` for `Invalid` is `"invalid signature"`. Used in operator-facing surfaces only (the receive task does not include the error text in the `message_dropped` log entry; it folds every variant to `cause = "invalid_signature"` per FR-013). |
| `impl std::error::Error` (via thiserror) | Per the crate-wide typed-errors convention (ADR 0005 lineage). |

### `Signer` trait

```rust
pub trait Signer: Send + Sync {
    fn public_key(&self) -> PublicKey;
    fn sign(&self, msg: &[u8]) -> Signature;
}
```

| Item | Contract |
|------|----------|
| `Send + Sync` super-traits | Per FR-004. Implementors must be thread-shareable. |
| `fn public_key(&self) -> PublicKey` | Returns the public key matching this signer's secret. For `TestSigner`, this is `derive_public(&self.private)`. For future `Ed25519Signer`, this is the cryptographic public-key derivation. |
| `fn sign(&self, msg: &[u8]) -> Signature` | Infallible. Takes raw bytes (decoupled from any domain type); the caller passes `&plain.signed_bytes()` where `plain: PlainMessage`. |
| No type parameters, no associated types | Per FR-004 + ADR 0009. The trait is dyn-compatible (object-safe). |

### `Verifier` trait

```rust
pub trait Verifier: Send + Sync {
    fn verify(&self, key: &PublicKey, msg: &[u8], sig: &Signature) -> Result<(), VerifyError>;
}
```

| Item | Contract |
|------|----------|
| `Send + Sync` super-traits | Per FR-005. Required for storage as `Arc<dyn Verifier>` on `Node`. |
| `fn verify(...) -> Result<(), VerifyError>` | Synchronous. No `await`; called from the receive task directly. Returns `Ok(())` on signature match, `Err(VerifyError::Invalid)` (or future variant) on mismatch. |
| No type parameters, no associated types | Per FR-005 + ADR 0009. The trait is dyn-compatible. |

## `pubsub_node::crypto::mock` module

> **MOCK — not unforgeable.** The mock implementations below are designed to mirror the *shape* of real asymmetric crypto without imposing its security properties. Anyone with read access to this module's source can produce a forged signature that `TestVerifier::verify` accepts. The mock exists solely to differentiate correct-vs-incorrect key+message bindings in tests. Real authenticity arrives in feature 011 (`Ed25519Signer` / `Ed25519Verifier`).

### `MockCryptoScheme`

| Item | Contract |
|------|----------|
| `pub fn with_seed(seed: [u8; 32]) -> Self` | Constructs the scheme with a `ChaCha20Rng` seeded from the caller-supplied 32-byte seed. Tests pin reproducibility with this constructor (US4). |
| `pub fn from_entropy() -> Self` | Constructs the scheme with `ChaCha20Rng::from_entropy()` (OS entropy via `rand::rngs::OsRng`). For tests that explicitly want non-determinism. |
| `pub fn generate_keypair(&mut self) -> KeyPair` | Draws 32 fresh random bytes from the internal RNG for `private`, calls `derive_public` to produce `public`, returns `KeyPair { public, private }`. Successive calls on the same scheme yield independent pairs (the RNG advances). Two schemes with the same seed yield byte-identical pair sequences (FR-008, US4 AS-1). |
| `pub fn signer(&self, private: PrivateKey) -> TestSigner` | Constructs a `TestSigner` wrapping the given key. `&self` — does not advance the RNG. |
| `pub fn verifier(&self) -> TestVerifier` | Returns a fresh `TestVerifier` (zero-sized, stateless). `&self` — does not advance the RNG. |
| Rustdoc | Carries the "MOCK — not unforgeable" warning per SC-006. |

### `KeyPair`

| Item | Contract |
|------|----------|
| `pub public: PublicKey` | The key callers embed in `PublisherId` to receive the message envelope. |
| `pub private: PrivateKey` | The key callers pass to `MockCryptoScheme::signer` (or `TestSigner::new`) to produce signatures. |
| Invariant: `derive_public(&kp.private) == kp.public` | Holds for any `KeyPair` produced by `MockCryptoScheme::generate_keypair`. US4 AS-3 pins this. |
| Derives | `Clone, Debug` (via the redacted `PrivateKey::Debug`). No `Hash`, no `Display` (because `PrivateKey` doesn't have them). |

### `TestSigner`

| Item | Contract |
|------|----------|
| `pub fn new(private: PrivateKey) -> Self` | Constructs the signer wrapping the given private key. Used directly by tests holding a `PrivateKey` (often destructured from a `KeyPair`). |
| `impl Signer for TestSigner` | `public_key(&self) = derive_public(&self.private)`. `sign(&self, msg) = Signature(sha256(self.private.as_bytes() \|\| msg))`. |
| Rustdoc | Carries the "MOCK — not unforgeable" warning per SC-006. |

### `TestVerifier`

| Item | Contract |
|------|----------|
| Zero-sized struct (`pub struct TestVerifier;`) | No instance state; concurrent use is trivially safe. |
| `impl Verifier for TestVerifier` | `verify(&self, public, msg, sig)`: strips `PUBLIC_SUFFIX` from `public.as_bytes()`; if strip fails, returns `Err(VerifyError::Invalid)`. Otherwise computes `sha256(stripped \|\| msg)` and compares byte-for-byte with `sig.as_bytes()`. Match: `Ok(())`. Mismatch: `Err(VerifyError::Invalid)`. |
| Rustdoc | Carries the "MOCK — not unforgeable" warning per SC-006. |

### `derive_public` free function

| Item | Contract |
|------|----------|
| `pub fn derive_public(private: &PrivateKey) -> PublicKey` | Returns `PublicKey(private.as_bytes().to_vec() \|\| PUBLIC_SUFFIX)`. Deterministic, total. Exposed at `pubsub_node::crypto::mock::derive_public` so US4 AS-3 can assert `derive_public(&kp.private) == kp.public`. |

### `PUBLIC_SUFFIX` constant

Visibility: `pub(crate)`. Not re-exported. The fixed byte suffix `b"_public"` shared between `derive_public` (append) and `TestVerifier::verify` (strip). Test code that needs to fabricate a "key without the suffix" (US4 AS-5) does so by passing arbitrary bytes through `PublicKey::new(…)` directly — no direct access to the constant is required.

## `pubsub_node::PublisherId`

| Item | Contract |
|------|----------|
| `pub fn new(public: PublicKey) -> Self` | Direct constructor. |
| `impl From<PublicKey> for PublisherId` | Same as `new`; idiomatic. |
| `pub fn as_public_key(&self) -> &PublicKey` | Borrows the inner key for verifier dispatching. |
| `impl Display for PublisherId` | Delegates to the inner `PublicKey`'s `Display` — full lowercase hex. |
| `derive(Clone, Debug, Eq, PartialEq, Hash)` | Per FR-002. Distinct at the type level from `PeerId` even when both wrap the same `PublicKey` bytes (the compiler distinguishes them). |

## `pubsub_node::Message` — reshaped (was struct in 002, now enum per ADR 0010)

```rust
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Message {
    Signed(SignedMessage),
    // Future variants per ADR 0010:
    //   ConnectionHello(ConnectionHello),     // 004
    //   PeerSample(PeerSampleMessage),        // 005 / 010
    //   CatchUpRequest(CatchUpRequest),       // deferred replication
    //   …
}
```

| Item | Contract |
|------|----------|
| `#[non_exhaustive]` | Future protocol-message variants are added without breaking external callers. Pattern-matches on `Message` outside the crate MUST include a catch-all `_ =>` arm. |
| Sole 003 variant | `Message::Signed(SignedMessage)` — see the `SignedMessage` table below. |
| Derives | `Clone, Debug, Eq, PartialEq`. No `Hash` (not a HashMap key). |

## `pubsub_node::SignedMessage` — new (introduced by 003 per ADR 0010)

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedMessage {
    pub plain: PlainMessage,
    pub signature: Signature,
}
```

| Item | Contract |
|------|----------|
| `plain: PlainMessage` | The signed-over content; see `PlainMessage` below. |
| `signature: Signature` | The signer's output over `plain.signed_bytes()`. |
| Public fields | Callers destructure freely: `let SignedMessage { plain, signature } = signed;`. |
| Derives | `Clone, Debug, Eq, PartialEq`. No `Hash`. |
| Future methods (anticipated, not 003) | `fn verify(&self, verifier: &impl Verifier) -> Result<(), VerifyError>` (thin helper); `fn message_hash(&self) -> MessageHash` (delegates to `MessageHash::of(&self.plain)`). Adding these later is non-breaking. |

## `pubsub_node::PlainMessage` — new (introduced by 003 per ADR 0010)

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

| Item | Contract |
|------|----------|
| Field set | Per FR-001 — the §2.3 fields **excluding the signature**. Public fields; consumers construct directly (or via the `tests/common/mod.rs::build_signed_message` helper which wraps the result into a `Message::Signed(SignedMessage { … })`). |
| `pub fn signed_bytes(&self) -> Vec<u8>` | The canonical-encoding seam used by both signature production and verification (FR-010). Layout: `u32_be(topic.as_str().len()) \|\| topic.as_str().as_bytes() \|\| u32_be(publisher_id.as_public_key().as_bytes().len()) \|\| publisher_id.as_public_key().as_bytes() \|\| parent_hash.unwrap_or(MessageHash::ZERO).as_bytes() (32 bytes) \|\| sequence.to_be_bytes() (8 bytes) \|\| timestamp.as_millis().to_be_bytes() (8 bytes) \|\| u32_be(payload_encoded.len()) \|\| payload_encoded`. The `payload_encoded` for `MessagePayload::Ping(n)` is `[0x00] \|\| n.to_be_bytes()`. Per FR-010 + research.md §7. |
| Rustdoc on `signed_bytes` | Documents the byte layout in full (field order, widths, endianness, `MessageHash::ZERO` sentinel, `MessagePayload` variant tags). Treated as part of the protocol surface per FR-010 + IMPLEMENTATION_NOTES.md N-004 — changes to the layout require a rustdoc update in the same commit. |
| Derives | `Clone, Debug, Eq, PartialEq`. No `Hash`. |
| Signing workflow (post-ADR 0010 — no placeholder) | `let plain = PlainMessage { … };  let signature = signer.sign(&plain.signed_bytes());  let signed = SignedMessage { plain, signature };  let msg = Message::Signed(signed);`. |

## `pubsub_node::MessagePayload` — preserved from 002 (unchanged)

```rust
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MessagePayload {
    Ping(u64),
}
```

Carried unchanged from 002. Now lives as a field of `PlainMessage` (the dissemination application content). Future variants extend the enum without touching `PlainMessage`'s shape; `Ping(u64)` remains the sole variant in 003.

## `pubsub_node::network::RoutingFrame` — renamed from 001's `Envelope`

```rust
pub struct RoutingFrame {
    pub from: PeerId,
    pub message: Message,
}
```

Same shape as 001's `Envelope`; only the type name changes (per ADR 0010, freeing the term "envelope" for prose-level use matching the synthesis §2.3). All call sites that pattern-match on the type, all `tests/common/mod.rs` references, and the existing 002 tests that name the type are updated in the same commit as the rename.

## `pubsub_node::Node::new` — extended constructor signature

```rust
pub async fn new<N: Network>(
    self_id: PeerId,
    config: NodeConfig,
    initial_subscriptions: HashSet<TopicId>,
    network: Arc<N>,
    verifier: Arc<dyn Verifier>,
) -> Result<Self, NodeError>
```

| Item | Contract |
|------|----------|
| New parameter: `verifier: Arc<dyn Verifier>` | Appended at the end of the existing 002 parameter list (research.md §3). Required (not optional) per FR-012. Stored on the Node for the lifetime of the instance; consulted by the receive task on every inbound message that passes the topic filter (FR-013). |
| Other parameters | Unchanged from 002: `self_id`, `config`, `initial_subscriptions`, `network`. |
| No `signer` parameter | Per FR-012. Signing is caller-side; `Node::send` continues to take an already-built `Message`. |
| Return shape | Unchanged: `Result<Self, NodeError>`. No new error variants. |

## `pubsub_node::Node::send` — unchanged

```rust
pub async fn send(&self, to: &PeerId, message: Message) -> Result<(), NetworkError>
```

The 002 signature compiles unchanged after the `Message`-shape migration. The `Message` argument is now the post-ADR-0010 enum (sole 003 variant `Message::Signed(SignedMessage)` with the §2.3 fields living on the inner `PlainMessage`) rather than the 002-era two-field struct. Callers construct the message (typically via the test-support helper for tests; via a future publisher-CLI workflow for production). Per FR-012.

## Receive-task pipeline shape — operator-visible delta

The receive task body (per FR-013 + clarify Q6 + ADR 0010's pattern-match wrapper) follows this pipeline:

```text
inbound RoutingFrame { from, message }    // 001's `Envelope` renamed to `RoutingFrame` per ADR 0010
  ↓
  match message {
      Message::Signed(signed) → {
          ↓
          topic-subscription filter on signed.plain.topic (002 FR-004)
          ↓ miss → drop + emit event="message_dropped" cause="topic_not_subscribed" (FR-015)
          ↓ hit
          ↓
          signature verification:
            self.verifier.verify(signed.plain.publisher_id.as_public_key(),
                                 &signed.plain.signed_bytes(),
                                 &signed.signature)
          ↓ Err → drop + emit event="message_dropped" cause="invalid_signature" (FR-014)
          ↓ Ok
          ↓
          snapshot append: received_messages() grows by one entry, wrapping the full
                           Message::Signed(signed) value per the 001 FR-006
                           snapshot-append contract — extended by 002 FR-004 with
                           the topic-filter precondition.
      }
      // Future variants (Message::ConnectionHello, Message::PeerSample, etc.) gain
      // their own arms in subsequent features per ADR 0010's #[non_exhaustive] design.
  }
```

The 002 `event = "topic_drop"` emitter is REMOVED in this feature's implementation commit (FR-015). No source file in the crate emits an entry whose event marker is the legacy `"topic_drop"` value after 003 lands.

**Test-anchored contract**: tests assert only on `received_messages()` (presence vs absence). Log content is operator UX only (FR-014's tests-do-not-check-logs convention; matches 002 FR-011 / FR-014).

---

## What 003 does NOT change

For completeness, the following surface elements are explicitly unchanged by 003 (per `data-model.md` and FR-005 of 002 / FR-017 of 003):

- `PeerId`, `TopicId`, `PeerDescriptor`, `BasicPeerDescriptor`, `Network` trait, `NetworkHandle`, `InMemoryNetwork`, `MessagePayload` enum shape — all unchanged in behavior.
- The 001-era `Envelope { from, message }` routing-wrapper struct is **renamed to `RoutingFrame`** (per ADR 0010). Its fields, behavior, and routing role are unchanged; only the type name changes. The rename is the only `network.rs` edit in 003.
- `ReceivedDelivery` shape — unchanged. The `message` field's type is now the extended `Message` (per FR-001), but the wrapper struct itself is not edited.
- `NodeConfig`, `NodeConfig`'s TOML schema, `load_node_config`, `PeerEntry` — all unchanged (FR-017).
- `SubscribeOutcome` / `UnsubscribeOutcome`, `Node::subscribe` / `unsubscribe`, `Node::subscriptions`, `Node::peers`, `Node::received_messages` — all unchanged in shape; `received_messages` returns the extended `Message` shape transitively.
- All existing error types (`NodeError`, `NetworkError`, `ConfigError`) — unchanged.
- The CLI surface from `contracts/cli.md` (001) — unchanged (no new flag per FR-017 + spec Assumptions).

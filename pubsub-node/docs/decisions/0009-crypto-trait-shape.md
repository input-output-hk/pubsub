# ADR 0009: Crypto trait shape — concrete domain-owned `PublicKey` / `Signature`, trait objects on `Node`

**Status**: Accepted
**Date**: 2026-06-01
**Feature**: 003 (planned — message envelope + mock crypto)
**Source**: `specs/ROADMAP.md` §2 feature 003 + pre-spec design discussion 2026-06-01

## Context

Feature 003 grows `Message` to the staged-design-synthesis §2.3 envelope
shape — `(topic, publisher_id, parent_hash, sequence, timestamp, payload,
signature)` — and introduces a `Signer` / `Verifier` trait pair. The first
impl pair is `TestSigner` / `TestVerifier` (hash-based, deterministic, fast);
the real `Ed25519Signer` / `Ed25519Verifier` arrives in feature 011.

The shape of those traits is a structural decision per Constitution
Principle III: the trait's signature determines whether type parameters
propagate through `Message`, `Network`, `ChainState`, and ultimately `Node`,
or whether the crypto layer sits behind concrete types that callers depend
on by name. Reversing it later would touch every type that mentions a
public key or a signature.

Three constraints frame the choice:

1. **Crypto-clean trait.** The trait should describe a generic
   "sign-and-verify" capability, not a publisher-specific or peer-specific
   one. Feature 004+ will introduce per-peer connection authentication, and
   those peers will sign handshake messages with the same trait. The trait
   must therefore know nothing about publishers, topics, envelopes, or
   any other domain concept.

2. **No accidental type-parameter contagion on `Node`.** Feature 001
   established a "trait-at-construction, concrete-at-storage" pattern: the
   `N: Network` bound exists only on `Node::new`; once registration
   completes, the Node stores a concrete `NetworkHandle` and carries no
   type parameter on the struct itself. The crypto shape must respect this
   precedent — adding `<C: CryptoScheme>` to `Node` would break it.

3. **Cheap migration path to real crypto in 011.** Both shapes considered
   below admit a mechanical 011 migration at prototype scale (per saved
   memory: prototype phase, deployment-update cost not material). The
   question is which shape leaves the rest of the code base
   simplest *now*, during 003–010, while the trait is exercised by only
   one impl pair at a time.

Two shapes are on the table — concrete domain-owned newtypes (the
`libp2p` / `iroh` / `pallas` pattern) versus associated types bundled in
a `CryptoScheme` trait (the RustCrypto `signature` pattern). The
discussion that produced this ADR worked through both side by side.

## Decision

**The `Signer` / `Verifier` traits operate on concrete domain-owned
`PublicKey` and `Signature` newtypes that live in `pubsub_node::crypto`.
The traits take no type parameters. `Node` stores `Arc<dyn Verifier>` as
a field and remains type-parameter-free.**

Concretely:

```rust
// crate::crypto
pub struct PublicKey(Vec<u8>);
pub struct PrivateKey(Vec<u8>);
pub struct Signature(Vec<u8>);

#[derive(Debug, thiserror::Error)]
pub enum VerifyError { #[error("invalid signature")] Invalid }

pub trait Signer: Send + Sync {
    fn public_key(&self) -> PublicKey;
    fn sign(&self, msg: &[u8]) -> Signature;
}

pub trait Verifier: Send + Sync {
    fn verify(&self, key: &PublicKey, msg: &[u8], sig: &Signature) -> Result<(), VerifyError>;
}
```

`PublicKey`, `PrivateKey`, and `Signature` are opaque byte newtypes — the
traits do not interpret them. Concrete impls choose how to populate the
bytes: the future `Ed25519Signer` uses Ed25519's 32-byte public, 32-byte
private, and 64-byte signature representations; the mock impl described
below uses opaque random bytes plus a synthetic public-key derivation.
Both impls satisfy the same trait without the trait knowing the scheme.

### Mock crypto for tests (`crate::crypto::mock`)

The mock pair (`MockCryptoScheme`, `TestSigner`, `TestVerifier`) lives in
a `mock` submodule of `crate::crypto`. It is asymmetric-shaped — the
signer takes a `PrivateKey` and the verifier takes a `PublicKey`, matching
the production trait shape — but the asymmetry is faked: the public key
is derived from the private key by appending a fixed byte suffix
(`b"_public"`), and the verifier recovers the private by stripping that
suffix. Anyone with the `PublicKey` can therefore forge a signature; this
is acceptable for a mock and is loudly documented in rustdoc.

```rust
// crate::crypto::mock
const PUBLIC_SUFFIX: &[u8] = b"_public";

pub struct MockCryptoScheme {
    rng: rand_chacha::ChaCha20Rng,
}

pub struct KeyPair {
    pub public: PublicKey,
    pub private: PrivateKey,
}

impl MockCryptoScheme {
    /// Reproducible — explicit 32-byte seed.
    pub fn with_seed(seed: [u8; 32]) -> Self {
        use rand::SeedableRng;
        Self { rng: rand_chacha::ChaCha20Rng::from_seed(seed) }
    }

    /// Non-deterministic — seeded from OS entropy.
    pub fn from_entropy() -> Self {
        use rand::SeedableRng;
        Self { rng: rand_chacha::ChaCha20Rng::from_entropy() }
    }

    pub fn generate_keypair(&mut self) -> KeyPair {
        use rand::RngCore;
        let mut private_bytes = vec![0u8; 32];
        self.rng.fill_bytes(&mut private_bytes);
        let private = PrivateKey(private_bytes);
        let public  = derive_public(&private);
        KeyPair { public, private }
    }

    pub fn signer(&self, private: PrivateKey) -> TestSigner { TestSigner { private } }
    pub fn verifier(&self) -> TestVerifier { TestVerifier }
}

fn derive_public(private: &PrivateKey) -> PublicKey {
    let mut bytes = private.0.clone();
    bytes.extend_from_slice(PUBLIC_SUFFIX);
    PublicKey(bytes)
}

pub struct TestSigner { private: PrivateKey }

impl Signer for TestSigner {
    fn public_key(&self) -> PublicKey { derive_public(&self.private) }
    fn sign(&self, msg: &[u8]) -> Signature {
        let mut input = self.private.0.clone();
        input.extend_from_slice(msg);
        Signature(sha256(&input).to_vec())
    }
}

pub struct TestVerifier;

impl Verifier for TestVerifier {
    fn verify(&self, key: &PublicKey, msg: &[u8], sig: &Signature) -> Result<(), VerifyError> {
        let private_bytes = key.0
            .strip_suffix(PUBLIC_SUFFIX)
            .ok_or(VerifyError::Invalid)?;
        let mut input = private_bytes.to_vec();
        input.extend_from_slice(msg);
        if sig.0 == sha256(&input) { Ok(()) } else { Err(VerifyError::Invalid) }
    }
}
```

Design points worth recording:

- **`PrivateKey` never appears in `Node`'s API.** It is consumed by test
  harnesses (and, eventually, a publisher CLI) that build signed
  envelopes. `Node` only holds `Arc<dyn Verifier>` and indirectly observes
  `PublicKey` via `PublisherId` on inbound messages.
- **Seeded RNG for reproducibility.** Tests that depend on key bytes
  call `MockCryptoScheme::with_seed(...)` to pin the byte stream;
  successive `generate_keypair()` calls advance the RNG so each pair is
  independent. `from_entropy()` covers test cases that don't care about
  reproducibility, and the eventual publisher CLI that wants a one-shot
  fresh key.
- **Shared suffix constant.** `PUBLIC_SUFFIX` is declared once and used
  by both `derive_public` (signer's public-key path) and the verifier's
  `strip_suffix` (verifier's private-key recovery path). The two
  operations stay byte-symmetric by construction; a typo on either side
  would break verification immediately.
- **PRNG: `rand` + `rand_chacha`.** Idiomatic Rust randomness; ChaCha20
  is the standard seeded-RNG choice. The dep adds ~5 sub-crates that are
  ubiquitous across the Rust ecosystem.
- **SHA-256 dep: `sha2`.** Needed independently for the canonical
  envelope hash that produces `MessageHash` and that feeds the signing
  input (see `specs/IMPLEMENTATION_NOTES.md` N-004).
- **Mock-only forgery.** `TestVerifier` can recover the private key from
  the public key via `strip_suffix`, so anyone with read access to the
  module source can forge a signature for a given public key. This is
  a deliberate property of a mock — rustdoc on `MockCryptoScheme`,
  `TestSigner`, and `TestVerifier` carries a prominent **"MOCK — not
  unforgeable"** warning paragraph. Real authenticity arrives with
  `Ed25519Signer` in feature 011.
- **Migration path to feature 011.** The trait surfaces (`Signer`,
  `Verifier`, `PublicKey`, `PrivateKey`, `Signature`) do not change.
  Feature 011 adds an `ed25519` submodule next to `mock` with
  `Ed25519Signer`, `Ed25519Verifier`, and an `Ed25519CryptoScheme` whose
  `generate_keypair` calls `ed25519_dalek::SigningKey::generate(rng)`
  with the same `rand_chacha` RNG. Call sites switch the factory and the
  internal byte interpretation but keep the same trait API.

`PublisherId` is a thin newtype around `PublicKey`:

```rust
// crate::message
pub struct PublisherId(PublicKey);

pub struct Message {
    pub topic: TopicId,
    pub publisher_id: PublisherId,
    pub parent_hash: Option<MessageHash>,
    pub sequence: u64,
    pub timestamp: Timestamp,
    pub payload: MessagePayload,
    pub signature: Signature,
}
```

The newtype expresses the role distinction ("this `PublicKey` is the
publisher's key, used to verify their signature on this message") at the
type level rather than only in the field name. Future `PeerId(PublicKey)`
under real crypto in 011 will be a sibling newtype, distinguishable by the
compiler from `PublisherId` even when both wrap the same underlying byte
representation.

`Node` extends the 001 pattern by storing the verifier as a trait object:

```rust
// crate::node
pub struct Node {
    handle: NetworkHandle,
    peers: Vec<BasicPeerDescriptor>,
    received: Arc<Mutex<Vec<ReceivedDelivery>>>,
    subscriptions: Arc<Mutex<HashSet<TopicId>>>,
    verifier: Arc<dyn Verifier>,
    recv_task: JoinHandle<()>,
}

impl Node {
    pub async fn new<N: Network>(
        ...,
        verifier: Arc<dyn Verifier>,
    ) -> Result<Self, NodeError> { ... }
}
```

The trait bound `N: Network` on `new` is preserved unchanged from 001; the
verifier enters as an already-erased trait object passed in by the caller
(the TOML loader in production, test harnesses in tests). No type
parameter is added to `Node` itself.

Note that `Node` does **not** hold a `Signer` field at this iteration —
the Node does not itself construct signed messages. `Node::send` continues
to take an already-built `Message`, and any caller that needs to publish
(test harnesses today, a publisher CLI in a later feature) uses the
`Signer` trait from `pubsub_node::crypto` at the call site to assemble
the envelope before handing it to `Node::send`. Moving the `Signer` into
the Node, along with the per-publisher chain-head tracking that would
make `Node::send(payload)` ergonomic, is a deliberate non-goal for 003.

### Side-by-side comparison

For the record, the alternative was an associated-types pattern bundling
the scheme into a `CryptoScheme` trait:

```rust
// Option B (rejected) — crate::crypto
pub trait CryptoScheme: Send + Sync + 'static {
    type PublicKey: Clone + Eq + Hash + Send + Sync;
    type Signature: Clone + Eq + Send + Sync;
}

pub trait Signer<C: CryptoScheme>: Send + Sync {
    fn public_key(&self) -> C::PublicKey;
    fn sign(&self, msg: &[u8]) -> C::Signature;
}

pub trait Verifier<C: CryptoScheme>: Send + Sync {
    fn verify(&self, key: &C::PublicKey, msg: &[u8], sig: &C::Signature) -> Result<(), VerifyError>;
}

pub struct TestScheme;
impl CryptoScheme for TestScheme {
    type PublicKey = TestKey;
    type Signature = TestSig;
}
```

```rust
// Option B — crate::message
pub struct PublisherId<C: CryptoScheme>(C::PublicKey);

pub struct Message<C: CryptoScheme> {
    pub topic: TopicId,
    pub publisher_id: PublisherId<C>,
    pub parent_hash: Option<MessageHash>,
    pub sequence: u64,
    pub timestamp: Timestamp,
    pub payload: MessagePayload,
    pub signature: C::Signature,
}
```

```rust
// Option B — crate::node
pub struct Node<C: CryptoScheme, N: Network<C>, V: Verifier<C>, S: Signer<C>> {
    handle: NetworkHandle,
    verifier: V,
    signer:   Option<S>,
    chain_state: ChainState<C>,
    ...
}

// at the top of main.rs / tests:
type AppCrypto = TestScheme;  // swap to Ed25519Scheme in 011
let node: Node<AppCrypto, InMemoryNetwork<AppCrypto>, TestVerifier, TestSigner> = ...;
```

| Aspect | Option A (chosen) | Option B (rejected) |
|---|---|---|
| Type params on `Node` | none (matches 001) | `<C, N, V, S>` |
| Type params on `Message` | none | `<C>` |
| Type params on `Network` | none | `<C>` (since `Network::send(&Message<C>)`) |
| Type params on `ChainState` | none | `<C>` |
| Wire-format / serialization | one `Encode for Message` impl | generic over `C`; usually needs `C::PublicKey: AsRef<[u8]>` bound |
| Multiple schemes simultaneously | only via enum dispatch | natively supported |
| 011 migration cost | swap `PublicKey(Vec<u8>)` internal repr | swap `type AppCrypto = TestScheme` to `Ed25519Scheme` once |
| Compiler-enforced "Ed25519 sig can't verify a BLS message" | no | yes |
| New-engineer ramp-up | reads as plain Rust app code | reads as a generic protocol crate |
| Fit with 001's "trait-at-construction, concrete-at-storage" pattern | preserved | broken |

## Consequences

- The crypto trait stays domain-clean. Feature 004+ peer-side signing
  reuses the same `Signer` / `Verifier` traits with different impls; the
  trait knows nothing about the role and does not need to.
- `Node`, `Message`, `Network`, and `ChainState` carry no crypto-related
  type parameters. The 001 pattern of trait-at-construction,
  concrete-at-storage is preserved.
- The 011 migration is a single internal edit: replace
  `PublicKey(Vec<u8>)` with `PublicKey([u8; 32])` (or whichever Ed25519
  representation), and replace the `TestSigner` / `TestVerifier` impls
  with `Ed25519Signer` / `Ed25519Verifier`. Field types, function
  signatures, and call sites elsewhere in the crate are unaffected.
- `PublisherId` and the future `PeerId(PublicKey)` distinguish their roles
  at the type level. The compiler rejects accidental cross-role use even
  when both wrap the same byte representation.
- The trade-off: the crate cannot statically enforce "an Ed25519 signature
  cannot be verified with a BLS verifier" — multi-scheme mixing would have
  to be expressed dynamically (enum tag inside `PublicKey` /
  `Signature`, or runtime registry of verifier impls). This is not a
  concern at SRL 2–3; the crate will run a single scheme at a time
  through the foreseeable roadmap.
- The trait-object choice (`Arc<dyn Verifier>` on `Node`) imposes vtable
  dispatch per `verify` call. For the message-rate workloads the v1
  system targets (per-publisher rates dominated by network and signature
  costs), this is in the noise. If a future feature profiles a hot path
  that bottlenecks on dispatch, it can promote the relevant field to a
  concrete monomorphised type without changing the trait surface.

## Alternatives considered

- **Associated types via `CryptoScheme` trait (Option B above)**:
  rejected. Adds type parameters to `Node`, `Message`, `Network`, and
  `ChainState`. Breaks 001's no-type-parameter pattern. Pays the
  generic-bookkeeping cost for a benefit (compile-time scheme
  distinction) the project does not currently need.

- **Associated types directly on the traits without a `CryptoScheme`
  wrapper** (`pub trait Verifier { type PublicKey; type Signature; ... }`):
  rejected. Forces `Message` to require an `S: Signer` *and* a
  `V: Verifier` bound with `where S::PublicKey = V::PublicKey, S::Signature
  = V::Signature` to keep the two sides agreeing. Same propagation
  problem, with worse ergonomics than the bundled-scheme variant.

- **Plain `&[u8]` everywhere** (no `PublicKey` / `Signature` newtypes;
  the trait takes byte slices directly): rejected. Loses the typed
  distinction between an arbitrary byte slice and a public key on the
  caller side — easy to pass the wrong bytes. The newtype cost is one
  struct and a few accessors; the safety it buys is non-trivial.

- **`PublisherId(String)` decoupled from `PublicKey`**: rejected. At
  prototype the publisher_id and the key coincide (TestSigner hashes
  payload with the key-bytes; the id *is* the key). Introducing a
  separate `String` representation would require a `String -> PublicKey`
  bridge at every `Verifier::verify` call. The newtype-around-`PublicKey`
  shape mirrors the 011 end-state where the id literally is the key
  bytes.

- **`pub type PublisherId = PublicKey;` alias instead of a newtype**:
  rejected. Loses the role distinction at the type level. The future
  `PeerId(PublicKey)` newtype would then be indistinguishable from
  `PublisherId` to the compiler, and accidental cross-role use becomes
  a comment-discipline problem rather than a compile error.

- **Promote `PublicKey` / `Signature` to associated types only on
  `Signer`, keep `Verifier` concrete**: rejected as asymmetric and
  confusing. Either both sides parameterize over the scheme or neither
  does; mixing is the worst of both worlds.

## Sources

- `specs/ROADMAP.md` §2 feature 003 — feature scope, open questions
  enumeration, dependency on 002.
- `../docs/staged-design-synthesis.md` §2.3 — message envelope shape that
  `Message` realises.
- ADR 0006 — establishes the receive-task model that the `Arc<dyn
  Verifier>` field plugs into.
- ADR 0007 — establishes the `NetworkHandle` concrete-storage pattern
  this ADR extends to crypto.
- Saved feedback memory `lock_in_future_interface_shapes` — informs
  preferring opaque types and trait abstractions over the simplest
  stub when downstream iterations will need them.

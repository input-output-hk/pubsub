# ADR 0010: Protocol-message type hierarchy — `Message` enum, `SignedMessage` / `PlainMessage` split, 001 `Envelope` renamed `RoutingFrame`

**Status**: Accepted
**Date**: 2026-06-03
**Feature**: 003 (message envelope + mock crypto)
**Source**: `specs/003-message-envelope-mock-crypto/spec.md ## Clarifications` Session 2026-06-03, post-round-5 design discussion + this ADR

## Context

The 003 plan as originally drafted (commits `16af6fb` and `b397041`) treated `Message` as a single struct carrying the staged-design-synthesis §2.3 envelope shape — `{ topic, publisher_id, parent_hash, sequence, timestamp, payload, signature }`. The implicit claim was "every protocol message looks like this." That claim is false the moment 004 lands and stays false through every subsequent feature.

Survey of protocol-message types arriving in the next several features (ROADMAP §2):

- **004 (connection-oriented network model)**: introduces `Connection` lifecycle. Even if the TCP-level handshake is transport-internal, Nodes will exchange peer-id confirmation / capability advertisement / clock-sync probes between accept and first dissemination. None of these carry a topic, a publisher_id, or a signature.
- **005 / 010 (peer view, real peer-sampling)**: SecureCyclon-style peer-view fragment exchanges per the AUEB paper. Short structured packets carrying lists of peer descriptors. Not topic-tagged, not publisher-signed; protocol-level only.
- **006 (epochal dialer + fan-out)**: no new message types directly, but the dialer's connection lifecycle creates non-dissemination traffic on every epoch boundary.
- **008 (registry abstraction)**: registry-lookup queries ("is publisher X authorized on topic T?") plus their responses. These exchanges may flow alongside dissemination but carry no envelope.
- **Deferred replication / catch-up**: "send me messages from sequence N..M for publisher P on topic T" + a batch response. The request carries no envelope; the response is a batch of signed envelopes.

Locking `Message` to the §2.3 envelope shape in 003 means every subsequent feature either contorts non-dissemination messages into the envelope shape (with stub publisher_id / signature fields), or carries a parallel non-Message type at the network layer (defeating the single-type-on-the-wire goal). Both options are wrong.

The second motivating concern is **the placeholder-signature workflow** that 003's FR-010 originally specified: "construct the message with a placeholder signature, compute `signed_bytes`, sign those bytes, replace the placeholder with the produced signature." This dance exists because the canonical-bytes computation is on a type that includes the signature field. Separating the signed-over content from the signature eliminates the dance entirely.

Both concerns argue for the same structural change: split `Message` into a top-level enum (kinds of protocol traffic), with the dissemination case landing on a separate type whose pre-signature content is its own type.

A third concern — surfaced during this ADR's drafting — is **terminology alignment with the staged-design-synthesis**. The synthesis §2.3 uses "envelope" for the *whole signed message* (signature included). Earlier 003 drafts conflated "envelope" with "the signed-over content" and bundled the term into a spec-Assumptions clarification. That clarification was internally consistent but diverged from the synthesis's wording. The restructure also gives us a chance to reserve "envelope" in prose for what the synthesis means and let the Rust type names sit alongside the prose without competing for the term.

## Decision

**The `pubsub_node::Message` type becomes a `#[non_exhaustive]` enum whose sole 003-era variant carries a separate `SignedMessage` struct. `SignedMessage` is split into a `PlainMessage` (the signed-over content) and a `Signature`. The 002-era `MessagePayload` enum (`Ping(u64)`) is preserved unchanged and lives as a field of `PlainMessage`. The 001-era `Envelope { from: PeerId, message: Message }` routing-wrapper struct is renamed to `RoutingFrame` to free the term "envelope" for prose-level use matching the staged-design-synthesis's terminology.**

Concretely:

```rust
// crate::message

#[non_exhaustive]
pub enum Message {
    Signed(SignedMessage),
    // Future variants land here without touching SignedMessage / PlainMessage:
    //   ConnectionHello(ConnectionHello),       // 004
    //   ConnectionAccept(ConnectionAccept),     // 004
    //   PeerSample(PeerSampleMessage),          // 005 / 010
    //   CatchUpRequest(CatchUpRequest),         // deferred replication
    //   CatchUpBatch(CatchUpBatch),             // deferred replication
    //   …
}

pub struct SignedMessage {
    pub plain: PlainMessage,
    pub signature: Signature,
}

pub struct PlainMessage {
    pub topic: TopicId,
    pub publisher_id: PublisherId,
    pub parent_hash: Option<MessageHash>,
    pub sequence: u64,
    pub timestamp: Timestamp,
    pub payload: MessagePayload,
}

#[non_exhaustive]
pub enum MessagePayload {
    Ping(u64),
}

// PlainMessage owns the canonical-encoding seam (no signature field in scope):
impl PlainMessage {
    pub fn signed_bytes(&self) -> Vec<u8> { /* hand-rolled length-prefixed encoding */ }
}

// MessageHash::of consumes a PlainMessage (or a SignedMessage's plain), not a Message:
impl MessageHash {
    pub fn of(plain: &PlainMessage) -> MessageHash { /* sha256(plain.signed_bytes()) */ }
}
```

```rust
// crate::network — the 001-era routing wrapper renamed:

pub struct RoutingFrame {
    pub from: PeerId,
    pub message: Message,
}
// formerly: pub struct Envelope { … }
```

The signing workflow at any call site (test helper, future publisher CLI) becomes:

```rust
let plain = PlainMessage { topic, publisher_id, parent_hash, sequence, timestamp, payload };
let signature = signer.sign(&plain.signed_bytes());
let signed = SignedMessage { plain, signature };
let msg = Message::Signed(signed);
```

No placeholder; the signature is produced from the `PlainMessage` without ever appearing in scope as a field of a not-yet-signed value.

The receive task pattern-matches on the variant:

```rust
match envelope.message {  // envelope here is the RoutingFrame from the network layer
    Message::Signed(signed) => {
        // 002 topic-filter step → verification step → snapshot append (per FR-013)
    }
    // Future variants get their own handlers.
}
```

The 003 receive-task pipeline is unchanged in behaviour: topic filter first (002 FR-004 + 003 FR-013's Q6 ordering), then signature verification, then snapshot append. The only delta is the pattern-match wrapper.

## Consequences

- **No placeholder-signature workflow.** `PlainMessage::signed_bytes` is computed without any signature field in scope; the result feeds `Signer::sign`; the resulting `Signature` and the original `PlainMessage` are then assembled into a `SignedMessage`. Cleaner at every call site, no `Signature::placeholder()` constructor needed, no "construct → recompute → replace" dance.
- **Future protocol-message variants are append-only on `Message`.** 004's connection-control messages, 005 / 010's peer-sampling messages, 008's registry-lookup messages, and the deferred-replication request/response pair all slot in as new `Message` variants. The dissemination pipeline only fires for `Message::Signed`; other variants get their own handlers. Adding variants is non-breaking for downstream callers thanks to `#[non_exhaustive]`.
- **Type-level separation of "what kind of protocol message" from "what application content".** The top-level enum is `Message` (kinds of protocol traffic). The application content carried inside a dissemination message is `MessagePayload` (variants of dissemination body). These are different concerns; the restructure separates them cleanly.
- **Re-justifies 002's `MessagePayload` introduction.** 002 introduced `MessagePayload` anticipating future variants. The restructure preserves that intent unchanged — `MessagePayload` lives inside `PlainMessage`, as a payload of the dissemination case. The 002-era argument ("different messages will share different fields") now holds at the *outer* layer (variants of `Message`), and `MessagePayload` continues to do its 002 job at the *inner* layer (variants of dissemination body).
- **Re-aligns Rust types with synthesis terminology.** The staged-design-synthesis §2.3 uses "envelope" for the *whole signed message* (signature included). After the restructure, that prose-level "envelope" corresponds to `SignedMessage`. The Rust type `Envelope` (formerly the 001 routing wrapper) is renamed to `RoutingFrame`, freeing the term in prose. The spec's existing Assumptions terminology bullet is updated to reflect the new alignment.
- **Methods belong on the variant payload.** `impl SignedMessage { fn verify(&self, v: &impl Verifier) -> Result<(), VerifyError>; fn message_hash(&self) -> MessageHash; }` reads naturally without going through `Message` first. The receive task can pass `&SignedMessage` to a verify helper; test fixtures return `SignedMessage` from builders rather than `(Envelope, Signature)` tuples or `Message`.
- **Test-support helper signatures change.** The `tests/common/mod.rs::build_signed_message` helper now returns `SignedMessage` (the layered type), not `Message`. Tests call `Message::Signed(build_signed_message(…))` at the moment of dispatching to `node.send`. The `build_signed_message_simple` convenience wrapper does the same.
- **Migration cost extends to the 001 / 002 tests, but mechanically.** Every existing test that constructed `Message { topic, payload }` becomes `Message::Signed(SignedMessage { plain: PlainMessage { topic, publisher_id, parent_hash: None, sequence: 0, timestamp: Timestamp::from_millis(0), payload }, signature })` — typically replaced via the `build_signed_message_simple` helper. The 001 `RoutingFrame` rename is a single-file edit in `src/network.rs` plus a small grep-and-replace across tests that pattern-match on the type name.
- **The receive task gains a pattern-match wrapper.** Today: `while let Some(env) = rx.recv().await { … topic-filter logic …  }`. After 003: `while let Some(frame) = rx.recv().await { match frame.message { Message::Signed(signed) => { … topic-filter + verify + snapshot … } } }`. The `#[non_exhaustive]` attribute means the compiler doesn't require a catch-all arm in 003 (only the `Signed` variant exists), but adding one (`_ => { /* drop with cause = "unsupported_message_kind" */ }`) is a small future enhancement that 004+ may introduce.
- **`MessageHash::of` consumes `&PlainMessage`** (not `&SignedMessage`). This is the function downstream callers use to derive the hash that becomes the next message's `parent_hash`. Test fixtures that build a chain of messages compute `MessageHash::of(&prev.plain)` and pass it as `Some(...)` on the next `PlainMessage`. **The choice to hash the content (signature excluded) rather than the wrapping `SignedMessage` (signature included) is deliberate** and is recorded as part of this ADR's structural decision because the `PlainMessage` / `SignedMessage` split is what *enables* the content-only-hash shape. The rationale:
  - **Signature-malleability immunity**: the parent-hash chain stays valid across signing-scheme changes (feature 011's Ed25519 swap; any later scheme migration). This is the canonical Bitcoin pre-SegWit lesson: early TXIDs hashed `(body || signature)`, and ECDSA's signature malleability meant byte-different but semantically-identical transactions hashed to different TXIDs, breaking every layered protocol that addressed transactions by TXID. SegWit fixed it by separating the witness from the body in the hash input. Hashing `PlainMessage` puts us in the post-SegWit world from day one.
  - **Cardano ecosystem alignment**: Cardano's `tx_hash = blake2b(tx_body)` hashes the body separately from witnesses; the witness set is hashed (and stored on-chain) as its own object. The pubsub-node lives in the Cardano workstream, and its hash semantics should match. Cross-language signers (when 009+ enables them) only need to agree on the content encoding to interoperate on the chain, not on the signature byte format.
  - **Content addressing as a clean concept**: `MessageHash` answers "what is the identity of the content this publisher committed to?" — independent of how that commitment was attested. The signature is one (or potentially several, under future schemes) witnesses to that content. Separating content from witness at the hash level keeps the protocol mental model crisp.
  - **Stability across signing-scheme evolution**: when 011 lands and the verifier swaps from `TestVerifier` to `Ed25519Verifier`, no existing chain bookkeeping breaks because the hash input is signature-independent. Same for any subsequent scheme change (Ed25519 → BLS for aggregation, or post-quantum schemes much later).
  - **Counter-argument acknowledged**: wire-level dedup of signature-different but content-identical messages cannot rely on `MessageHash` alone — a future caching feature would need a separate "wire hash" over `SignedMessage`. This is a real but bounded cost and is plan-level for that future feature, not 003. Tracked for revisit in `specs/IMPLEMENTATION_NOTES.md` N-005.
- **The 002 `topic_drop` → `message_dropped` / `cause = "topic_not_subscribed"` rename is unaffected by this ADR.** FR-015 and the saved drop-event convention apply unchanged; the rename just happens inside the new `Message::Signed(signed)` pattern-match arm.
- **No new dependencies introduced by this ADR.** The restructure is purely a type-shape decision; the existing 003 dep set (`rand`, `rand_chacha`, `sha2`) carries through.

## Alternatives considered

- **Keep `Message` as a single struct, defer the restructure to 004.** Rejected. The argument from the user surfacing this concern is correct: the restructure is cheaper now than after 004 builds more code on top of the current shape. By 005 / 010 there will be peer-sampling messages flowing through the same network; restructuring at that point would touch the connection layer, the peer-sampling protocol, and the dissemination layer simultaneously.

- **Use an inline struct variant `Message::Signed { plain, signature }` (Form B) instead of a tuple variant wrapping a `SignedMessage` struct (Form A).** Rejected. Form B precludes implementing methods on the dissemination payload directly (an `impl SignedMessage` block can't exist if there's no `SignedMessage` type), forces every test helper that wants a "signed message" to return a `Message` or a `(PlainMessage, Signature)` tuple, and prevents future polymorphism (e.g., `trait HasEnvelope` implemented for the dissemination case but not for `ConnectionHello`). Form A is the Rust idiomatic answer for protocol-message enums (`rustls::Message`, `webrtc::Message`, etc.) and costs only one extra type name plus a single layer of nesting at construction sites.

- **Promote `Ping` to a top-level variant of `Message`** (parallel to `Message::Signed`) rather than keeping it as a `MessagePayload::Ping` under the dissemination case. Rejected. Introduces an awkward "what's the dissemination payload, then?" question — either `MessagePayload` becomes an empty/opaque enum (`MessagePayload::Empty` or `MessagePayload::Data(Vec<u8>)`), or `Ping` appears at two layers simultaneously (top-level + payload variant) which is conceptually muddled. Keeping `Ping` as the sole `MessagePayload` variant for now preserves 002's pattern, minimises migration cost, and leaves the future option of adding richer payload variants (`GovernanceUpdate`, `DeFiIntent`, `SPOAlert`, …) as the application-level use cases mature.

- **Name the pre-signature content type `Envelope`** (matching the spec's earlier internal use of "envelope" for the §2.3 shape) rather than `PlainMessage`. Rejected. The staged-design-synthesis §2.3 uses "envelope" for the *whole signed message*, signature included. Using `Envelope` for the signed-over content type would force the spec's Assumptions terminology bullet to diverge from the synthesis. Using `PlainMessage` keeps the Rust type names neutral and leaves the prose-level "envelope" aligned with what the synthesis means. The `plain → signed` pairing also reads naturally at call sites.

- **Name the pre-signature content type `RawMessage`** (the user's initial suggestion in the surfacing discussion). Rejected. "Raw" carries a "before parsing / validation / processing" connotation that's a near-miss for what we want; the type is parsed and validated, just not signed. `PlainMessage` is the cleaner fit semantically.

- **Name the pre-signature content type `SignedContent` / `SignedBody`.** Considered. Accurate ("what gets signed") but slightly redundant with the outer `SignedMessage` name, and less concise than `PlainMessage`. Reading "SignedMessage wraps a SignedContent" feels noisier than "SignedMessage wraps a PlainMessage". Skipping.

- **Rename `MessagePayload` to `Payload`** in the same commit as this restructure. Considered. The 003 plan originally proposed this rename on the grounds that "MessagePayload" was redundant once the type lived inside a deeper hierarchy. Rejected after weighing migration cost: 002 introduced the name, the type still does its job, and renaming touches every 001 / 002 test plus all the spec / plan / data-model / contracts artifacts. Preserving `MessagePayload` is the cheaper choice.

- **Keep the 001 `Envelope` name and use a different name for the §2.3 content (or for the synthesis prose).** Considered. The naming collision is real either way — the synthesis uses "envelope" for one thing, 001 used `Envelope` for another, and the 003 spec previously used "envelope" for a third (the §2.3 shape minus signature). The cleanest resolution is to rename one of the Rust types and let the prose-level term sit on the protocol concept; renaming the 001 routing wrapper to `RoutingFrame` (which describes its job more precisely anyway) frees the prose-level "envelope" for the synthesis's meaning.

- **Add a `Verifier` trait method directly on `SignedMessage` (e.g., `signed.verify(&verifier) -> Result<…>`)** rather than the current `verifier.verify(plain.as_public_key(), &plain.signed_bytes(), &signature)` shape. Considered. The shorter method form is ergonomic at call sites, but couples `SignedMessage`'s impl to the `Verifier` trait, which weakens the trait's "I am crypto, not a domain type" property from ADR 0009. Deferred — could be added later as a thin helper without changing the trait surface. Out of scope for this ADR.

- **Hash `SignedMessage` (signature included) rather than `PlainMessage` (signature excluded) for `MessageHash`.** Considered explicitly during ADR drafting; the alternative would tie the parent-hash chain to the signature format. Rejected for the four reasons enumerated in the "Consequences — `MessageHash::of` consumes `&PlainMessage`" bullet above: signature-malleability immunity (the Bitcoin pre-SegWit lesson), Cardano `tx_hash = blake2b(body)` alignment, content-addressing cleanness, and stability across signing-scheme evolution. The acknowledged counter-argument (wire-level dedup of signature-different / content-identical messages needs a separate hash) is real but bounded and deferred to a future caching feature; tracked in `specs/IMPLEMENTATION_NOTES.md` N-005.

## Sources

- `specs/003-message-envelope-mock-crypto/spec.md ## Clarifications` Session 2026-06-03 — the surfacing of the architectural concern that prompted this ADR (post-round-5 design discussion).
- `../docs/staged-design-synthesis.md §2.3` — canonical envelope shape and the three protocol properties it enables; uses "envelope" for the whole signed message.
- `specs/ROADMAP.md §2` features 004 / 005 / 008 / 010 — the future protocol-message types that motivate the restructure.
- ADR 0009 (`docs/decisions/0009-crypto-trait-shape.md`) — the crypto trait shape decision; this ADR is compatible with it (the `Signer` / `Verifier` traits stay unchanged; only the data-type hierarchy changes).
- ADR 0006 — receive-task model that the new pattern-match shape extends.
- ADR 0007 — network handle / routing-wrapper structural shape; the 001 `Envelope` → `RoutingFrame` rename happens at this boundary.
- Saved feedback memory `feedback_message_dropped_event_convention.md` — drop-event shape, unaffected by this ADR but referenced because FR-014 / FR-015 carry the same shape after the restructure.

## Amendments

### 2026-06-23 — `Message::Signed` variant renamed to `Message::Dissemination`

The dissemination variant of `Message` is renamed `Signed` → `Dissemination` (the named handler `handle_signed_message` follows to `handle_dissemination`). The original name dates from when this was the only signed variant; once `Message::Connection` landed (004-connections, ADR 0017) both variants are signed, so "Signed" no longer distinguishes them — the meaningful axis is dissemination-vs-control, which the new name names directly. The inner type `SignedMessage` (and `PlainMessage`) are **unchanged**: this is a variant-only rename, and `SignedMessage` remains accurate (it is the signed dissemination message). The body examples above retain the original `Message::Signed` spelling as the point-in-time record. Behavior-preserving; the rename was earmarked out-of-scope in 006-fanout-policy and carried out in the connection-acceptance-strategy refactor.
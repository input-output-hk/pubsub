# ADR 0017: Key-backed peer identity and signed connection control messages

**Status**: Accepted
**Date**: 2026-06-12
**Feature**: 004-connections
**Source**: `specs/004-connections/{spec,research,data-model}.md`; `specs/004-connections/contracts/connection-protocol.md`; ADR 0009 (crypto trait shape), ADR 0010 (message hierarchy), ADR 0013 (pubkey-keyed subscription list context); `../docs/node-lifecycle/README.md` (on-chain artifacts: node pubkey → topics → deposit).

## Context

Feature 004-connections introduces application-level connection control messages
(Request / Accepted / Terminated) that are **signed by the emitting node** and handled
entirely by their carried emitter identity (spec FR-011/FR-015; the transport frame is
not consulted on the control path). Verifying such a message requires the emitter's
public key to be recoverable from the message itself — there is no key registry, and
the 003 precedent (`PlainMessage` carries `publisher_id: PublisherId(PublicKey)` inside
`signed_bytes()`) already solves exactly this problem for payload messages.

Separately, the protocol's own identity model is key-shaped: the subscription list is
keyed by node pubkey (`node-lifecycle/README.md`; ADR 0013's context). pubsub-node's
`PeerId(String)` was a 001 placeholder.

Both choices are structural (Principle III): the message layout is wire-protocol
shape, and the identity representation pervades config, network registration, the 008
registry, candidates, and every test helper.

## Decision

### 1. `PeerId` wraps `PublicKey`

`PeerId(PublicKey)` — the `PublisherId` pattern; the two stay distinct newtypes (role
distinction documented in `message.rs`). Node identity **is** a public key, matching
the protocol's pubkey-keyed subscription list.

### 2. The mock-stage alias rule is the string form

- `FromStr`: validate (non-empty, no internal NUL — unchanged rules) then derive:
  `PeerId(derive_public(&PrivateKey::new(alias_bytes)))`.
- `Display`: the inverse — strip the mock public suffix and render the alias when the
  prefix is valid UTF-8; lowercase hex otherwise. `FromStr` ∘ `Display` round-trips
  for aliases.
- serde stays string-shaped at the file level (Deserialize via FromStr, Serialize via
  Display): config files, subscription-list fixtures, and logs remain human-legible.
- `PeerId::as_str()` is removed (no stable inner string).
- The companion test convenience `MockCryptoScheme::keypair_from_alias(alias)`
  (private = alias bytes, public derived) makes alias identities sign and verify
  through the unmodified mock pair, and agree with `PeerId::from_str(alias)` by
  construction.

The alias rule is explicitly the **mock-stage** format: real crypto (feature 011)
replaces `FromStr`/`Display` with a real key encoding in one place; nothing else
about the type changes.

### 3. Control messages are self-contained signed envelopes

`Message::Connection(ConnectionMessage)` joins the ADR 0010 hierarchy as the second
variant, mirroring the plain/signed split: `ConnectionMessage { plain: PlainConnection,
signature }` with `PlainConnection { emitter: PeerId, action: ConnectionAction }` and
`ConnectionAction::{Request, Accepted, Terminated}` (each topic-carrying,
`#[non_exhaustive]` — a Rejected variant returns with the deny-path package).
`PlainConnection::signed_bytes()` uses the established length-prefixed layout
(emitter key, then action tag byte + topic), so the signature binds
**emitter + kind + topic**.

### 4. The carried emitter is the control-path identity

Verification uses the carried emitter's key; the self-check, membership validation,
entry keying, and reply addressing all read the carried emitter. The transport frame's
sender is not consulted on the control path and no frame-vs-emitter cross-check is
performed (identity-binding hardening is a recorded deferral). The payload path is the
deliberate opposite: its connection check keys on the frame's delivering peer, because
a payload message carries a *publisher* identity, not the sender's.

## Consequences

- Identity/signer coherence becomes checkable: `Node::new` validates
  `self_id.as_public_key() == signer.public_key()` (the trait already exposes
  `public_key()`, ADR 0009) and fails construction typed-and-early on mismatch.
- Blast radius is wide but mechanical: config loading, network registration maps,
  the 008 registry and `MembershipEvent`, candidates, and test helpers all flow
  through the unchanged string forms; only the type's interior changes. Fixture and
  log legibility is preserved by the Display inverse.
- Under mock crypto the binding remains symbolic (forgeable by construction — the 003
  caveat, unchanged); what this ADR buys now is the **shape** real crypto needs, with
  the swap confined to the string-form rule and the mock scheme.
- `PublisherId` and `PeerId` may wrap identical bytes for a node that both publishes
  and forwards; the type distinction (already documented) is what keeps the roles
  apart.

## Alternatives considered

- **Signature/emitter as outer unsigned fields**: rejected — a signature would be
  replayable under a different claimed emitter; the 003 pattern (identity inside the
  signed bytes) exists precisely to bind them.
- **`PublicKey(alias bytes)` without derivation**: rejected — no private key derives
  to a suffix-less public key, so the node's own id could never satisfy coherence.
- **Hex-encoded keys in files now**: rejected — destroys fixture/log readability for
  zero security gain under mock crypto.
- **Keep `PeerId(String)` + a separate key field in control messages**: rejected —
  two unanchored identity concepts and a guaranteed rework at real crypto.
- **Resolve keys from the routing frame**: rejected — contradicts the no-cross-check
  decision and requires a key registry that does not exist.

## Sources

- `specs/004-connections/spec.md` — FR-010..015, FR-024, Clarifications (emitter,
  coherence, frame-trust assumption).
- `specs/004-connections/research.md` — R1, R2, R3; `contracts/connection-protocol.md` §1.
- ADR 0009 / 0010 / 0013; `../docs/node-lifecycle/README.md`.

# Research: 004-connections

Phase 0 record. The feature was designed conversationally across nine clarify rounds
before planning; this file consolidates the planning-level decisions that the spec
deliberately left open ("fixed at planning"), each with rationale and alternatives.
No `NEEDS CLARIFICATION` items remain.

## R1 — Identity/signer coherence mechanism

**Decision**: `Node::new` keeps the explicit `self_id: PeerId` parameter, adds
`signer: Arc<dyn Signer>`, and validates `*self_id.as_public_key() == signer.public_key()`
**before** network registration, returning the new typed `NodeError::IdentityMismatch`
on failure. No trait change is needed: `Signer::public_key()` already exists
(`src/crypto/mod.rs`, ADR 0009 shape).

**Rationale**: checking before `network.register` keeps the no-leak construction
property trivially (nothing to unregister, no tasks spawned — FR-024); the existing
trait method makes the keypair-shaped-parameter alternative unnecessary.

**Alternatives considered**: (a) a `KeyPair` parameter — couples `Node::new` to the
mock factory's pair type and still needs the id comparison; (b) deriving `self_id`
from the signer (drops the parameter) — rejected by the user at clarify (Option C
there): a larger signature reshape for no additional safety once the check exists.

## R2 — `PeerId` representation and the alias rule

**Decision**: `PeerId(PublicKey)` (the `PublisherId` pattern; `PublisherId` stays a
distinct newtype — `message.rs` already documents the two roles as type-distinct even
over the same bytes). The string form is the **mock-stage alias rule**, applied
uniformly everywhere a string identity appears (node config `node_id`-equivalents,
`[[peers]]` entries, subscription-list files, test helpers):

- `FromStr`: validate as today (non-empty, no internal NUL) then
  `PeerId(derive_public(&PrivateKey::new(alias.as_bytes().to_vec())))`.
- `Display`: the inverse — if the key bytes end with the mock `PUBLIC_SUFFIX` and the
  prefix is valid UTF-8, render the prefix (the alias); otherwise lowercase hex.
  `FromStr` ∘ `Display` round-trips for aliases.
- `as_str()` is removed (no stable inner string); display formatting replaces it.
- serde: `Deserialize` via `FromStr` (unchanged pattern), `Serialize` via `Display`.

**Rationale**: a node's own `PeerId` must equal `derive_public(private)` for coherence
(R1) and for control-message verification, so alias strings must map through the same
derivation everywhere or ids would not match across config, registry, and wire. The
Display inverse keeps fixtures, logs, and assertion output as readable as today
("a" in, "a" out).

**Alternatives considered**: (a) `PublicKey(alias bytes)` without derivation — breaks
coherence (no private key derives to a suffix-less public); (b) hex-encoded keys in
files — kills fixture readability, the very property the alias scheme preserves;
(c) keeping `PeerId(String)` plus a separate key field in control messages — two
unanchored identity concepts, ripped out at real crypto. The `FromStr`/`Display`
alias rule is explicitly the mock-stage wire/file format; real crypto (feature 011)
replaces both ends with a real encoding in one place. ADR 0017 records this.

## R3 — Control-message shape

**Decision**: mirror the 003 plain/signed split (ADR 0010):

```rust
pub enum Message { Signed(SignedMessage), Connection(ConnectionMessage) } // 2nd variant

pub struct ConnectionMessage {           // sibling of SignedMessage
    pub plain: PlainConnection,          // the signed-over content
    pub signature: Signature,            // over plain.signed_bytes()
}
pub struct PlainConnection {
    pub emitter: PeerId,                 // identity INSIDE the signed content
    pub action: ConnectionAction,
}
#[non_exhaustive]
pub enum ConnectionAction {
    Request   { topic: TopicId },
    Accepted  { topic: TopicId },
    Terminated{ topic: TopicId },
}
```

`PlainConnection::signed_bytes()` reuses the hand-rolled length-prefixed scheme of
`PlainMessage::signed_bytes()`: emitter key bytes (u32 length-prefixed), then a 1-byte
action tag (`0x00` Request, `0x01` Accepted, `0x02` Terminated) followed by the topic's
UTF-8 bytes (u32 length-prefixed). The signature therefore binds emitter + kind + topic
(FR-011). `ConnectionAction` is `#[non_exhaustive]` (a `Rejected` variant returns with
the deny-path package — the ROADMAP-justified forward shape).

**Alternatives considered**: signature/emitter as outer unsigned fields — rejected
(signature replayable under a different claimed emitter; clarified at spec).

## R4 — Event and effect vocabulary

**Decision**:

- `Event::ConnectionSetup` — the setup trigger (timer-produced or externally pushed);
  unit variant.
- `Event::Shutdown` — graceful-teardown trigger; doubles as the event loop's terminal
  marker.
- Control messages arrive inside the existing `Event::MessageReceived` and dispatch in
  `handle_message_received` on `Message::Connection` (no new event variant; named
  handlers per ADR 0011: `handle_connection_message` → `handle_connection_request` /
  `handle_connection_accepted` / `handle_connection_terminated`).
- `Effect` gains its first variants:
  - `Effect::Send { to: PeerId, message: Message }` — one generic wire-send effect
    (request, acceptance, termination notices all reduce to it; the executor is a
    single arm).
  - `Effect::Misbehaved { peer: PeerId, topic: TopicId, cause: &'static str }` — the
    semantic misbehavior signal; the executor logs it (`event = "connection_severed"`,
    warn level) and nothing else in this feature. The future blacklist consumes this
    variant without reshaping `apply`'s output (the spec's stated design intent).

**Alternatives considered**: per-action send effects (`SendRequest`/`SendAccepted`/…)
— three executor arms doing the same thing; rejected as noise. A `Shutdown` effect
instead of shell-side loop break — the break is loop lifecycle, not state semantics;
ADR 0019 records the carve-out.

## R5 — Strategy seam

**Decision**: `src/connection.rs` hosts:

```rust
pub trait ConnectionStrategy: Send + Sync {
    /// Pure, synchronous: the expected upstream set given the node's view.
    fn expected_upstream(
        &self,
        subscriptions: &HashSet<TopicId>,
        candidates: &HashMap<TopicId, HashSet<PeerId>>,
    ) -> HashSet<(PeerId, TopicId)>;
}
pub struct ConnectToAllCandidates;   // v1 policy
```

Injection point: a `strategy: Arc<dyn ConnectionStrategy>` **field on `NodeState`**
beside the verifier — same precedent (the immutable service handle a transition
consults), keeps `apply`'s signature unchanged, and the strategy is reachable from
the `ConnectionSetup` arm without threading parameters. `Node::new` takes it as a
new parameter (`Arc<dyn ConnectionStrategy>` — sync trait, dyn-compatible, matching
`Arc<dyn Verifier>`). ADR 0018 records the seam.

**Alternatives considered**: threading the strategy into `apply` as a parameter —
changes the transition signature every feature that adds a service; a generic
`Node<S: ConnectionStrategy>` — infects every consumer's type for no v1 benefit
(config-driven instantiation later wants dyn anyway).

## R6 — Setup timer and config field

**Decision**: TOML field `connection_setup_delay_ms: Option<u64>` →
`NodeConfig.connection_setup_delay: Option<Duration>` (loader converts; parse at the
edge). Default `None` — nothing spawned, no event self-generated. When `Some(d)`,
`Node::new` registers a third node-owned producer via the existing `spawn_producer`
(named async fn `setup_timer_producer`: `sleep(d)` then one `push(Event::ConnectionSetup)`
and return) — owned, drop-aborted, exactly the producer discipline ADR 0012 set.

**Alternatives considered**: a free-floating ephemeral task — loses the drop-abort
ownership for free; milliseconds vs. seconds — ms matches `Timestamp`'s unit choice.

## R7 — Receive-path order and drop causes

**Decision**: `handle_signed_message` prepends the connection check (frame sender,
per FR-016), keeping the existing order after it; the signature-failure arm — when
reached over an Active upstream — additionally removes the upstream entry and emits
`Effect::Misbehaved` (FR-017). New `message_dropped` causes (snake_case, FR-025):

| cause | path |
|---|---|
| `not_connected` | payload from a sender without an Active upstream for its topic |
| `membership_validation_failed` | Request failing FR-012's membership gate |
| `unsolicited_accept` | Accepted with no matching AwaitingAccept |
| `unknown_termination` | Terminated for a connection not held |
| `self_emitter` | control message whose carried emitter is the node itself |
| `invalid_signature` | control message failing verification (reuses the existing cause value on a new path) |

Existing causes (`topic_not_subscribed`, `invalid_signature` on the payload path)
are unchanged — the regression boundary (FR-019).

## R8 — Shutdown mechanics

**Decision**: `pub async fn shutdown(mut self)`: push `Event::Shutdown`;
`handle_shutdown` clears both structures and returns one
`Effect::Send { to, message: Terminated }` per entry (both roles, any state, FR-020);
the event loop executes the effects then `break`s when the drained event was
`Shutdown`; `shutdown` awaits `(&mut self.event_loop)` (`JoinHandle` is `Unpin`;
`Node` has a `Drop` impl so fields cannot be moved out), logs-and-ignores a
`JoinError`, and lets `self` drop (producers aborted as today). `Drop` itself is
unchanged. The effect executor needs the network send half inside the loop task: the
loop closure captures a clone of the handle's crate-internal `NetworkSender` (it is
`Clone`; a small `NetworkHandle::sender()` `pub(crate)` accessor exposes it). ADR 0019.

## R9 — Test identity and script builders

**Decision**: `MockCryptoScheme::keypair_from_alias(&self, alias: &str) -> KeyPair`
(does not advance the RNG, like `signer()`/`verifier()`): private = alias bytes,
public = `derive_public` — alias identities sign and verify through the unmodified
`TestSigner`/`TestVerifier`, and `KeyPair.public` equals `PeerId::from_str(alias)`'s
inner key by construction. State-machine tests get a `pub(crate) ConnectionScript`
builder in `src/connection.rs`'s test-support (sibling of `MembershipScript`,
constitution v1.2.0): one chained step per line producing `Vec<Event>` covering
membership, control-message, setup, and shutdown steps. `tests/common` gains
`await_connection`-style helpers and an establishment-preamble fixture mirroring
`await_subscriptions`.

## R10 — Operator-facing severance signal

**Decision**: the misbehavior executor logs `event = "connection_severed"` (warn)
with `peer`, `topic`, `cause` fields — implementation-neutral wording, no FR
citations; logs stay a non-test surface (correctness is asserted via the connection
getters and `apply`'s returned effects).

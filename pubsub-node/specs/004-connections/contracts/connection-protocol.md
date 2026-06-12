# Contract: connection control protocol and public surface (004-connections)

The externally observable contract this feature adds: the wire-level control
messages, the validation/drop vocabulary, and the library's public-surface changes.
Implementation-internal shapes (NodeState fields, Effect variants) are in
`data-model.md`; this file is what another implementation or a consumer would code
against.

## 1. Wire surface (over the unchanged peer-addressed network)

A third protocol message kind alongside the signed dissemination message:

```text
Message::Connection(ConnectionMessage)
ConnectionMessage = { plain: PlainConnection, signature: Signature }
PlainConnection   = { emitter: PeerId, action: ConnectionAction }
ConnectionAction  = Request{topic} | Accepted{topic} | Terminated{topic}   (non-exhaustive)
```

### 1.1 Canonical signing bytes — `PlainConnection::signed_bytes()`

Hand-rolled, length-prefixed concatenation in the `PlainMessage::signed_bytes()`
style; multi-byte integers big-endian; no version tag. Fields, in order:

1. emitter key — `u32` byte length, then the emitter's public-key bytes.
2. action — 1 tag byte, then the topic as `u32` byte length + UTF-8 bytes.

Action tags are assigned explicitly: `0x00` Request, `0x01` Accepted,
`0x02` Terminated. Future variants (e.g. a Rejected) append new tag values.

The signature is produced by the emitting node's signer over exactly these bytes —
it binds emitter identity, action kind, and topic together. Any layout change is a
protocol change and must update this section in the same commit.

### 1.2 Identity rules

- **Control path**: every handling decision — the self-check, membership validation,
  the (peer, topic) an entry is recorded under, and the addressing of an `Accepted`
  reply — uses the **carried emitter**. The transport frame's sender is not consulted,
  and no frame-vs-emitter cross-check is performed (deferred to identity-binding
  hardening).
- **Payload path**: the connection check uses the **transport frame's delivering
  peer** (a payload message carries a publisher identity, not the sender's).
- `PeerId` is key-backed. Its string form is the mock-stage **alias rule**:
  parsing derives the key from the alias; display renders the alias back. All
  identity strings in TOML (peer entries, subscription-list node ids) and test
  helpers go through the same rule, so ids agree across config, registry, and wire.

## 2. Handshake and lifecycle semantics

| Message | Receiver obligation |
|---|---|
| `Request{T}` from E | After control checks: accept iff T ∈ own topics AND E ∈ candidates[T]; record downstream (E,T); send `Accepted{T}`. Duplicate of a held entry: re-validate, keep entry, re-send `Accepted`. Failing validation: drop, no reply, no state change. |
| `Accepted{T}` from E | Iff upstream (E,T) is `AwaitingAccept`: transition to `Active`. Otherwise drop, no state change. |
| `Terminated{T}` from E | Iff an entry (E,T) is held (either role): remove it. Otherwise drop. Never replied to. |

Establishment is initiated only by the setup event (optional one-shot timer, unset
by default, or external injection through the public event intake), applied as a
diff: dial everything expected that is not `Active` (pending pairs are re-dialed;
nothing is ever removed by selection). Misbehavior severance (invalid payload
signature over an `Active` upstream, having passed the subscription filter) removes
the upstream entry silently — no `Terminated` is sent. Graceful shutdown sends one
`Terminated` per held entry, both roles, any state.

## 3. Drop-event vocabulary (operator-facing, `message_dropped` convention)

All new drops emit the established `message_dropped` event (info level) with a
snake_case `cause`; the misbehavior severance additionally emits
`connection_severed` (warn) with `peer`, `topic`, `cause`. Logs are operator UX —
never a test surface.

| cause | emitted when |
|---|---|
| `not_connected` | payload from a sender without an Active upstream for its topic |
| `topic_not_subscribed` | (existing) admitted-connection payload outside the subscription set |
| `invalid_signature` | (existing value) payload or control message failing verification |
| `membership_validation_failed` | Request failing the membership gate |
| `unsolicited_accept` | Accepted with no matching pending entry |
| `unknown_termination` | Terminated for a connection not held |
| `self_emitter` | control message whose carried emitter equals the receiving node |

## 4. Public library surface (delta)

### Changed

- `PeerId`: now wraps a public key; `as_str()` **removed**; `FromStr`/`Display`
  follow the alias rule; serde formats unchanged at the file level (strings in,
  strings out).
- `Node::new(self_id, config, network, verifier, registry)` →
  `Node::new(self_id, config, network, signer, verifier, registry, strategy)`:
  - `signer: Arc<dyn Signer>` — the node's signing identity; construction fails
    (typed `NodeError::IdentityMismatch`, no background activity) when
    `self_id` does not match `signer.public_key()`. Checked before network
    registration; duplicate-registration failure behavior unchanged.
  - `strategy: Arc<dyn ConnectionStrategy>` — the connection-selection policy
    (v1: `ConnectToAllCandidates`).
- `NodeConfig`: new optional field, TOML `connection_setup_delay_ms` (u64 ms) →
  `connection_setup_delay: Option<Duration>`; absent by default (no autonomous
  establishment).

### Added

- `pub async fn Node::shutdown(self)` — graceful teardown: drains queued events,
  notifies every connection counterpart, then releases the node. Plain drop remains
  the abrupt, notice-less path.
- Getters: `Node::upstream_connections() -> Vec<(PeerId, TopicId, UpstreamState)>`,
  `Node::downstream_connections() -> Vec<(PeerId, TopicId)>` — lock-and-clone
  snapshots (the `received_messages()` pattern).
- Re-exports: `UpstreamState`, `ConnectionStrategy`, `ConnectToAllCandidates`,
  `ConnectionMessage`, `PlainConnection`, `ConnectionAction` (alongside the existing
  message types); `MockCryptoScheme::keypair_from_alias` (mock test convenience).

### Unchanged but consumed

- The pre-existing event-intake surface (`Node::events()` / `EventQueue::push` /
  `Node::spawn_producer`) is untouched; this feature consumes it for the setup event
  (the scripted establishment path) without altering its semantics (FR-022).

### Explicitly absent (spec scope boundaries)

- No manual connect/disconnect verbs (FR-022).
- No `Rejected` message, no acceptance policy beyond the fixed membership validation,
  no reconnection/GC/blacklist (FR-027); no transport changes (FR-028).

## 5. Verification note (Development Workflow)

The post-implementation analyze pass must verify this contract's public-surface
claims against `lib.rs` re-exports and module visibility (the 003 lesson) — in
particular the re-export list above, the removal of `PeerId::as_str`, and that
`NodeState`/`Effect` remain crate-internal.

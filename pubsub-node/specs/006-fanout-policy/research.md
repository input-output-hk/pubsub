# Research — 006 Message Publishing and Fan-out Forwarding

Phase 0 planning decisions. Each is `Decision / Rationale / Alternatives`. They are binding inputs to `/speckit-tasks`; tactical details (exact names, log strings) are recorded here and in `contracts/` and are exempt from ADR treatment.

## R1 — `FanoutStrategy` seam shape

**Decision**: A pure, synchronous trait mirroring `ConnectionStrategy`:

```rust
pub trait FanoutStrategy: Send + Sync {
    /// The downstream peers that receive a forward of a message on `topic`.
    /// `exclude` is the delivering peer to skip (split-horizon) on the receive
    /// path, or `None` on the publish path.
    fn targets(
        &self,
        topic: &TopicId,
        downstream: &HashSet<(PeerId, TopicId)>,
        exclude: Option<&PeerId>,
    ) -> Vec<PeerId>;
}
```

The v1 implementor `ForwardToAll` returns every `peer` for which `(peer, topic) ∈ downstream` and `Some(peer) != exclude`. Held on `NodeState` as `Arc<dyn FanoutStrategy>` beside `strategy`/`verifier`/`signer`; injected as the new last parameter of `Node::new`.

**Rationale**: Symmetric with the connection seam (ADR 0018) — same purity, same `Arc<dyn>`-at-storage shape, same "the trait is the variation point future strategies replace" intent. Taking the whole `downstream` set + `topic` + `exclude` keeps the strategy free to implement degree caps / sampling later without a signature change. Justified forward shape per the constitution: ROADMAP 006/007 name pick-k and golden-mode fan-out as the consumers.

**Alternatives**: (a) a free function `fanout_targets(...)` — rejected: no seam for the ROADMAP-named variants, and inconsistent with the connection side. (b) passing only the topic's downstream peers (pre-filtered) — rejected: marginally simpler signature but moves topic-scoping out of the strategy, which a future topology policy may want to own.

## R2 — Dedup key and store

**Decision**: `seen: HashSet<MessageHash>` on `NodeState`, keyed on `MessageHash::of(&signed.plain)` (the content-anchored hash, already `#[derive(Hash, Eq, PartialEq)]`). Unbounded.

**Rationale**: Content hashing is the honest loop-prevention key — it dedups *identical* messages without conflating distinct messages that an equivocating publisher emits under the same `(publisher, sequence)` (those have different content → different hash → both propagate, which is the documented out-of-scope stance). `MessageHash::of` already exists and is content-anchored (N-005), so no new hashing surface. Unbounded is correct for the in-memory PoC; bounding is a real-impl concern (deferred, see ADR 0020 + N-new).

**Alternatives**: (a) `(publisher_id, sequence)` key — rejected: silently collapses equivocation to first-seen-wins, hiding a conflict the project tracks separately (N-003/012). (b) `LruCache` now — rejected: premature; an eviction policy is a deployment tuning concern with no PoC consumer, and an unbounded set keeps tests deterministic.

## R3 — Where dedup sits in each path

**Decision**: Dedup is the **last** gate before recording, applied identically on both paths via the shared fan-out helper's caller:

- Receive path (`handle_signed_message`): connection gate → subscribed → registered → authorized → **signature** → **dedup** → record + insert-seen + fan-out.
- Publish path (`handle_publish`): subscribed → registered → authorized → **signature** → **dedup** → record + insert-seen + fan-out (no connection gate; no severance).

**Rationale**: Placing dedup after signature verification means a message that fails verification never enters `seen` (FR-013) — so a forged duplicate cannot poison the set and suppress a later genuine message of the same content (there can be none: same content ⇒ same validity). Placing it at the record point keeps a single record-and-forward site reused by both handlers.

**Alternatives**: dedup *first* (before the check chain) — rejected: would let an unverified hash enter `seen`, and the early-exit saving is irrelevant in the in-memory model.

## R4 — `Publish` event and `handle_publish`

**Decision**: `Event::Publish(SignedMessage)` (new `#[non_exhaustive]` variant), dispatched by `apply` to a named `handle_publish`. `Node::publish(&self, message: SignedMessage)` is fire-and-forget: it pushes the event onto the queue (`EventQueue::push`, already infallible) and returns `()`. No `Result`.

**Rationale**: A typed event + named handler matches the per-variant dispatch discipline (ADR 0011). Fire-and-forget is forced by the event-queue architecture: validation happens later in the loop, so a synchronous verdict would require splitting validation out of the handler. The caller observes success via `received_messages()` and failures via the `message_dropped` log convention — exactly the receive-path contract. A publish never severs (no upstream to sever) — an invalid-signature publish is a plain drop.

**Alternatives**: (a) synthesize `Event::MessageReceived { from: self }` — rejected: the receive handler runs the connection gate first and the node is never its own Active upstream, so a self-routed message would drop at `not_connected`. (b) `publish` returns `Result` after pre-enqueue validation — rejected: duplicates the validation logic across a sync pre-check and the handler, and breaks the single-transition-owns-validation invariant.

## R5 — `Origin` on `ReceivedDelivery`

**Decision**: Replace `ReceivedDelivery.from: PeerId` with `origin: Origin`, `enum Origin { Local, Peer(PeerId) }`. `Local` for a locally-published message; `Peer(id)` for the forwarding peer of a received message. The publisher identity stays inside `message.plain.publisher_id`.

**Rationale**: A locally-published message has no wire sender, so a `PeerId`-typed `from` has no honest value for it (overloading `self_id` conflates two roles, and `self_emitter` is already a *drop* cause on the control path). Modelling origin explicitly is the minimal correct shape and corrects the pre-existing doc drift (the field's rustdoc said "originated" but stored the *forwarding* peer). `Origin` lives in `received.rs` beside `ReceivedDelivery`.

**Alternatives**: (a) `from: Option<PeerId>` (`None` = local) — rejected: an `Option` reads as "maybe unknown," not "local origin"; the named enum is clearer and extensible. (b) keep `from: PeerId`, set to `self_id` for local — rejected as above.

## R6 — Shared fan-out helper

**Decision**: One crate-internal pure helper in `state.rs`:

```rust
fn fanout(state: &NodeState, topic: &TopicId, message: &SignedMessage, exclude: Option<&PeerId>) -> Vec<Effect>
```

returning one `Effect::Send { to, message: Message::Signed(message.clone()) }` per target the strategy selects. Called by `handle_signed_message` (after recording, `exclude = Some(&from)`) and `handle_publish` (after recording, `exclude = None`). Forwarding is **verbatim** — the helper clones the original `SignedMessage`, never re-signs.

**Rationale**: A single helper keeps the two paths' forwarding identical and re-uses the existing `Effect::Send` executor arm (no new effect variant). Verbatim forwarding preserves the publisher's end-to-end signature — relays are not signing authorities for dissemination (contrast control messages, which the node *does* sign as emitter).

**Alternatives**: inlining fan-out into each handler — rejected: duplicates the split-horizon/target logic and risks the two paths drifting.

## R7 — Test topology construction (per Clarifications 2026-06-16)

**Decision**: Dissemination integration tests cover both shapes. **Full-mesh** tests use the natural 004 path (registry + setup → `ConnectToAllCandidates`) and assert dedup absorbs the redundant-delivery storm. **Partial/line** tests are built by scripting `Request`/`Accepted` control messages through the public event intake (the mechanism `connections.rs` already uses), shaping a topology where a node receives a message *only* via relay. A test-only no-op `FanoutStrategy` lives in `fanout::test_support` (cfg-gated, never in the production surface) for connection-lifecycle suites where fan-out is irrelevant noise.

**Rationale**: A full mesh masks relay correctness (every node also gets a direct copy); a partial topology isolates relay as the sole delivery path. Both are constructible with existing machinery. The no-op strategy keeps `connections.rs`-style suites focused without disabling the feature in production.

**Alternatives**: full-mesh only — rejected (masks relay, US2); a production "forward to nobody" strategy — rejected (test-shaped production code; could ship the feature switched off).

## R8 — Not parity-preserving (chartered rework)

**Decision**: Receive-path unit tests in `state.rs` keep their assertions (their `downstream` is empty ⇒ `ForwardToAll` is a no-op); the only mechanical change is the shared `node_state` test constructor gaining the fan-out-strategy argument. Dissemination integration suites are reworked to assert forwarding. This rework is chartered work (spec US1/US2/US3, SC-001..006), not collateral damage.

**Rationale**: Same posture 004 established and the crate CLAUDE.md records. The empty-downstream invariant makes the unit-test blast radius a one-line helper change.

**Alternatives**: a `FanOutToNobody` production default to preserve every test verbatim — rejected (see R7).

## R9 — Shared validation/record-and-fanout factoring (publish ∩ receive)

**Decision**: The publish and signed-receive paths share their middle and tail; factor that into two pure crate-internal helpers in `state.rs`, leaving only the genuinely path-specific bits in each handler:

```rust
// subscribed → registered → authorized; returns the drop cause, or None if all pass.
fn validate_dissemination(state: &NodeState, plain: &PlainMessage) -> Option<&'static str>;

// dedup-check → record ReceivedDelivery{origin,..} → seen.insert → fanout(exclude).
fn record_and_fanout(state: &mut NodeState, signed: SignedMessage, origin: Origin, exclude: Option<&PeerId>) -> Vec<Effect>;
```

Resulting handlers are thin:
- `handle_signed_message`: connection gate → `validate_dissemination` → verify (**fail ⇒ sever**: `Effect::Misbehaved` + remove upstream) → `record_and_fanout(Origin::Peer(from), Some(&from))`.
- `handle_publish`: `validate_dissemination` → verify (**fail ⇒ plain drop**, no upstream to sever) → `record_and_fanout(Origin::Local, None)`.

**Path-specific bits that deliberately stay out of the helpers**: the connection gate (receive-only), the signature-failure *action* (sever vs plain drop), the `Origin` value, and the fan-out `exclude`. Drop **logging** also stays in each handler — `validate_dissemination` returns the cause; the caller logs it with path-appropriate fields (e.g. `from=` on receive) — so observability stays per-path while the decision logic is shared once.

**Rationale**: two concrete call sites with an identical 3-check chain and an identical dedup→record→fanout tail is genuine duplication (not speculative), so DRY is justified by real consumers. Factoring at these seams keeps each handler readable and the shared correctness logic single-sourced, without merging two dispatch handlers (the named-handler-per-dispatch-level discipline is preserved — these are sub-helpers, not a dispatch merge).

**Scope note**: `handle_signed_message` is existing merged 004/013 code; this refactors it to call the shared helpers — a deliberate touch of working code, already inside the not-parity-preserving charter. Tactical/local (reversal = inline rewrite in `state.rs`), so no ADR.

**Alternatives**: (a) one merged handler with a `bool is_publish` flag — rejected: branchy, obscures the two dispatch entries, fights the named-handler discipline. (b) leave both inline (no extraction) — rejected: duplicates the check chain and the dedup/record/fanout tail, two places to drift.

## Cross-cutting

- **No new dependencies** — `HashSet`, the existing `MessageHash`, `Arc<dyn>` only. No justified-dependency ADR needed.
- **Determinism** — `ForwardToAll` is deterministic in *which* peers (set-derived); target *order* is unspecified (set iteration), so tests sort before asserting, as the connection-effect helpers (`request_sends`, `sorted_pairs`) already do. No RNG enters state (pick-k, which would, stays out of scope).
- **Logs** — dedup drop `cause = "duplicate"`; publish validation failures reuse the receive-path causes (`topic_not_subscribed`, `topic_not_registered`, `publisher_not_authorized`, `invalid_signature`). Operator UX, never asserted (constitution).

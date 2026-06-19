# ADR 0021: Fan-out strategy seam, content-hash dedup, and message origin

**Status**: Accepted
**Date**: 2026-06-16
**Feature**: 006-fanout-policy
**Source**: `specs/006-fanout-policy/{spec,research,data-model,contracts}.md` (FR-001..016, R1–R8); ADR 0018 (strategy-seam precedent), ADR 0011 (pure core), ADR 0012 (producer ownership), ADR 0010 (`Message` hierarchy), ADR 0009 (`Arc<dyn …>` service-handle / content-anchored hash, N-005); `specs/ROADMAP.md` 006/007 (fan-out policy consumers).

## Context

004 built the connection topology (per-`(peer, topic)` upstream sources + downstream sinks) but no traffic flows over it. This feature makes a node (a) originate a dissemination message via a local publish, and (b) forward messages — published or received — to its downstream peers, with loop suppression so a cyclic mesh does not circulate a message forever.

Three decisions are structural — they shape `Node::new`'s signature, the public `received_messages()` type, the event set, and where the forwarding-policy variation point lives — so they are recorded here rather than left tactical:

1. How the fan-out target selection is made swappable (ROADMAP 006 pick-k, 007 golden mode are the named consumers).
2. How duplicates are detected (the key choice interacts with equivocation, which the project tracks separately).
3. How a recorded delivery represents "locally published" when there is no wire sender.

## Decision

### 1. `FanoutStrategy` — a sync, pure trait, `Arc<dyn>`-injected, stored on `NodeState`

```rust
pub trait FanoutStrategy: Send + Sync {
    fn targets(
        &self,
        topic: &TopicId,
        downstream: &HashSet<(PeerId, TopicId)>,
        exclude: Option<&PeerId>,
    ) -> Vec<PeerId>;
}
pub struct ForwardToAll;   // v1: every downstream peer on the topic, minus `exclude`
```

This is the **deliberate twin of ADR 0018's `ConnectionStrategy`**: same purity, same dyn-compatible shape (no async, no associated types — the ADR 0009 shape), same `Arc<dyn>`-at-storage stored beside the verifier/signer/connection-strategy service handles. `Node::new` gains `fanout_strategy` as its final parameter; `apply`'s signature is unchanged. The trait takes the whole `downstream` set + `topic` + `exclude` so a future degree-cap/sampling policy varies behind it without a signature change. Lives in a new `fanout` module (parallel to `connection`), with a `#[cfg(test)]` no-op strategy in `fanout::test_support` — never in the production surface.

### 2. Fan-out is verbatim and reuses `Effect::Send`

A shared pure helper, called from both `handle_signed_message` (after recording, `exclude = Some(sender)` — split-horizon) and `handle_publish` (after recording, `exclude = None`), emits one `Effect::Send { to, Message::Signed(original.clone()) }` per target. Forwarding **clones**, never re-signs: the publisher's end-to-end signature is preserved (relays are not signing authorities for dissemination — the contrast with control messages, which the node signs as emitter, ADR 0017). No new `Effect` variant.

### 3. Content-hash dedup, after verification, on both paths

`NodeState` holds `seen: HashSet<MessageHash>`, keyed on `MessageHash::of(&signed.plain)` (the content-anchored hash, N-005). The check is the **last gate before recording**, on both the publish and receive paths: already-seen ⇒ drop (`duplicate`); first-seen ⇒ record + insert + fan-out. Placed after signature verification so a failed-verification message never enters `seen` (no poisoning), and at the single record-and-forward site reused by both handlers. The set is **unbounded** in the in-memory model.

### 4. `Publish` event + fire-and-forget `Node::publish`

`Event::Publish(SignedMessage)` (new `#[non_exhaustive]` variant) dispatched to a named `handle_publish`, which runs the receive-path checks **minus the connection gate** (a published message has no upstream) and **minus severance** (nothing to sever). `Node::publish(&self, SignedMessage) -> ()` pushes the event and returns; validation and its `message_dropped` outcomes happen in the handler. Publishing does **not** require `publisher_id == self_id` — authorization + signature are the only authenticity gates, which enables proxy/injection of an external publisher's pre-signed message.

### 5. `Origin` on `ReceivedDelivery`

`ReceivedDelivery.from: PeerId` becomes `origin: Origin`, `enum Origin { Local, Peer(PeerId) }`. A locally-published message has no wire sender, so `Local` is the honest value; `Peer(id)` names the forwarding peer of a received message. The publisher identity stays in the message envelope. This also corrects pre-existing rustdoc drift (the old field said "originated" but stored the forwarding peer).

## Consequences

- The whole publish/relay/dedup decision is exercisable in synchronous `state.rs` tests; `ForwardToAll` is testable as a pure function with zero node machinery.
- 006's pick-k and 007's golden-mode fan-out are future `FanoutStrategy` implementors — no new seam. Pick-k must bring a seeded RNG into state to keep `apply` deterministic; that is why it is out of scope here (the v1 `ForwardToAll` is deterministic; target order is unspecified, so tests sort).
- `received_messages()` is a public-surface change (the `Origin`-tagged delivery); existing readers of `.from` are updated. This is chartered, not collateral.
- Equivocation stays detectable later, not masked: distinct content ⇒ distinct hash ⇒ both copies propagate and are recorded (N-003 / feature 012 owns conflict detection).
- The unbounded `seen` set is a known PoC simplification; a bounded (LRU/TTL) store is a real-impl concern, recorded in `IMPLEMENTATION_NOTES.md`.

## Alternatives considered

- **A free `fanout_targets(...)` function instead of a trait**: rejected — no seam for the ROADMAP-named pick-k/golden consumers, and asymmetric with the connection side.
- **Dedup keyed on `(publisher_id, sequence)`**: rejected — collapses equivocation to first-seen-wins, hiding a conflict the project surfaces elsewhere.
- **Bounded `seen` (LRU) now**: rejected — premature; eviction is deployment tuning with no PoC consumer, and unboundedness keeps tests deterministic.
- **`publish` returns `Result` after a synchronous pre-check**: rejected — duplicates validation across a pre-check and the handler and breaks the single-transition-owns-validation invariant; fire-and-forget matches the queue architecture.
- **`from: Option<PeerId>` (None = local)** or **`from = self_id` for local**: rejected — an `Option` reads as "maybe unknown" and `self_id` conflates two roles; the named `Origin` enum is clearer and extensible.
- **A production `FanOutToNobody` strategy to preserve every existing test verbatim**: rejected — test-shaped production code that could ship the feature switched off; a `#[cfg(test)]` no-op in `test_support` serves the lifecycle suites instead.

## Sources

- `specs/006-fanout-policy/spec.md` — FR-001..016, US1–US3, Clarifications 2026-06-16.
- `specs/006-fanout-policy/research.md` — R1–R8; `data-model.md` §1–§7; `contracts/fanout-protocol.md`.
- ADR 0009 / 0010 / 0011 / 0012 / 0017 / 0018; `specs/ROADMAP.md` entries 006, 007; `IMPLEMENTATION_NOTES.md` N-003, N-005.

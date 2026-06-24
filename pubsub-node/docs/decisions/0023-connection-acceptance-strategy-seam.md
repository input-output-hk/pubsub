# ADR 0023: Connection-acceptance strategy seam

**Status**: Accepted
**Date**: 2026-06-23
**Feature**: connection-acceptance-strategy refactor (behavior-preserving; not a Spec Kit feature)
**Source**: ADR 0018 (`ConnectionStrategy` strategy-seam precedent), ADR 0021 (`FanoutStrategy`, the second seam), ADR 0017 (connection model + acceptance rule), ADR 0011 (pure core), ADR 0009 (`Arc<dyn …>` service-handle shape); 004-connections `handle_connection_request` (the hardcoded logic this extracts); `specs/ROADMAP.md` 006/007 (degree-cap / sampling policy consumers).

## Context

004-connections introduced two injected, pure, `Arc<dyn>`-stored strategy seams that future iterations vary: `ConnectionStrategy` (ADR 0018 — which upstreams to dial on a setup event) and, in 006, `FanoutStrategy` (ADR 0021 — which downstream peers to forward a recorded message to). The third decision of the connection triad — **whether to accept an inbound connection `Request`** — stayed hardcoded inside `handle_connection_request`: a fixed membership predicate (`topic_is_own && emitter_is_member`) with no injection point.

This is the inbound mirror of the dial decision and the natural third member of the family. Extracting it makes the accept-side policy swappable on the same terms as the other two (degree caps, allowlists, rate limits at ROADMAP 006/007), and makes the decision unit-testable in isolation. The extraction is **behavior-preserving**: the v1 default reproduces the hardcoded predicate exactly.

## Decision

### 1. `ConnectionAcceptanceStrategy` — a sync, pure trait, `Arc<dyn>`-injected, stored on `NodeState`

```rust
pub trait ConnectionAcceptanceStrategy: Send + Sync {
    fn accepts(
        &self,
        emitter: &PeerId,
        topic: &TopicId,
        subscriptions: &HashSet<TopicId>,
        candidates: &HashMap<TopicId, HashSet<PeerId>>,
    ) -> bool;
}
pub struct AcceptFromAllCandidates;   // v1: accept every membership-valid request
```

The **deliberate inbound twin of ADR 0018's `ConnectionStrategy`**: same purity (no I/O, no RNG — `apply` stays deterministic, ADR 0011), same dyn-compatible shape (no async, no associated types — the ADR 0009 shape), same `Arc<dyn>`-at-storage beside the verifier/signer/connection-strategy/fanout-strategy service handles. `Node::new` gains `acceptance_strategy` as its final parameter; `apply`'s signature is unchanged. Lives in a new `acceptance` module (parallel to `connection` and `fanout`).

### 2. Narrow borrows, not `&NodeState`

The trait takes the `subscriptions` set and `candidates` map by reference — the same inputs the dial-side `ConnectionStrategy::expected_upstream` takes — rather than `&NodeState`. `NodeState` is `pub(crate)` with private fields, so a `&NodeState` parameter on a `pub` trait would either leak internals or force a getter surface, and it would couple the public seam to the struct's layout. Narrow borrows also make the decision's input surface an explicit, least-privilege contract (it structurally cannot read `seen`, `synced`, …). When a future strategy's input grows past what narrow borrows carry comfortably (e.g. degree caps needing `downstream`/`upstream`, or peer-sampling views at ROADMAP 005), the agreed evolution is a curated read-only context struct with a `NodeState` builder — introduced across all three seams together — not `&NodeState`.

### 3. The strategy owns the *policy*; the handler keeps the *mechanics* and the log

`accepts` returns `bool`. `handle_connection_request` consults it and, on `false`, emits the existing `message_dropped` / `cause = "membership_validation_failed"` info event and returns; on `true`, it performs the existing idempotent `downstream` insert and the signed `Accepted` reply. Logging stays in the handler — the strategy emits nothing, matching the two existing seams and the Constitution's "logs are operator UX, owned by the node, not policy" stance. A single hardcoded drop cause stays accurate while membership is the only rejection reason (the v1 default). When a later strategy introduces distinct rejection reasons, the evolution is for `accepts` to return a structured reason (e.g. `Result<(), RejectCause>`) the handler maps to the cause — deferred until a consumer needs it.

### 4. `AcceptFromAllCandidates` — the v1 default reproduces the hardcoded predicate verbatim

```rust
subscriptions.contains(topic)
    && candidates.get(topic).is_some_and(|peers| peers.contains(emitter))
```

The name mirrors `ConnectToAllCandidates`: the "all" is **membership-scoped**, not unconditional — registration gates delivery, not acceptance (the S7 pin), so the seam reads the membership-derived view only. A genuinely unconditional `AcceptsAll` (accept non-members) would change behavior — it belongs to a future feature, not this refactor.

## Consequences

- `Node::new` takes a fifth `Arc<dyn …>` collaborator (signer/verifier aside). The parameter list is now long enough that a config/builder struct is the natural future refactor, noted at the constructor.
- The acceptance decision is unit-tested in `acceptance::tests` in isolation from the node, like `fanout`/`connection`.
- Behavior is unchanged: the dissemination/connection integration suites pass unmodified beyond the new constructor argument; operator log output (the `membership_validation_failed` drop) is byte-for-byte identical.
- The triad is now symmetric: `connection_strategy` (dial) / `acceptance_strategy` (accept) / `fanout_strategy` (forward), each a pure injected seam. ROADMAP 006/007 degree-cap and sampling policies slot in behind whichever seam they constrain.

## Alternatives considered

- **Leave the logic hardcoded.** Rejected: the other two connection decisions are seams; the asymmetry was incidental, and downstream features (006/007) need accept-side variation.
- **`&NodeState` parameter.** Rejected — see Decision 2 (visibility coupling + least-privilege).
- **Log inside the strategy / return a structured reason now.** Deferred — see Decision 3 (single reason today; `bool` is the right altitude until a consumer needs distinct causes).
- **Name the default `AcceptsAll`.** Rejected — see Decision 4 (would imply unconditional acceptance, a behavior change).

## Sources

- ADR 0018 — `ConnectionStrategy` seam; this ADR is its inbound mirror.
- ADR 0021 — `FanoutStrategy`, the second seam; same purity / `Arc<dyn>` shape.
- ADR 0017 — connection model and the membership-based acceptance rule (the S7 pin) this extracts.
- ADR 0011 — pure deterministic `apply`; the strategy carries no RNG/I/O.
- ADR 0009 — `Arc<dyn …>` service-handle shape (no async, no associated types).
- `specs/ROADMAP.md` 006/007 — degree-cap / sampling policy consumers; 005 (PeerView) — the context-struct evolution trigger.

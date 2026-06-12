# ADR 0018: Connection-selection strategy seam

**Status**: Accepted
**Date**: 2026-06-12
**Feature**: 004-connections
**Source**: `specs/004-connections/{spec,research,data-model}.md` (FR-006..009, R5/R6); ADR 0011 (pure core), ADR 0012 (producer ownership), ADR 0009 (`Arc<dyn …>` service-handle precedent); `specs/ROADMAP.md` 006/007 (policy consumers).

## Context

Establishment is autonomous and policy-driven: on a setup event, the transition must
map the node's current knowledge (own topics + candidates) to an expected upstream set
and apply it as a diff (spec FR-007/008). The selection policy must be swappable —
ROADMAP 006 (`DialerPolicy`-style epochal selection) and 007 (golden mode disabling
dialing) are the named consumers — and the decision must stay inside the pure core
(synchronously testable, ADR 0011). Where the policy object lives and how the setup
trigger is produced are structural: they shape `apply`'s reach, `Node::new`'s
signature, and the producer set.

## Decision

### 1. A sync, pure trait, `Arc<dyn>`-injected, stored on `NodeState`

```rust
pub trait ConnectionStrategy: Send + Sync {
    fn expected_upstream(
        &self,
        subscriptions: &HashSet<TopicId>,
        candidates: &HashMap<TopicId, HashSet<PeerId>>,
    ) -> HashSet<(PeerId, TopicId)>;
}
pub struct ConnectToAllCandidates;   // v1 policy: every candidate of every own topic
```

`Node::new` takes `strategy: Arc<dyn ConnectionStrategy>` and hands it to `NodeState`,
which stores it **beside the verifier** — the established slot for immutable service
handles a transition consults. `apply`'s signature is unchanged; the
`ConnectionSetup` arm reads the strategy from state. The trait is synchronous and
dyn-compatible by design (no async, no associated types — the ADR 0009 shape), so
config-driven instantiation later is a constructor-time concern only.

### 2. The diff rule lives in the transition, not the strategy

The strategy returns *what should exist*; `apply` owns *how to get there*: dial
everything expected that is not Active (missing pairs gain `AwaitingAccept` +
Request; pending pairs are re-dialed keeping their entry; Active pairs untouched;
expected-set membership never removes). Policies stay declarative and trivially
testable; the lifecycle mechanics are written once.

### 3. The setup trigger is an ordinary event with two producers

`Event::ConnectionSetup` is pushed either by the optional one-shot timer — a fourth
node-owned producer, after the network mailbox and the two registry readers
(`setup_timer_producer`: sleep, push once, return), spawned via
the existing `spawn_producer` only when `NodeConfig.connection_setup_delay` is
`Some`, and therefore drop-aborted like every producer (ADR 0012) — or externally
through the public event intake. The transition cannot tell the producers apart and
processes every setup event by the same diff; the delay is **unset by default**
(autonomy is opt-in; scripted tests never race a timer).

## Consequences

- The whole establishment decision (selection + diff) is exercisable in synchronous
  `state.rs` tests; a policy is testable as a pure function with zero node machinery.
- 006 implements its epochal/pick-n policy as another `ConnectionStrategy` (plus its
  own periodic producer pushing setup events); 007's golden mode is "no timer + a
  policy returning the empty set" or simply an unset delay — no new seams needed.
- Repeated setup events are well-defined (diff semantics) — the static v1 topology is
  a consequence of the single self-generated event, not an enforced invariant.
- The strategy joins the verifier as transition-visible state; anything else a future
  policy needs (e.g. bootstrap peers) must first move into `NodeState` per the
  established "what `apply` reads lives in state" rule (N-007 discipline).

## Alternatives considered

- **Threading the strategy into `apply` as a parameter**: rejected — changes the
  transition signature for every future service and breaks the one-slot precedent.
- **Generic `Node<S: ConnectionStrategy>`**: rejected — infects every consumer's
  type; config-driven instantiation wants dynamic dispatch anyway.
- **Strategy returns effects directly**: rejected — would let policies bypass the
  diff and mutate lifecycle mechanics; the declarative expected-set keeps the
  invariants (no removal, no re-dial of Active) in one place.
- **A latch making setup once-only**: rejected at clarify (user decision) — staticness
  is a scoping simplification, not an invariant; the diff makes repetition safe.

## Sources

- `specs/004-connections/spec.md` — FR-006..009, Clarifications (diff-reprocess,
  pending re-dial, opt-in delay).
- `specs/004-connections/research.md` — R5, R6; `data-model.md` §1.4, §2.
- ADR 0009 / 0011 / 0012; `specs/ROADMAP.md` entries 006, 007.

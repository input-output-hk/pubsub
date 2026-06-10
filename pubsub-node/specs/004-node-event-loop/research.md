# Research: Node Event-Loop Refactor (004)

**Date**: 2026-06-09 | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

No `NEEDS CLARIFICATION` markers existed in the Technical Context — every design question
was resolved in pre-plan discussion between the maintainer and the implementation agent
(2026-06-09 session), grounded in the shared seam contract
[`../event-loop-and-registry-contract.md`](../event-loop-and-registry-contract.md) and the
current code on `main` (the 004/008 seam commit). This document consolidates those
resolutions in decision/rationale/alternatives form. Structural decisions are formalized in
ADR 0011 and ADR 0012; this file records the full set, including tactical ones.

---

## R1. Feature scope: refactor only; connections deferred

- **Decision**: This feature delivers only the pure-core restructure (NodeState + `apply` +
  `Effect`). The connection model (dial/accept, `Connection`, fan-out, keyed producers) is
  deferred to a follow-on feature `004-connections`, branched from `main` after this lands.
- **Rationale**: The contract doc frames the refactor as the core deliverable and the
  parallel-work plan requires it to merge first so 008 can branch from updated `main`.
  A thin 004 unblocks 008 sooner and avoids dragging five unresolved connection questions
  (state machine, deny path, backpressure, reconnection, multi-connection recv) into this
  cycle. ROADMAP numbers are IDs, not order; both parts stay under the 004 umbrella.
- **Alternatives considered**: (a) full ROADMAP-004 (refactor + connections) in one feature —
  rejected: delays the 008-unblocking merge and widens 008's rebase surface; (b) numbering
  the connections feature 005+ — rejected: collides with ROADMAP 005 (peer view) and breaks
  the conceptual grouping.

## R2. `Effect` ships uninhabited; signature locked

- **Decision**: `#[non_exhaustive] pub(crate) enum Effect {}` (no variants). `apply` returns
  `Vec<Effect>`, always empty in this feature. The shell executes effects via `match effect {}`
  (vacuous match over the uninhabited type).
- **Rationale**: Pre-connection the node only ingests — a valid message is recorded (state
  mutation, not an effect), a dropped one is logged (ambient effect, see R3), and
  subscribe/unsubscribe are sync methods. The first real inhabitants (`ForwardTo`/`Dial`/
  `Close`) arrive with 004-connections; 008's `RegistryUpdate` arm is also state-only.
  Locking `-> Vec<Effect>` now means neither 008 (which writes an `apply` arm against this
  signature) nor 004-connections reshapes the contract — a forward-compatible interface
  justified by named ROADMAP consumers (constitution Engineering Standards).
- **Alternatives considered**: returning `()` until connections — rejected: changes `apply`'s
  signature later, touching every call site and 008's merged arm; folding drop-logging into
  `Effect` as an observability variant — rejected, see R3.

## R3. Purity carve-out: `tracing` is a permitted ambient effect

- **Decision**: `apply` is pure **with respect to state and protocol effects**. Inline
  `tracing` calls (the `message_dropped` events with `cause = topic_not_subscribed |
  invalid_signature`, and the subscription events) move with the logic into the pure core
  and are emitted at the decision site. They are not modeled as `Effect`s or observations,
  and are never asserted on in tests.
- **Rationale**: The constitution pins logs as operator UX, not a test surface — the
  single biggest reason to extract logs (assertability) is anti-aligned here. `tracing` is
  ambient by design: it touches no protocol state and does not affect `apply`'s return or
  mutation, so the contract-relevant purity (synchronous, deterministic, no protocol I/O)
  is fully preserved. Extraction would split each decision from its log across two sites
  and re-plumb every log field through a new type for no testing benefit.
- **Alternatives considered**: (a) `ApplyOutcome { effects, observations }` with the shell
  rendering `Dropped { cause }` observations as logs — rejected: real plumbing cost, no
  practical benefit under the logs-not-a-test-surface standard, deviates from the contract's
  literal `-> Vec<Effect>` signature; (b) `Effect::Drop { cause, .. }` — rejected: mixes
  observability into the outbound-command type, a modeling smell once real effects arrive.

## R4. State sharing: `Arc<Mutex<NodeState>>`, sync lock-and-clone getters

- **Decision**: `NodeState` is shared as `Arc<Mutex<NodeState>>`. The event loop is the sole
  **event-driven** writer (`apply(&mut state.lock().unwrap(), event)`); public getters
  (`received_messages()`, `subscriptions()`) lock and clone synchronously, exactly as the
  003 API behaves today.
- **Rationale**: Consolidates today's two separate mutexes into one struct while keeping the
  entire 003 public surface and test patterns (`await_delivery` polling) intact, and
  preserves 003's linearizability via the single lock. The contract doc shows this as the
  default shape.
- **Alternatives considered**: event loop owns `NodeState` outright, getters answered via
  query events carrying `oneshot` reply channels — rejected: every getter becomes async and
  eventually-consistent, `oneshot` machinery per read, breaks the 003 API and test suite,
  and buys only a "single owner" property that matters under contention we don't have.
  Recorded in ADR 0012 so the trade-off isn't re-litigated.

## R5. `subscribe`/`unsubscribe`: sync methods delegating to the pure core

- **Decision**: `Node::subscribe`/`unsubscribe` remain synchronous public methods returning
  `SubscribeOutcome`/`UnsubscribeOutcome` (per ADR 0008), implemented as thin lock-takers
  delegating to `NodeState::subscribe`/`unsubscribe` where the logic (and its inline
  logging, per R3) lives.
- **Rationale**: The protocol is epochal — the future dialer reads the *current*
  subscription set on epoch tick; `subscribe` never emits effects, so event-sourcing it buys
  no effect-routing and costs the synchronous return. Subscriptions in this feature are
  config-seeded and static in practice (mutated only in tests), and the topics read from
  config are assumed already registry-confirmed (spec Assumptions).
- **Alternatives considered**: `Event::Subscribe { topic, reply: oneshot }` through `apply` —
  rejected for this feature: async API break, eventual consistency, `oneshot` plumbing.
  **Expected future direction** (recorded, not built): registry-driven subscription-update
  events through the queue (008+) will likely deprecate these sync methods; that flow rides
  the existing `Event::RegistryUpdate` seam, not a new mechanism.

## R6. Lifecycle: spawn-in-constructor, drop-abort

- **Decision**: `Node::new` spawns the event loop (and the network producer); `Drop` aborts
  the loop and every producer. Unchanged from the seam commit.
- **Rationale**: `Node` is an interactive handle — the §5 test strategy and the sync
  getter/subscribe surface (R4/R5) require the loop to run concurrently while the caller
  holds the node. Self-contained ownership (own it, drop it, done) preserves 001–003
  ergonomics.
- **Alternatives considered**: caller-driven future (`Node::new` returns `(Node, impl
  Future)` or `node.run_loop()`) — rejected: caller must remember to drive it, drop-abort
  ownership gets fuzzier, and the actor-style variant re-opens the rejected R4/R5 choices.

## R7. Code organization: named handlers and named producer fns

- **Decision**: `apply` is a thin match dispatching each `Event` variant to a named handler
  (`handle_message_received(state, from, message) -> Vec<Effect>`); producer bodies are
  named async fns (`network_mailbox_loop(queue, rx)`) passed to `spawn_producer`, not inline
  closures.
- **Rationale**: Maintainer convention (explicit functions over anonymous bodies). Bonus:
  008's merge adds a one-line dispatch arm plus its own handler function — near-zero merge
  surface on `apply` while the compiler's exhaustiveness check still enforces wiring; named
  producers keep network and registry readers symmetric.
- **Alternatives considered**: inline match arms / inline closures (current code) — rejected
  by convention; no functional difference.

## R8. Visibility: crate-internal core (from spec clarification 2026-06-09)

- **Decision**: `NodeState`, `apply`, `Effect`, and the handlers are `pub(crate)` in a new
  `src/state.rs` module that is **not** re-exported from `lib.rs`. `Node` remains the only
  public surface; the already-public seam items (`Event`, `EventQueue`, `events()`,
  `spawn_producer`) stay exactly as `main` has them.
- **Rationale**: No external consumer needs the core (008 is in-crate); the constitution
  requires a named ROADMAP consumer to justify public surface; keeps SC-004 ("no new public
  API") exactly true. US2's synchronous tests live as in-module unit tests.
- **Alternatives considered**: public `NodeState`/`apply` (as the contract doc's
  illustrative sketch shows) — rejected: an API commitment nothing external consumes; the
  contract's normative seam (§3) doesn't require it. Deviation noted in ADR 0011.

## R9. What moves where (mechanical inventory)

- **Decision**: `node.rs` keeps the shell concerns: `handle` (network I/O), `peers` (static
  descriptor list), `events` queue handle, `event_loop` + `producers` JoinHandles, public
  methods. `NodeState` takes `self_id`, `subscriptions`, `received`, `verifier` (per the
  contract §1.1); the `Node`'s duplicate `#[allow(dead_code)] verifier` field is removed —
  `NodeState` becomes the verifier's canonical owner.
- **Rationale**: Live I/O handles stay on the shell (this is what keeps `apply` pure and is
  the same split 004-connections will use for connection sinks); everything `apply` reads or
  writes lives in `NodeState`.
- **Alternatives considered**: keeping `verifier` duplicated on `Node` for a hypothetical
  future sync-verify API — rejected: speculative (no ROADMAP consumer); re-add when named.

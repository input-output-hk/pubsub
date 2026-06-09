# ADR 0011: Pure state-transition core — `NodeState` + `apply` + uninhabited `Effect`

**Status**: Accepted
**Date**: 2026-06-09
**Feature**: 004-node-event-loop
**Source**: `specs/event-loop-and-registry-contract.md` §1; `specs/004-node-event-loop/research.md` R1–R3, R7–R9

## Context

The 004/008 seam commit gave the node a single event queue with one consumer loop and
node-owned producers, but left the 002/003 message-handling logic inline in the consumer
loop, operating over scattered `Arc<Mutex<…>>` fields (`received`, `subscriptions`). The
shared seam contract (`specs/event-loop-and-registry-contract.md`) commits Feature A (this
feature) to formalizing the next step as ADR(s): an explicit state struct mutated only by a
pure transition function that returns effects, so that

- the protocol logic is synchronously testable with no async runtime, and
- the event stream is the single seam that 008 (registry reader) and the future
  004-connections feature extend without reshaping the core.

This is structural per Principle III: the `apply` signature is the contract 008 writes its
`RegistryUpdate` arm against and the contract connections will return fan-out commands
through — reversing it later touches another feature's merged code.

## Decision

A new **crate-internal** module `src/state.rs` holds the pure core:

```rust
pub(crate) struct NodeState {
    self_id: PeerId,
    subscriptions: HashSet<TopicId>,
    received: Vec<ReceivedDelivery>,
    verifier: Arc<dyn Verifier>,   // shared immutable service handle, not shared mutable state
}

/// Outbound commands the shell executes. Uninhabited at this stage: the node only
/// ingests. First variants (ForwardTo / Dial / Close) arrive with the connection model.
#[non_exhaustive]
pub(crate) enum Effect {}

pub(crate) fn apply(state: &mut NodeState, event: Event) -> Vec<Effect> {
    match event {
        Event::MessageReceived { from, message } => handle_message_received(state, from, message),
        // 008 adds: Event::RegistryUpdate(update) => handle_registry_update(state, update),
    }
}
```

Three sub-decisions:

1. **`Effect` ships uninhabited with the signature locked.** `apply` always returns an
   empty `Vec` in this feature; the shell's effect-execution step is the vacuous
   `match effect {}`. The forward-compatible shape is justified by named ROADMAP consumers
   (004-connections fan-out/dial/close; 008 writes an arm against this signature), per the
   constitution's forward-compatible-interfaces standard.
2. **Purity carve-out — `tracing` is a permitted ambient effect.** `apply` is pure with
   respect to **state and protocol effects**: synchronous, deterministic, no protocol I/O,
   no `.await`. Inline `tracing` calls (the `message_dropped` events, subscription events)
   move with the logic into the core and are emitted at the decision site. They are not
   modeled as `Effect`s: the constitution pins logs as operator UX that tests must not
   assert on, which removes the only practical reason to extract them, while extraction
   would split each decision from its log and re-plumb every log field for no benefit.
3. **Named per-variant handlers.** `apply` is a thin match dispatching each variant to a
   named function (`handle_message_received(state, from, message) -> Vec<Effect>`). Each
   future variant gets its own handler (008's arm is one dispatch line plus its own
   function — near-zero merge surface on `apply` while the compiler's exhaustiveness check
   still enforces wiring). Producer bodies follow the same convention as named async fns
   (`network_mailbox_loop`) rather than inline closures.

Visibility: the core is `pub(crate)` and not re-exported — `Node` remains the only public
surface (spec clarification 2026-06-09). The seam contract's illustrative sketch shows these
items as `pub`; the normative seam (§3 of that doc) only requires `EventQueue`,
`spawn_producer`, and the `Event` variant ownership split, all of which are already public
on `main` and unchanged. The synchronous state-machine tests live in-module
(`#[cfg(test)] mod tests` in `state.rs`).

## Consequences

- Protocol logic gains a synchronous, deterministic test surface: construct `NodeState`,
  apply a scripted `Vec<Event>`, assert on state and returned effects after each step — no
  runtime, no channels, no tasks (contract doc §5, primary testing approach).
- 008 extends the core additively: one `Event` variant, one dispatch line, one handler
  function; existing branches untouched.
- 004-connections populates `Effect` and implements the shell-side executor without
  touching `apply`'s signature or existing handlers; live connection sinks stay on the
  shell, which is what keeps `apply` pure when fan-out lands.
- The uninhabited `Effect` makes "no effects pre-connection" a compiler-checked fact
  (`Vec<Effect>` can only be empty), not a convention.
- Log-emission sites move source location (loop → handlers); event names and fields are
  unchanged, so operator-visible behavior is identical.
- If a future feature genuinely needs drop *dispositions* as data (e.g. a misbehavior
  signal that closes connections), that lands as a real `Effect` variant carrying the
  protocol decision — not as log extraction.

## Alternatives considered

- **Return `()` until connections need effects**: rejected — changes `apply`'s signature
  later, touching every call site including 008's already-merged arm; the whole point of
  the seam is that the contract doesn't reshape under the parallel feature.
- **`ApplyOutcome { effects, observations }`** (drop dispositions returned for the shell to
  log): rejected — real plumbing cost (every log field re-exported through a type, logic
  and log split across two sites) purchasing a purity level nothing consumes: logs are
  constitutionally not a test surface, and `tracing` subscribers already handle
  redirection/suppression for any future replay harness.
- **`Effect::Drop { cause, … }`** (logging folded into the effect channel): rejected —
  mixes observability into the outbound-command type; a modeling smell once
  `ForwardTo`/`Dial` arrive, and it falsifies "no effects pre-connection".
- **Public `NodeState`/`apply`** (as the contract doc's sketch shows): rejected — an API
  commitment with no external consumer (008 is in-crate); the constitution requires a named
  ROADMAP consumer to justify forward-facing surface, and crate-internal keeps the spec's
  "no new public API" success criterion exactly true.
- **Inline match arms / inline producer closures** (current seam-commit shape): rejected by
  maintainer convention — named functions over anonymous bodies; no functional difference.

## Sources

- `specs/event-loop-and-registry-contract.md` §1 (Feature A shape), §3 (normative seam),
  §5 (testing approach).
- `specs/004-node-event-loop/spec.md` — FR-008, FR-012, FR-013; Clarifications 2026-06-09.
- `specs/004-node-event-loop/research.md` — R1, R2, R3, R7, R8, R9.
- ADR 0012 — the companion sharing/lifecycle decision for the shell side.

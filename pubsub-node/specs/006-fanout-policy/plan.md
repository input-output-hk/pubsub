# Implementation Plan: Message Publishing and Fan-out Forwarding

**Branch**: `006-fanout-policy` | **Date**: 2026-06-16 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/006-fanout-policy/spec.md` — converged across two clarify rounds (trajectory in its Clarifications section) — plus the agreed plan-input text reproduced verbatim in [`plan-input.md`](./plan-input.md).

## Summary

A node gains the ability to **originate** a dissemination message (`Node::publish`, fire-and-forget, validated in a new `handle_publish` transition) and to **forward** messages — published or received — to its downstream peers on the message's topic. Forwarding targets come from an injected `FanoutStrategy` (v1 `ForwardToAll`), the deliberate twin of 004's `ConnectionStrategy`; each forward reuses `Effect::Send` verbatim (no re-signing, no new effect variant). Loop suppression is a `seen: HashSet<MessageHash>` checked after signature verification at the record point, identical on both paths. A recorded delivery's wire-origin becomes explicit (`Origin { Local, Peer(PeerId) }`). Everything that would compromise determinism or scope (pick-k sampling, bounded `seen`, equivocation detection, the `Signed`→`Dissemination` rename, the epochal re-dialer) stays deferred and catalogued.

Technical approach per `research.md` R1–R8 and ADR 0021: all behavior lands in the pure core (`state.rs` — a new `handle_publish` arm, the receive arm extended with dedup + fan-out, a shared `fanout` helper), a new `fanout` module for the strategy seam, and a one-parameter addition to `Node::new` plus the `Node::publish` method on the shell. No new producers, no new `Effect` variant, no new dependencies.

## Technical Context

**Language/Version**: Rust, edition 2021, `rust-version = 1.75` (workspace unchanged)

**Primary Dependencies**: tokio 1, serde/toml, tracing, thiserror, clap, sha2 + rand/rand_chacha (mock crypto) — **no new dependencies** (`HashSet` + existing `MessageHash` + `Arc<dyn>` only; no justified-dependency ADR needed)

**Storage**: none (in-memory state; the `seen` set is in-memory and unbounded)

**Testing**: cargo test — synchronous state-machine tests in `src/state.rs` (extending the existing module + `ConnectionScript`), a pure unit test for `ForwardToAll` in `src/fanout.rs`, reworked dissemination integration suites under `tests/`; proptest available where a property claim (dedup idempotence, termination) warrants it

**Target Platform**: library + thin CLI binary, platform-agnostic (development on macOS/Linux)

**Project Type**: single Rust crate inside the parent `pubsub/` repo

**Performance Goals**: n/a (in-memory PoC; correctness over optimization)

**Constraints**: pure/sync transition core (no I/O or awaits under the state lock); effects executed outside the lock; deterministic `apply` (no RNG in state — `ForwardToAll` is set-deterministic; pick-k stays out); reproducible tests (no wall-clock assertions); logs are operator UX, never a test surface

**Scale/Scope**: small fixed test topologies (2–N in-process nodes); full-mesh and scripted partial/line shapes

## Constitution Check

*GATE: evaluated pre-Phase-0 and re-checked post-Phase-1 (both pass; no Complexity Tracking entries needed).*

- **I. Correctness Over Optimization — ✅** Every behavior traces: spec FR-001..016 + Clarifications; research R1–R8; ADR 0021; data-model flows §2–§6 map transitions to FRs; the deferral catalogue §7 maps every gap to its spec line and follow-up.
- **II. Test-Driven for Correctness Claims — ✅ (TDD required)** Publishing, relay, and dedup are protocol-behavior claims; this feature is **critical**. Tasks must order failing tests before implementation: pure `ForwardToAll` and `state.rs` script tests first, then integration. SC-001..006 are the test-shape contract.
- **III. Document Structural Decisions as ADRs — ✅** ADR 0021 (fan-out strategy seam + content-hash dedup + `Origin`) authored with this plan, referencing 0018's seam rationale and N-005's content-anchored hash. Tactical choices (names, the `duplicate` cause, module layout) are recorded in research/contracts and exempt.
- **IV. Specifications as Ambiguity Detectors — ✅** The one ambiguity (relay/dedup test topology) was surfaced and resolved in the recorded clarify round; the round-2 story-independence fix was applied to the spec, not silently in this plan. No new protocol-doc ambiguity emerged.
- **V. Specifications Are Read-Only — ✅** No edits to `pubsub/docs/` or `pubsub/formal_spec/`. Edits are confined to `specs/006-fanout-policy/`, `docs/decisions/`, and (at implement time) `pubsub-node/src` + `IMPLEMENTATION_NOTES.md`.

Engineering Standards applied: logs-not-a-test-surface (drop/`duplicate` and `connection_severed` events are operator UX; assertions go through `received_messages()` and effect lists); neutral operator strings (snake_case causes, no FR citations); parse-at-the-edge (the caller builds and signs the `SignedMessage`; the node mints nothing and consults no clock); forward-compatible shapes justified by named consumers (`FanoutStrategy` → ROADMAP 006/007; `Event::Publish` rides the `#[non_exhaustive]` enum); declarative test construction (extend `ConnectionScript`; the `fanout::test_support` no-op strategy); reproducible tests (no wall-clock; set-deterministic fan-out, order sorted in assertions); property testing available for dedup idempotence / termination.

## Plan input

> The agreed `/speckit-plan` input text is preserved verbatim in [`plan-input.md`](./plan-input.md) (committed alongside this plan), per the project's verbatim-input convention (mirrors `specs/004-connections/plan-input.md`). All of its mandates are discharged by this plan's artifacts.

## Project Structure

### Documentation (this feature)

```text
specs/006-fanout-policy/
├── spec.md              # converged specification (2 clarify rounds)
├── plan.md              # this file
├── plan-input.md        # verbatim /speckit-plan input (agreed record)
├── research.md          # Phase 0 — decisions R1–R8
├── data-model.md        # Phase 1 — entities, decision flows, propagation walkthroughs, deferrals
├── quickstart.md        # Phase 1 — publish/relay/dedup walkthrough on the new surface
├── contracts/
│   └── fanout-protocol.md   # publish/fan-out/dedup contract, drop vocabulary, public-surface delta
├── checklists/requirements.md  # spec quality checklist (from /speckit-specify)
└── tasks.md             # Phase 2 (/speckit-tasks — not created by this command)

docs/decisions/
└── 0021-fanout-strategy-seam-dedup-and-message-origin.md
```

### Source Code (repository root: `pubsub-node/`)

```text
src/
├── fanout.rs            # NEW: FanoutStrategy trait, ForwardToAll, pub(crate) #[cfg(test)]
│                        #      test_support no-op strategy
├── received.rs          # + Origin enum; ReceivedDelivery.from → origin: Origin
├── event.rs             # + Event::Publish(SignedMessage)
├── state.rs             # NodeState + seen: HashSet<MessageHash> + fanout handle;
│                        #   new apply arm handle_publish; handle_signed_message extended
│                        #   (dedup + fan-out at the record point); shared fanout() helper
├── node.rs              # Node::new(+ fanout_strategy param); Node::publish(self, SignedMessage)
├── lib.rs               # re-export delta: FanoutStrategy, ForwardToAll, Origin (per contracts §5)
└── main.rs              # construction updated to pass Arc::new(ForwardToAll)

tests/
├── common/mod.rs        # + fanout-strategy arg in the shared node constructor;
│                        #   partial-topology (scripted handshake) + await-relay helpers
├── <dissemination suites>   # reworked: assert forwarding — full-mesh dedup + scripted line relay
└── connections.rs       # touched only for the constructor's new arg (public ForwardToAll; the cfg(test) no-op is invisible to integration crates)
```

**Structure Decision**: single-crate layout unchanged; one new module (`src/fanout.rs`) for the fan-out domain + strategy + test no-op, keeping `state.rs` focused on transition arms — mirrors how `src/connection.rs` hosts the connection seam.

## Decision summary (binding for /speckit-tasks)

| # | Decision | Where recorded |
|---|---|---|
| 1 | `FanoutStrategy` sync pure trait + `ForwardToAll`; `Arc<dyn>` on `NodeState` beside `strategy`; new `Node::new` param | R1, ADR 0021 §1, contracts §2/§5 |
| 2 | Shared verbatim `fanout()` helper; `Effect::Send` reused; no new effect variant; split-horizon `exclude` | R6, ADR 0021 §2, contracts §2 |
| 3 | `seen: HashSet<MessageHash>` keyed on `MessageHash::of(&plain)`; unbounded | R2, ADR 0021 §3, data-model §1.3 |
| 4 | Dedup after signature verification, at the record point, both paths | R3, ADR 0021 §3, contracts §3 |
| 5 | `Event::Publish` + named `handle_publish`; receive chain minus connection-gate/severance; proxy allowed | R4, ADR 0021 §4, data-model §2 |
| 6 | `Node::publish(self, SignedMessage) -> ()` fire-and-forget | R4, contracts §1 |
| 7 | `ReceivedDelivery.from` → `origin: Origin { Local, Peer(PeerId) }` (fixes doc drift) | R5, ADR 0021 §5, data-model §1.1 |
| 8 | Drop causes: dedup `duplicate`; publish reuses receive causes; publish never severs | R3/R4, contracts §4 |
| 9 | Tests: full-mesh + scripted partial/line; `fanout::test_support` no-op; not parity-preserving | R7/R8, contracts §6 |

## Post-plan obligations carried to /speckit-tasks (not done here)

- `IMPLEMENTATION_NOTES.md`: add the deferral entries — bounded `seen` store (D1, real-impl); pick-k fan-out needing seeded RNG (D2, ROADMAP 006/007); reaffirm equivocation at 012 (D3, links N-003); `Signed`→`Dissemination` rename (D4); epochal re-dialer (D5). Each cross-referencing data-model §7.
- Rustdoc: rewrite `ReceivedDelivery`'s field doc for `Origin` (the old "originated" wording was already drift); document `Node::publish` and the `FanoutStrategy` seam for the library audience in stable, FR-free terms.
- Not parity-preserving: the dissemination-suite rework is chartered (spec US1/US2/US3, SC-001..006), not collateral; the receive-path unit tests keep their assertions (empty downstream ⇒ no-op fan-out) beyond the shared constructor's new argument.

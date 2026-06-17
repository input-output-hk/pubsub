# Tasks: Message Publishing and Fan-out Forwarding

**Input**: Design documents from `/specs/006-fanout-policy/`

**Prerequisites**: plan.md (binding decision table rows 1–9), spec.md (US1–US3, FR-001..016, SC-001..006), research.md (R1–R9), data-model.md (decision flows + deferral catalogue D1–D5), contracts/fanout-protocol.md, ADR 0020

**Tests**: MANDATORY — the feature is designated critical (plan.md Constitution Check II). Within every story, the state-machine test task precedes the implementation it drives and must fail first. Two recorded exceptions, both compile-coupled changes with no red-green possible: T001 (pure `ForwardToAll`, tested in the same increment) and T002 (the `ReceivedDelivery` field reshape — behavior-preserving, existing assertions updated mechanically).

**Green checkpoints**: every task's commit passes the full sweep (`cargo fmt` + clippy + build + all tests). Phase boundaries marked ⛳ are the planned commit checkpoints.

**Ordering hazard (binding)**: receive-path fan-out (US2) in a **cyclic** topology loops forever **without** dedup (US3). Therefore US2's integration tests use **acyclic** topologies only (line/tree), and T009 must verify no pre-existing integration suite forms a payload-forwarding cycle (the 004 star and 2-node suites are cycle-free by split-horizon — each receiver's only downstream is the deliverer, excluded). The first **cyclic** (full-mesh/triangle) payload test lands in US3 (T012), after the `seen` gate exists.

**Conventions**: operator-facing strings carry no FR citations (FR refs live in `//` comments and these artifacts); drop causes per contracts §4 (dedup = `duplicate`; publish reuses the receive causes); declarative test construction per constitution v1.2.0; fan-out target order is unspecified, so assertions sort.

## Phase 1: Foundational — fan-out vocabulary + delivery origin (checkpoint 1)

**Purpose**: the inert seam + the origin reshape every story consumes; nothing produces a fan-out effect yet (plan rows 1, 7).

- [X] T001 [P] Create `src/fanout.rs`: `FanoutStrategy` trait (sync, pure: `targets(&topic, &downstream, exclude) -> Vec<PeerId>`), `ForwardToAll` impl, and a `#[cfg(test)] #[allow(dead_code)] test_support` no-op strategy (`targets` → empty); failing-first unit tests for `ForwardToAll` (every downstream on the topic; `exclude` removes that peer; empty/other-topic downstream → empty; assertions sort); declare the module in `src/lib.rs` and re-export **only** `FanoutStrategy` + `ForwardToAll` (the no-op stays crate-internal, never re-exported)
- [X] T002 [P] Add `enum Origin { Local, Peer(PeerId) }` in `src/received.rs` and reshape `ReceivedDelivery.from` → `origin: Origin` (rewrite the field rustdoc — the old "originated" wording was already drift); mechanically update the existing record site in `src/state.rs` (`Origin::Peer(from)`), the `received_messages` snapshot path in `src/node.rs`, and the existing `.from` assertions in `src/state.rs` tests — behavior preserved; re-export `Origin` from `src/lib.rs` ⛳

## Phase 2: User Story 1 — publish + first-hop fan-out (P1) 🎯 MVP

**Goal**: `Node::publish` originates a validated message, recorded `Local` and fanned to direct downstream; the shared `validate_dissemination` / `record_and_fanout` helpers (research R9) are extracted and adopted by both paths.

**Independent test**: a publisher with ≥2 downstream on a shared topic → `publish` → publisher records `Local`, each downstream records; failed-check publishes drop; proxy (publisher ≠ self) accepted (spec US1, SC-001/006).

- [X] T003 [US1] Write failing sync state tests in `src/state.rs` for `handle_publish`: valid publish records `Origin::Local` + one `Effect::Send` per downstream on the topic (sorted); no-downstream → recorded, no effects; proxy publish (publisher key ≠ self) accepted; the four drop scenarios (not-subscribed / not-registered / unauthorized / invalid-signature → no record, no effects, and **no severance**); each forward is verbatim (the `Effect::Send` payload equals the published `SignedMessage`) (FR-001..005, FR-007/011/016, US1-AS1..4)
- [X] T004 [US1] Implement to pass T003 in `src/state.rs`: `NodeState` gains `fanout: Arc<dyn FanoutStrategy>`; add the pure helpers `validate_dissemination` (subscribed → registered → authorized, returns the drop cause or `None`), the inner `fanout()` (strategy `targets` → `Vec<Effect::Send>`, verbatim clone), and `record_and_fanout` (record-with-`origin` → `fanout(exclude)`; **no dedup gate yet**); implement `handle_publish` (`validate_dissemination` → verify [**plain drop** on failure] → `record_and_fanout(Origin::Local, None)`); refactor the existing `handle_signed_message` to call `validate_dissemination` for its check chain (record + severance unchanged; **no receive-side fan-out yet** — that is US2). Drop logging stays in each handler (cause from `validate_dissemination`, fields per path)
- [X] T005 [US1] Wire the public surface: `Event::Publish(SignedMessage)` in `src/event.rs` (+ the `apply` arm → `handle_publish`); `Node::publish(&self, SignedMessage)` fire-and-forget in `src/node.rs`; add `fanout_strategy: Arc<dyn FanoutStrategy>` as `Node::new`'s final parameter threaded into `NodeState::new`; update every call site in the same increment — `src/main.rs` (`Arc::new(ForwardToAll)`), the shared `tests/common/mod.rs` constructor(s), the direct call in `tests/candidate_set.rs`, and the `no_run` doctest in `src/network.rs` (doctests compile) — suites stay green
- [X] T006 [US1] Integration test in new `tests/dissemination.rs`: a publisher with two downstream on a shared topic (established through the real path) → `publish` → publisher records `Origin::Local`, both downstream record; an off-topic / unauthorized publish records nowhere (SC-001, SC-006) ⛳

## Phase 3: User Story 2 — relay onward through the mesh (P2)

**Goal**: a received message, after recording, is fanned to the node's other downstream (split-horizon excludes the deliverer); forwarding is verbatim.

**Independent test**: a scripted **acyclic** line A→B→C (no A–C edge, built via `Request`/`Accepted` scripting) → publish at A → C records via B's relay only, B does not echo to A (spec US2, SC-002 partial).

- [ ] T007 [US2] Write failing sync state tests in `src/state.rs`: after recording, `handle_signed_message` emits one `Effect::Send` per downstream on the topic **minus the delivering peer** (split-horizon); deliverer-as-sole-downstream → no effects; the forward is verbatim (signature unchanged — US2 AS5); the recorded `origin` is `Peer(from)` (US2 AS1) (FR-006/007/009, US2-AS1..3,AS5)
- [ ] T008 [US2] Wire fan-out into `handle_signed_message` in `src/state.rs` via `record_and_fanout(Origin::Peer(from), Some(&from))`, replacing the bare record from T004 — the receive path now records **and** fans out
- [ ] T009 [US2] Integration in `tests/dissemination.rs`: scripted **acyclic** line A→B→C → publish at A → C records via relay only, no B→A echo (US2-AS1..3); add the `tests/common` declarative helpers this needs (a partial-topology builder extending `ConnectionScript`, an `await_relay`-style poll). **Verify** no pre-existing suite forms a payload-forwarding cycle (confirm the 004 star/2-node suites are cycle-free by split-horizon); any suite that would now loop is made acyclic or deferred to T012's note ⛳

## Phase 4: User Story 3 — forwarding loops suppressed (P3)

**Goal**: the `seen` set drops an already-recorded message (no re-record, no re-fan-out), making relay safe in cyclic meshes; dedup spans both paths and never poisons on a failed-verification message.

**Independent test**: a triangle of three mutually-connected members → one publishes → each records exactly once, propagation terminates (spec US3, SC-003/005).

- [ ] T010 [US3] Write failing sync state tests in `src/state.rs`: an already-seen message redelivered over an Active upstream is dropped (`duplicate`), not re-recorded, not re-fanned; re-publishing identical content (same content hash) is dropped as `duplicate` (contracts §1.6 — confirms the publish path inserts into `seen`); dedup spans both paths (a published-then-relayed-back message is suppressed — FR-015); **US3 AS4 publish-path no-poisoning** — an invalid-signature **publish** whose `plain` hashes identically to a genuine message is a plain drop that never seen-marks, so the genuine valid message is still recorded (FR-012/013/015, US3-AS1,2,4)
- [ ] T011 [US3] Add `seen: HashSet<MessageHash>` to `NodeState` and the dedup gate **inside** `record_and_fanout` in `src/state.rs`: compute `MessageHash::of(&signed.plain)`; if present → drop (`duplicate`), return no effects; else insert, record, fan out — after verification, at the shared record point (both paths gain dedup at once)
- [ ] T012 [US3] Integration in `tests/dissemination.rs`: a triangle of three mutually-connected members → one publishes → each node records the message exactly once and the total forwards are finite (no unbounded circulation), now safe because dedup exists (US3-AS3, SC-002 full / SC-003 / SC-005) ⛳

## Phase 5: Polish & cross-cutting obligations

- [ ] T013 [P] Update `specs/IMPLEMENTATION_NOTES.md`: add the deferral entries D1–D5 (next available N-numbers) — bounded `seen` store (real-impl), pick-k fan-out needing a seeded RNG (ROADMAP 006/007), equivocation detection (012, links N-003), `Message::Signed`→`Dissemination` rename, epochal/periodic re-dialer — each cross-referencing data-model §7
- [ ] T014 [P] Refresh rustdoc in `src/node.rs`, `src/fanout.rs`, `src/received.rs`: document `Node::publish`, the `FanoutStrategy` seam + `ForwardToAll`, and the reshaped `Origin` field in stable operator/library terms (no FR citations)
- [ ] T015 Rework the remaining dissemination integration suites (the not-parity-preserving charter): where a node holds downstream and receives a valid payload, assert the resulting forwarding (or inject the `fanout::test_support` no-op where forwarding is incidental noise — e.g. connection-lifecycle assertions in `tests/connections.rs`); re-assert the post-fan-out regression boundary — touch `tests/two_node_ping.rs`, `tests/topic_filter.rs`, `tests/n_node_graph.rs`, `tests/topic_validity.rs`, `tests/topic_registry_network.rs` only as their downstream/payload interplay requires
- [ ] T016 Verify-against-code pass for contracts §5: grep `src/lib.rs` re-exports + module visibility (`FanoutStrategy`/`ForwardToAll`/`Origin` exported; `seen`/`fanout`/the helpers/`handle_publish` crate-internal; the `test_support` no-op **not** re-exported; `ReceivedDelivery.origin` public) and reconcile contracts/quickstart if the code diverged
- [ ] T017 Final validation: full sweep; `quickstart.md` walked against the real API (code blocks compile-accurate); spec SC-001..006 checklist against the suite ⛳

## Dependencies

```text
Phase 1 (fanout + Origin)  →  Phase 2 (US1 publish+helpers)  →  Phase 3 (US2 relay, ACYCLIC)  →  Phase 4 (US3 dedup → cyclic safe)  →  Phase 5 (polish)
```

- US1 is the foundation consumer (introduces the shared helpers + `record_and_fanout` without dedup); US2 adopts `record_and_fanout` on the receive path; US3 adds the `seen` gate inside it.
- **Cyclic-topology payload tests must wait for US3** (T012) — receive fan-out without dedup loops. US2's tests (T009) are acyclic, and T009 confirms no pre-existing suite forms a cycle.
- T015 (suite rework) lands after US2/US3 so the suites assert the final forwarding behavior; T016 needs the final public surface.
- The MVP increment is **US1** (publish reaches direct subscribers); US2 adds multi-hop, US3 makes it safe at scale.

## Parallel execution examples

- Phase 1: T001 ∥ T002 (different files)
- Phase 2: T003 (test) → T004 (impl) → T005 (surface + call sites) → T006 (integration); T004 before T005 (T005's call sites need the new `NodeState`/`Node::new` shape)
- Phase 4: T010 (test) → T011 (impl) → T012 (integration)
- Phase 5: T013 ∥ T014, then T015 → T016 → T017

## Implementation strategy

One foundational checkpoint (the seam + origin), then stories in priority order, each independently testable and committed green. Strict TDD inside every story: T003, T007, T010 must fail before T004, T008, T011 respectively. The deliberate not-parity-preserving suite rework (T015) lands in polish, after the forwarding behavior is complete, so the reworked suites assert the final state. The one cross-story constraint — receive fan-out is unsafe in cycles until dedup exists — is honored by keeping US2 acyclic and deferring the triangle/full-mesh payload test to US3.

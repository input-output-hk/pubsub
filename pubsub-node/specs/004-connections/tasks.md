# Tasks: Logical Connection Management with Autonomous Static Topology

**Input**: Design documents from `/specs/004-connections/`

**Prerequisites**: plan.md (binding decision table rows 1–10), spec.md (US1–US5, FR-001..028), research.md (R1–R10), data-model.md (state machines + staleness catalog), contracts/connection-protocol.md, ADRs 0017–0019

**Tests**: MANDATORY — the feature is designated critical (plan.md Constitution Check II). Within every story, test tasks precede implementation and must fail first. Exception, recorded: the Phase 1 `PeerId` reshape is a compile-coupled type change where red-green sequencing is impossible; its behavior claims (alias round-trip, keypair agreement) are unit-tested in the same increment.

**Green checkpoints**: every task's commit passes the full sweep (`cargo fmt` + clippy + build + all tests). Phase boundaries marked ⛳ are the planned commit checkpoints; tasks inside Phase 4 land as **one commit** (the chartered not-parity-preserving break must not leave a red intermediate state).

**Conventions**: operator-facing strings carry no FR citations (FR refs live in `//` comments and these artifacts); drop causes per contracts §3; declarative test construction per constitution v1.2.0.

## Phase 1: Foundational A — key-backed `PeerId` (checkpoint 1)

**Purpose**: the cross-cutting identity reshape (plan row 2, ADR 0017), self-contained and green before any connection type exists.

- [X] T001 [P] Add `MockCryptoScheme::keypair_from_alias(&self, alias: &str) -> KeyPair` (private = alias bytes, public via `derive_public`; does not advance the RNG) with unit tests asserting the derive invariant and sign/verify through the unmodified `TestSigner`/`TestVerifier`, in `src/crypto/mock.rs`
- [X] T002 Reshape `PeerId` to wrap `PublicKey` in `src/peer.rs`: alias `FromStr` (existing non-empty/no-NUL validation, then derivation), `Display` inverse (UTF-8 prefix when bytes end with the mock public suffix, hex fallback), serde via FromStr/Display, `new`/`as_public_key`; **remove `as_str`** — including the type's rustdoc example, which uses it (doctests compile); unit tests: alias round-trip, hex fallback, equality across construction paths (FromStr vs `keypair_from_alias(..).public`)
- [X] T003 Propagate the reshape mechanically until green: `src/config.rs`, `src/main.rs`, `src/subscription_registry/{mod,in_memory,test_support}.rs`, `src/state.rs`/`src/node.rs` (compile fixes only), `tests/common/mod.rs` and any test using `PeerId::as_str`/string assumptions — no behavior change ⛳

## Phase 2: Foundational B — connection domain vocabulary (checkpoint 2)

**Purpose**: the inert types every story consumes (plan rows 3–5; nothing produces effects yet).

- [X] T004 [P] Create `src/connection.rs`: `UpstreamState { AwaitingAccept, Active }`, `ConnectionStrategy` trait (sync, pure: `expected_upstream(&subscriptions, &candidates)`), `ConnectToAllCandidates` impl; unit tests for the v1 policy (all candidates across own topics; empty view → empty set; candidates' self-exclusion is input-borne) — module declared in `src/lib.rs`, exporting **only this module's types** (`UpstreamState`, `ConnectionStrategy`, `ConnectToAllCandidates`); the rest of the contracts §4 delta is owned by T014
- [X] T005 [P] Extend `src/message.rs`: `Message::Connection(ConnectionMessage)`, `ConnectionMessage { plain, signature }`, `PlainConnection { emitter, action }`, `#[non_exhaustive] ConnectionAction::{Request, Accepted, Terminated}` (topic-carrying), `PlainConnection::signed_bytes()` (length-prefixed emitter key; tag bytes 0x00/0x01/0x02 + topic per contracts §1.1); unit tests: layout stability, sign/verify round-trip, tamper detection on each bound field (emitter, kind, topic)
- [X] T006 [P] Extend `src/event.rs` with `Event::ConnectionSetup` and `Event::Shutdown` (doc comments name the two producers and the terminal-marker role per ADRs 0018/0019)
- [X] T007 Inhabit `Effect` in `src/state.rs` (`Send { to, message }`, `Misbehaved { peer, topic, cause }`) and replace the vacuous executor match in `src/node.rs`: add `pub(crate) NetworkHandle::sender()` in `src/network.rs`, clone the `NetworkSender` into the loop task, execute `Send` via it (failure logged only) and `Misbehaved` as the warn-level `connection_severed` operator event — no `apply` arm produces effects yet ⛳

## Phase 3: User Story 1 — autonomous per-topic establishment (P1) 🎯 MVP

**Goal**: strategy-driven establishment over the diff rule; membership-validated idempotent acceptance; construction grows signer/strategy/delay.

**Independent test**: N nodes sharing a topic, registry scripted, setup injected → every pair holds Active upstream + downstream for the topic; no further activity (spec US1, SC-001).

- [X] T008 [P] [US1] Add the `pub(crate) ConnectionScript` declarative builder (membership, setup, control-message, payload, shutdown steps → `Vec<Event>`) beside the types it builds, in `src/connection.rs` test-support section
- [X] T009 [US1] Write failing sync state tests for the **dialer side** in `src/state.rs`: setup event dials all candidates (AwaitingAccept entries + one `Send(Request)` each), empty view no-op, self never dialed, repeated setup re-dials pending pairs / skips Active / never removes, and a post-setup membership update **folds into candidates but creates no connection entries and returns no effects** — selection runs only on setup events; a subsequent setup event then dials the new member (FR-006..009, US1-AS1..4 incl. AS3 two-sidedly, repeated-setup EC)
- [X] T010 [US1] Write failing sync state tests for the **acceptor + activation side** in `src/state.rs`: membership-validated accept (downstream + `Send(Accepted)` to the carried emitter), validation-failure silent drop (no reply, no state), **acceptance succeeds for a membership-valid topic absent from the topic registry** (the revisit-flagged S7 pin: registration gates delivery, not acceptance), idempotent re-accept incl. failing re-validation, self-emitter drop, control invalid-signature drop, `Accepted` activation, unsolicited `Accepted`, unknown `Terminated` (FR-011..015, US1-AS5..7, ECs)
- [X] T011 [US1] Implement the `apply` arms to make T009–T010 pass, in `src/state.rs`: `NodeState` gains `upstream`/`downstream`/`strategy` + snapshot fns; named handlers `handle_connection_setup` (strategy + diff) and `handle_connection_message` → `handle_connection_request`/`_accepted`/`_terminated` (drop causes per contracts §3)
- [X] T012 [US1] Extend construction in `src/node.rs` + `src/error.rs`: `Node::new(self_id, config, network, signer, verifier, subscription_registry, topic_registry, strategy)`, identity/signer coherence check **before** registration returning new `NodeError::IdentityMismatch`, state seeded with strategy; update every `Node::new` call site in the same increment — `src/main.rs`, `tests/common/mod.rs` (four calls incl. `node_sharing`), the direct call in `tests/candidate_set.rs`, and the `no_run` doctest in `src/network.rs` (doctests compile) — (suites stay green — the receive gate is not yet in)
- [X] T013 [US1] Add the setup delay end to end: TOML `connection_setup_delay_ms: Option<u64>` → `NodeConfig.connection_setup_delay: Option<Duration>` in `src/config.rs` (loader conversion; `deny_unknown_fields` updated), `setup_timer_producer` named async fn spawned via `spawn_producer` only when `Some` in `src/node.rs`; loader cases in `tests/config_loading.rs`
- [X] T014 [US1] Add public snapshot getters `upstream_connections()` / `downstream_connections()` in `src/node.rs` and complete the `src/lib.rs` re-export delta per contracts §4 as its **single owner** beyond T004's module types — notably T005's message types (`ConnectionMessage`, `PlainConnection`, `ConnectionAction`; `keypair_from_alias` needs no lib.rs work, being a method on the already-exported `MockCryptoScheme`)
- [X] T015 [US1] Extend `tests/common/mod.rs`: alias-keypair fixtures, establishment preamble (topic-registry `set_topic` + subscription-registry `set_topics` → `await_candidates` → push `Event::ConnectionSetup` via `Node::events()`), `await_connection`/`await_upstream_active`-style helpers
- [X] T016 [US1] Write integration tests in `tests/connections.rs`: full bidirectional per-topic graph for N nodes (SC-001), partial-convergence subset stays static, empty-view node still accepts inbound, two-topics-two-connections, **one** configured-timer test (short real delay), construction failures — duplicate registration and identity mismatch (N-006) ⛳

## Phase 4: User Story 2 — connection-gated delivery (P1) — single-commit break

**Goal**: the receive path admits payload only from Active upstreams; the chartered compatibility break + suite rework land together.

**Independent test**: connected vs unconnected sender of the same valid message — only the connected one recorded; the other drops `not_connected` (spec US2).

- [X] T017 [US2] Write failing sync state tests in `src/state.rs`: gate order: connection first, the merged chain (subscription → registration → authorization → signature) unchanged after it; `not_connected` for absent and AwaitingAccept connections; per-topic gating (T's connection does not admit U); post-connection behaviors unchanged (subscription filter, valid recording — FR-016/019, US2-AS1..4)
- [X] T018 [US2] Implement the connection gate as the first check in `handle_signed_message` in `src/state.rs` (frame-sender keyed per FR-016; existing checks untouched after it)
- [X] T019 [US2] Rework the pre-existing suites with establishment preambles in `tests/two_node_ping.rs`, `tests/topic_filter.rs`, `tests/n_node_graph.rs`, `tests/topic_validity.rs`, `tests/topic_registry_network.rs` (the two 013 suites send payload and fall under the gate too), re-asserting the post-connection regression boundary (SC-005); **T017–T019 are one commit** with T020
- [X] T020 [US2] Add the unconnected-sender integration test (valid signed message from a non-connected peer → not recorded, getter-observable) in `tests/connections.rs` ⛳

## Phase 5: User Story 3 — silent misbehavior severance (P2)

**Goal**: invalid signature over an Active, subscribed connection severs it silently with the semantic effect.

**Independent test**: establish → one tampered message → upstream gone, misbehavior signal raised → subsequent valid message drops `not_connected` (spec US3, SC-003).

- [X] T021 [US3] Write failing sync state tests in `src/state.rs`: severance (entry removed + `Effect::Misbehaved`, **no** `Send`) only when every earlier check passed (connection, subscription, registration, authorization); earlier-check failures never sever; other topics/peers untouched; post-severance valid message → `not_connected` (FR-017/018, US3-AS1..4)
- [X] T022 [US3] Implement severance at the signature step of `handle_signed_message` in `src/state.rs` (executor's `connection_severed` warn already lands via T007)
- [X] T023 [US3] Add the misbehavior integration flow to `tests/connections.rs` (tamper → severed → subsequent valid excluded; offender's other-topic connection intact) ⛳

## Phase 6: User Story 4 — graceful shutdown & restart recovery (P2)

**Goal**: consuming `shutdown` notifies every entry (both roles, any state); abrupt restart re-converges via idempotent re-accept.

**Independent test**: graceful shutdown clears the survivor's entries; abrupt drop + restart re-dial returns to Active (spec US4, SC-004).

- [X] T024 [US4] Write failing sync state tests in `src/state.rs`: `handle_shutdown` clears both structures and returns one `Send(Terminated)` per entry incl. AwaitingAccept; `Terminated` reception removes the matching entry in either role (FR-014/020, US4-AS1..2)
- [X] T025 [US4] Implement `handle_shutdown` in `src/state.rs` and `pub async fn shutdown(mut self)` in `src/node.rs` (push `Event::Shutdown`; loop breaks after executing a Shutdown event's effects per ADR 0019's recorded carve-out; `(&mut self.event_loop).await` with `JoinError` logged-ignored; `Drop` untouched)
- [X] T026 [US4] Add shutdown/restart integration tests to `tests/connections.rs`: graceful shutdown → zero dangling counterpart entries; abrupt drop → stale entries retained (harmless); restart under same alias → re-dial → idempotent re-accept → Active (US4-AS3..4) ⛳

## Phase 7: User Story 5 — observable, deterministically testable lifecycle (P3)

**Goal**: the whole machine walkable synchronously via scripts; diagnostics visible through getters.

**Independent test**: scripted lifecycle (setup → accept → misbehave → shutdown) asserted step-by-step via snapshots, no timers (spec US5, SC-006).

- [X] T027 [US5] Add the full-lifecycle `ConnectionScript` sync test in `src/state.rs` (every spec-defined transition reachable by fed events alone; snapshot assertions per step; determinism re-run) and a stuck-AwaitingAccept state test (request to an absent peer stays pending, admits nothing — US5-AS2/3, SC-006/SC-007 self-exclusion sweep)
- [X] T028 [US5] Add the getter-consistency integration test in `tests/connections.rs`: snapshots are stable clones (unaffected by subsequent events) and pending entries are visible diagnostics (US5-AS1) ⛳

## Phase 8: Polish & cross-cutting obligations

- [ ] T029 [P] Update `specs/IMPLEMENTATION_NOTES.md`: mark N-002 and N-006 resolved by this feature; add five deferral entries — stale-AwaitingAccept GC (trigger: dynamic transitions), Active-connection liveness (trigger: 009), identity-binding hardening (trigger: real crypto/011), misbehavior follow-ups package (blacklist, re-selection, topic-based misbehavior), and the acceptance-vs-registered-topics revisit (trigger: resolution of the cross-registry event-ordering invariant raised on the 013 PR; if rejected, add the registration check to acceptance or cascade topic removals into membership) — each cross-referencing its data-model staleness-catalog row (S1–S7); N-010 was already added mid-Phase-6 (in-memory network has no deregistration → literal same-alias restart inexpressible), so these five new entries take N-011..N-015
- [ ] T030 [P] Add the §1.3 supersession note to `specs/event-loop-and-registry-contract.md` (per-connection producers deferred to a real connection-oriented transport; 004-connections shipped logical connections over the unchanged single mailbox)
- [ ] T031 [P] Refresh `Node`/module rustdoc in `src/node.rs`, `src/peer.rs`, `src/connection.rs`, `src/message.rs`: document the new connection surface in stable operator/library terms (no FR citations) — 013 already fixed the old subscribe/unsubscribe staleness; the doc now describes the two-registry fold and must additionally describe connections, shutdown, and the gated receive path
- [ ] T032 Verify-against-code pass for contracts §4: grep `src/lib.rs` re-exports and module visibility (`NodeState`/`Effect` stay crate-internal; `as_str` gone; getter signatures) and reconcile contracts/quickstart if the code diverged
- [ ] T033 Final validation: full sweep, quickstart.md walked against the real API (code blocks compile-accurate), spec SC-001..007 checklist against the suite ⛳

## Dependencies

```text
Phase 1 (PeerId)  →  Phase 2 (vocabulary)  →  Phase 3 (US1)  →  Phase 4 (US2)  →  Phase 5 (US3)
                                                                            ↘  Phase 6 (US4)
US3/US4 are independent of each other (both need US1+US2's establishment + gate)
Phase 7 (US5) needs all arms it scripts (after US3+US4)
Phase 8 last (T032 needs the final public surface)
```

Story-level: US1 ⊥ nothing (foundation consumer); US2 needs US1's establishment to test admission; US3/US4 need US1+US2; US5 scripts everything. The MVP increment is **US1** (observable topology) with **US2** as the first behavioral payoff.

## Parallel execution examples

- Phase 1: T001 ∥ (T002 after T001 for the agreement test; T003 serial)
- Phase 2: T004 ∥ T005 ∥ T006, then T007
- Phase 3: T008 ∥ T009-drafting; after T012: (T013 → T014, serial — both edit `src/node.rs`) ∥ T015
- Phase 8: T029 ∥ T030 ∥ T031, then T032 → T033

## Implementation strategy

Two foundational green checkpoints (identity, vocabulary) before any behavior; then
stories in priority order, each independently testable and committed green; the one
deliberate compatibility break (Phase 4) is a single commit pairing the gate with the
reworked suites. Strict TDD inside every story: T009/T010, T017, T021, T024 must fail
before T011, T018, T022, T025 respectively.

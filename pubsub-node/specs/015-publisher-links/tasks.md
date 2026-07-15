# Tasks: Publisher links and dissemination-model configurations (M3/M4/M5)

**Input**: Design documents from `/specs/015-publisher-links/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R14), data-model.md, contracts/link-kinds-and-seams.md, quickstart.md

**Tests**: MANDATORY — this feature carries protocol-behaviour claims (receive-gate admission, owner-binding, severance, fan-out semantics, reciprocity). Test tasks precede implementation within each story and must fail first. The foundational reshape (Phase 2) is behaviour-preserving and is covered by the *existing* suite staying green — its "tests" are the current 200+ tests plus the deliberately updated wire-layout pin (research R3).

**ADRs**: one — `docs/decisions/0032-publisher-links-and-model-family.md` (research R14), authored in Phase 2 while the structural choices land.

**Reference**: `archive/015-full-exploration` holds the prior full implementation; port test *scenarios* from it (adjusting to the minimal API), never its abstraction layer.

## Phase 1: Setup

*(No setup tasks — existing crate, no new dependencies, branch already cut.)*

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the kind-in-key reshape and seam-signature changes every story builds on — behaviour-preserving (an M2 node is bit-identical before/after), existing suite green at each checkpoint, only mechanical renames + the wire-layout pin update in test files.

- [x] T001 Introduce `LinkKind` and `LinkKey` (topic-first field order, derives only) and rename `UpstreamState` → `LinkState` in src/connection_state.rs; re-export from src/lib.rs
- [x] T002 Add `kind: LinkKind` to `PlainConnection`, append the signed kind tag byte (0x00 Relay / 0x01 Publisher) in `signed_bytes()` in src/message.rs; update the layout-pin and tamper tests in the same file to the new canonical layout (deliberate exception, research R3)
- [x] T003 Reshape `NodeState.upstream`/`downstream` to `BTreeMap<LinkKey, LinkState>` in src/state.rs: rewrite the relay-only handlers against the new keys (heartbeat diff, request/accepted/rejected/terminated, receive gate, shutdown, topic-removal cascade) preserving today's semantics exactly; rename snapshots to `upstream_relays()`/`downstream_relays()` and add empty-returning `upstream_publishers()`/`downstream_publishers()`
- [x] T004 Update `NodeView` to borrow both link maps (`upstream`, `downstream`) in src/strategies/view.rs; generalise `downstream_scan` → kind-aware `link_scan` in src/strategies/acceptance/mod.rs (relay instances scan downstream × Relay — unchanged semantics); update the strategy test_support view builders in src/strategies/test_support.rs
- [x] T005 Rename `ConnectionStrategy::expected_upstream` → `expected_links` in src/strategies/connection/mod.rs and both implementors (connect_to_all.rs, hash_gated.rs); update the heartbeat call site in src/state.rs
- [x] T006 Change `FanoutStrategy::targets` to `(topic, downstream: &BTreeMap<LinkKey, LinkState>, origin: &Origin, exclude)` in src/strategies/fanout/mod.rs; update `ForwardToAll` (relay entries only for now — publisher facet arrives in US1) in src/strategies/fanout/forward_to_all.rs with per-peer `BTreeSet` dedup; thread `origin` through `fanout()`/`record_and_fanout()` in src/state.rs
- [x] T007 Extend `NodeStrategies` to four slots (publisher pair as `Option`), add `symmetric: bool` + publisher-degree fields to the params structs in src/strategies/config.rs; change `Node::new` to take `NodeStrategies` + fanout + `PublisherAdmission` (placeholder enum, default `OwnerOnly`) in src/node.rs and `NodeState::new` accordingly in src/state.rs; add the four link getters to src/node.rs
- [x] T008 Rename CLI flags `--connection-strategy`/`--acceptance-strategy`/`--target-degree` → `--relay-strategy`/`--relay-acceptance-strategy`/`--relay-degree` in src/main.rs (publisher/fanout/admission/symmetric flags arrive with their stories)
- [x] T009 Mechanical rename sweep across src/state/tests/ and tests/ (getter names, `LinkState`, script-constructor signatures, `Node::new` call sites via the shared test builders in tests/common/); full suite + clippy + fmt green — the M2-baseline checkpoint (spec US4/SC-004)
- [x] T010 Author docs/decisions/0032-publisher-links-and-model-family.md (kind-in-key shape, wire kind byte, seam reuse, origin-aware fan-out, `PublisherAdmission`, symmetric mode; supersession note for the archive branch's ADRs 0032–0036) and add it to docs/decisions/README.md index

**Checkpoint**: suite green, M2 behaviour bit-identical, publisher getters return empty — user stories can begin.

---

## Phase 3: User Story 1 — Configure a node fleet for M3 (publisher links) (Priority: P1) 🎯 MVP

**Goal**: strategy-driven, unconditional publisher links; locally-published messages ride relay + publisher links; publisher links never relay; owner-binding on the receive side.

**Independent Test**: `tests/publisher_links.rs` — a small fleet with the M3 recipe: links established at readiness regardless of relay topology; publish reaches publisher-link targets; a relayed message never crosses a publisher link; foreign publisher over a publisher link dropped.

### Tests for User Story 1 (write first, must fail)

- [x] T011 [P] [US1] State-machine tests in src/state/tests/: publisher heartbeat pass dials expected publisher links unconditionally (with zero and with full relay downstream — FR-002); publisher `Request` dispatch inserts upstream × Publisher `Active` and replies `Accepted` (kind Publisher); `None` publisher acceptance silently drops (FR-014); `Accepted`/`Rejected`/`Terminated` with kind Publisher mutate the publisher entries only (FR-015)
- [x] T012 [P] [US1] Gate tests in src/state/tests/: owner-bound admission over upstream × Publisher (owner admitted, foreigner dropped — FR-006); invalid signature over a publisher link severs the publisher entry, not a relay entry (FR-010); relay admission unchanged
- [x] T013 [P] [US1] Fan-out tests in src/state/tests/: local origin → relay ∪ publisher-Active targets; peer origin → relay only (FR-005); peer present as both kinds receives one send (FR-011)
- [x] T014 [P] [US1] Acceptance-cap test in src/state/tests/ or src/strategies/acceptance/: relay and publisher bounded acceptance count independently (FR-004)
- [x] T015 [P] [US1] Integration test tests/publisher_links.rs (port scenarios from the archive branch): M3 recipe end-to-end — unconditional establishment, delivery over publisher links, owner-binding rejection

### Implementation for User Story 1

- [x] T016 [P] [US1] Add `is_valid_edge_in(domain, …)` internal + `is_valid_edge_publisher` (`…/publisher-edge/v1`) in src/strategies/edge.rs (existing `is_valid_edge` signature/domain untouched)
- [x] T017 [US1] Add `kind: LinkKind` field to `HashGatedConnection` (src/strategies/connection/hash_gated.rs) and the hash-gated acceptance baselines (src/strategies/acceptance/hash_gated.rs, hash_gated_bounded.rs) selecting the domain; constructor defaults keep `Relay`; publisher acceptance instances scan upstream × Publisher via `link_scan`
- [x] T018 [US1] Publisher heartbeat pass in `handle_heartbeat` (src/state.rs): `if let Some(strategy)` — diff over downstream × Publisher, insert `AwaitingAccept`, send kind-Publisher `Request`; behind the existing `synced` gate; never reads relay entries
- [x] T019 [US1] Kind dispatch in the control handlers (src/state.rs): request → publisher acceptance slot (or `publisher_links_disabled` drop), accepted → activate downstream × Publisher, rejected → remove downstream × Publisher `AwaitingAccept`, terminated → remove the carried kind from both maps; shutdown and topic-removal cascade already iterate both maps (T003) — extend the `Terminated` notices with each entry's kind
- [x] T020 [US1] Receive gate + severance in `handle_dissemination` (src/state.rs): admit via upstream × Relay `Active` or upstream × Publisher under `PublisherAdmission::OwnerOnly` owner-binding; sever the admitting `LinkKey` on signature failure
- [x] T021 [US1] `ForwardToAll` gains the publisher facet: relay ∪ (publisher-Active iff `Origin::Local`) in src/strategies/fanout/forward_to_all.rs
- [x] T022 [US1] Publisher CLI flags (`--publisher-strategy`, `--publisher-acceptance-strategy`, `--publisher-degree`) in src/main.rs; two-phase build of the optional publisher pair in src/strategies/config.rs; extend ConnectionScript with publisher-kind steps (`publisher_request_from`, `publisher_accepted_from`, …) in src/connection_state.rs test_support
- [x] T023 [US1] Green checkpoint: T011–T015 pass, full suite + clippy + fmt green; commit

**Checkpoint**: M3 configurable end-to-end; M2 unchanged (defaults).

---

## Phase 4: User Story 2 — M4 bidirectional links (Priority: P2)

**Goal**: `--symmetric-edges` drives an order-independent relay edge predicate on both relay seams; reciprocal pairs emerge; flood covers a predicate-connected graph.

**Independent Test**: `tests/model_family.rs` M4 case — every relay link reciprocated; one publish reaches all nodes.

### Tests for User Story 2 (write first, must fail)

- [x] T024 [P] [US2] Predicate tests in src/strategies/edge.rs: `is_valid_edge_sym` is order-independent, uses its own domain (draws differ from directional for already-canonical pairs), density ≈ 1/B
- [x] T025 [P] [US2] Integration test tests/model_family.rs (port from archive): M4 recipe — pairwise reciprocity of `upstream_relays`/`downstream_relays` across the fleet + full-coverage flood over a genesis whose predicate graph is connected (export/reuse the predicate helpers to find one deterministically)

### Implementation for User Story 2

- [x] T026 [US2] `is_valid_edge_sym` (`…/edge-sym/v1`, canonical byte-order pair) in src/strategies/edge.rs
- [x] T027 [US2] `symmetric: bool` on `HashGatedConnection` and the hash-gated acceptance baselines (selects the symmetric predicate; publisher instances always directional); thread through src/strategies/config.rs params
- [x] T028 [US2] `--symmetric-edges` flag in src/main.rs setting both relay seams' `symmetric` from the one flag (contract §5); green checkpoint + commit

**Checkpoint**: M4 configurable; US1 behaviour untouched (flag off by default).

---

## Phase 5: User Story 3 — M5 directed k_in/k_out, everything-carrying (Priority: P3)

**Goal**: union fan-out (`all-links`) + relaxed publisher admission (`any-verified`) turn publisher links into everything-carrying k_out links.

**Independent Test**: `tests/model_family.rs` M5 case — a→b→c delivery over standing publisher links only, the hop `owner-only` drops.

### Tests for User Story 3 (write first, must fail)

- [x] T029 [P] [US3] State tests in src/state/tests/: `AnyVerified` admits a foreign publisher's message over upstream × Publisher; `OwnerOnly` (default) drops the same arrival (FR-008); `AllLinks` fan-out sends peer-origin messages over publisher links too, deduplicated (FR-007/011)
- [x] T030 [P] [US3] Integration test in tests/model_family.rs (port from archive): M5 chain a→b→c over publisher links with `any-verified` + `all-links`; same topology under defaults does NOT deliver (the M3 exclusivity pin); await Active handshakes before publishing (archive lesson — no retry means a pre-handshake publish is lost)

### Implementation for User Story 3

- [x] T031 [P] [US3] `AllLinks` fan-out strategy in src/strategies/fanout/all_links.rs + `FanoutStrategyKind` (`forward-to-all` | `all-links`) in src/strategies/fanout/kind.rs; re-export in src/lib.rs
- [x] T032 [US3] `PublisherAdmission::AnyVerified` arm in the receive gate (src/state.rs) — severance stays policy-independent
- [x] T033 [US3] `--fanout-strategy` and `--publisher-admission` flags (`FromStr` at the edge) in src/main.rs; green checkpoint + commit

**Checkpoint**: all three recipes configurable; axes independent (SC-005).

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T034 [P] Verify quickstart.md recipes against the shipped flags (spec-fidelity pass per constitution: grep lib.rs re-exports and main.rs flags against contracts §4/§5); fix docs, not code, where they drift
- [ ] T035 [P] Sync the CLAUDE.md SPECKIT active-feature block to the final shape (getters, flags, ADR 0032)
- [ ] T036 Full-suite audit: cargo test + clippy + fmt, confirm pre-existing test files show only mechanical renames (spec SC-004) — `git diff --stat archive-base` review; commit
- [ ] T037 Run /speckit-analyze and record findings + resolutions in specs/015-publisher-links/analysis.md (constitution Development Workflow)

---

## Dependencies & Execution Order

- **Phase 2 is strictly sequential-ish**: T001 → T002/T005 [P-capable] → T003 → T004 → T006 → T007 → T008 → T009 → T010 (ADR can be drafted alongside from T003 on).
- **US1 (Phase 3)** depends on Phase 2; T011–T015 [P] first (fail), then T016 [P] / T017 → T018 → T019 → T020 → T021 → T022 → T023.
- **US2 (Phase 4)** depends on Phase 2 only (not US1 — symmetric mode touches relay seams; run after US1 anyway to keep one green line).
- **US3 (Phase 5)** depends on US1 (publisher links must exist to carry everything).
- **Polish (Phase 6)** last; T034/T035 [P].

## Implementation Strategy

MVP = Phase 2 + Phase 3 (M3 — the adopted primary model; M2 baseline proven by the untouched suite). US2 and US3 are small, independent increments on top (two flags + one strategy each). Commit at every green checkpoint (T009, T023, T028, T033, T036); each commit compiles and passes the full suite (constitution Development Workflow).

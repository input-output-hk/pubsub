---

description: "Task list for feature 015 — unified connection link model & publishing links"
---

# Tasks: Unified connection link model (role + direction) & publishing links

**Input**: Design documents from `specs/015-connection-link-model/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R10), data-model.md, contracts/link-model-and-seams.md

**Tests**: MANDATORY. The behaviour-preserving migration (FR-012/SC-001), origin-restricted forwarding (FR-005/FR-006/SC-002), publisher binding (R5), the M3 trigger (FR-009b/SC-003), degree independence (FR-009/SC-004), and dedup across roles (FR-011/SC-005) are **critical** per plan.md — test tasks precede implementation and MUST fail first (Constitution II).

**ADRs**: 0032 (unified link store + role-carrying handshake), 0033 (publishing-link seams: deterministic trigger, degree/domain separation, role-aware acceptance, publisher-binding receive gate).

**Organization**: by user story. US1 (behaviour-preserving migration to the link store) is the foundation; US2 adds origin-restricted forwarding over `Publisher` links; US3 adds establishment (selection seam + trigger + role-aware acceptance + CLI).

## Format: `[ID] [P?] [Story] Description`

## Path Conventions

Single Rust project: sources under `pubsub-node/src/`, tests under `pubsub-node/tests/`. Paths repo-relative.

---

## Phase 1: Setup

- [X] T001 ADR 0032 (unified link store `BTreeMap<(PeerId, TopicId, LinkRole, LinkDirection), LinkState>`; role × direction orientation rule; `Both` emergent not stored; `ConnectionAction` role field + signed-bytes role tag) in `pubsub-node/docs/decisions/0032-unified-link-store-and-role-handshake.md`
- [X] T002 ADR 0033 (publishing-link seams: `PublishStrategy` + deterministic expected-downstream trigger; `publish_degree` + `publish-edge/v1` domain; role-aware acceptance dispatch with a second slot; publish cap disjoint from relay `OC`; publisher-binding receive gate `relay_over_publish_link`) in `pubsub-node/docs/decisions/0033-publishing-link-seams.md`

## Phase 2: Foundational (blocking)

- [X] T003 Mechanical rename `target_degree` → `relay_degree` across `src/` (config params, CLI `--relay-degree`, strategy fields, `edge.rs` signatures/docs) and test call sites; suite green (FR-009a)
- [X] T004 Link vocabulary in `src/connection_state.rs`: `LinkRole`, `LinkDirection` (`#[non_exhaustive]`), rename `UpstreamState` → `LinkState`; derive `Ord` on key components; export from `lib.rs`

## Phase 3: US1 — Behaviour-preserving migration (P1) 🎯 foundation

### Tests first ⚠️

- [X] T005 [US1] Migration parity pins: existing `state/tests/*` and `tests/connections.rs` updated only for the `LinkState` rename + role-carrying control constructors — assertions unchanged; full suite red only where the new store/wire is not yet in place
- [X] T006 [P] [US1] Wire pin: `signed_bytes` layout test extended with the role tag byte (relay + publisher variants distinct) in `src/message.rs` tests

### Implementation

- [X] T007 [US1] `ConnectionAction` variants gain `role: LinkRole`; `signed_bytes` appends the role tag; encoder doc updated in the same commit (R3) in `src/message.rs`
- [X] T008 [US1] Replace `upstream`/`downstream` with the `links` store; migrate every transition (heartbeat diff, request/accepted/rejected/terminated, shutdown, registry-removal cascade, dissemination gate) keyed by role; `NodeView` carries `links` + `inbound_scan(role, …)` in `src/state.rs`, `src/strategies/view.rs`
- [X] T009 [US1] Acceptance prelude (`admit_prelude`/`downstream_scan`) role-scoped via the view helpers in `src/strategies/acceptance/mod.rs`
- [X] T010 [US1] `Node::links()` getter; `upstream_connections`/`downstream_connections` as Relay-scoped views; exports in `src/node.rs`, `src/lib.rs`
- [X] T011 [US1] `ConnectionScript` role-aware steps (existing steps emit `Relay`) in `src/connection_state.rs`

**Checkpoint**: full suite green; relay-only behaviour identical (SC-001).

## Phase 4: US2 — Origin-restricted forwarding (P1)

### Tests first ⚠️

- [X] T012 [US2] State tests: node holding `(b,t,Publisher,Out,Active)` + `(c,t,Relay,In)`: publish on `t` → sends to both; `Origin::Peer` delivery → relay target only (FR-005/FR-006/SC-002); split-horizon regardless of role (FR-007); publish-link-only topic + `Origin::Peer` → no targets, in `src/state/tests/fanout.rs`
- [X] T013 [P] [US2] Receive-gate tests: payload from peer holding `(from,t,Publisher,In)` admitted iff `publisher_id == from`; foreign-publisher payload dropped (`relay_over_publish_link`, R5); dedup: same message over publish + relay paths recorded once (FR-011/SC-005), in `src/state/tests/gated_receive.rs`

### Implementation

- [X] T014 [US2] `FanoutStrategy::targets(topic, links, origin, exclude)`; `ForwardToAll` = In/Relay ∀origin ∪ Active Out/Publisher when `Origin::Local` in `src/strategies/fanout/`
- [X] T015 [US2] Thread `Origin` through `record_and_fanout`/`fanout`; receive gate extended with the In/Publisher + publisher-binding arm in `src/state.rs`

**Checkpoint**: US2 acceptance scenarios green.

## Phase 5: US3 — Establishment (P2)

### Tests first ⚠️

- [X] T016 [US3] Trigger tests: publisher with zero expected relay downstream forms hash-selected `Out/Publisher` links on `Heartbeat`; with expected downstream present forms none (FR-009b/SC-003); publish selection unaffected by `relay_degree` sweep and vice versa (FR-009/SC-004), in `src/strategies/publish/` unit tests + `src/state/tests/connection.rs`
- [X] T017 [P] [US3] Acceptance tests: publish-intent request admitted via the publish slot; cap `⌈publish_degree + c·√publish_degree⌉` counted only against In/Publisher links (relay `OC` untouched); readiness gate holds for publish requests (FR-010), in `src/strategies/acceptance/` + state tests
- [X] T018 [P] [US3] Integration: pure publisher (no relay upstream/downstream) publishes → message reaches overlay via publish links; dual-role pair (Relay + Publisher links between same peers) coexists (Clarifications), in `tests/publish_links.rs`

### Implementation

- [X] T019 [US3] `strategies::edge` publish-domain predicate (`publish-edge/v1`; relay bytes unchanged) in `src/strategies/edge.rs`
- [X] T020 [US3] NEW seam `src/strategies/publish/`: `PublishStrategy` trait, `NoPublishLinks` (default), `HashGatedPublish` (trigger + selection, R6), `PublishStrategyKind` (`none` | `hash-gated`)
- [X] T021 [US3] Role-aware request dispatch + publish acceptance slot on `NodeState`; publish dial pass in `handle_heartbeat` in `src/state.rs`
- [X] T022 [US3] Two-phase construction extended (`PublishParams`, `PublishAcceptanceParams`; role/domain-parameterised hash-gated acceptance instances) in `src/strategies/config.rs`, `src/strategies/acceptance/`
- [X] T023 [US3] CLI flags `--publish-strategy`/`--publish-degree`/`--publish-acceptance-strategy`; `Node::new` wiring in `src/main.rs`, `src/node.rs`

## Phase 6: Polish

- [X] T024 `cargo fmt` + `cargo clippy --all-targets -- -D warnings` clean; quickstart claims verified against the binary
- [X] T025 Update `pubsub-node/CLAUDE.md` active-feature block for 015
- [X] T026 `/speckit-analyze` pass recorded in `specs/015-connection-link-model/analysis.md`

## Phase 7: Model-family rework (post-review, ADR 0034)

- [X] T027 ADR 0034 (model-family seams; amends 0033 §1–2) in `pubsub-node/docs/decisions/0034-model-family-seams.md`
- [X] T028 Remove the M3 trigger: standing initiation links select unconditionally (`m3/README.md`); publish strategy loses its `relay_degree` param
- [X] T029 Unify the dial seams: one `LinkSelectionStrategy` family (`none` | `connect-to-all` | `hash-gated` with role) under `strategies/selection/`; acceptance params gain `role` (one build path, `build_for_role` folded away)
- [X] T030 Cell-structured `LinkStore` (`relay_out`/`relay_in`/`publish_out`/`publish_in`) replacing the quad-keyed map; `NodeView.links: &LinkStore`
- [X] T031 Fan-out kinds: `forward-to-all` (default, union) + `role-scoped` (strict M3 partition, mute-publisher caution documented); `--fanout-strategy` CLI flag; `NodeStrategies` gains the fanout slot
- [X] T032 Test/docs sweep: trigger tests retired, selection/fan-out kind tests added, integration test re-premised on unconditional initiation links; spec/contract/data-model/quickstart/analysis (A8) updated

## Phase 8: M4/M5 in-feature (maintainer-directed, ADR 0035)

- [X] T033 ADR 0035 (symmetric edges + publish-in admission) in `pubsub-node/docs/decisions/0035-symmetric-edges-and-publish-in-admission.md`
- [X] T034 `edge::is_valid_edge_sym` — unordered-pair hashing under per-role `…-sym/v1` domains; symmetry, domain-separation, and density tests
- [X] T035 `symmetric` mode on `HashGatedSelection` + hash-gated acceptance kinds, wired by one `--symmetric-edges` flag through both relay params; reciprocity test
- [X] T036 `role-agnostic` fan-out kind (né `flood-all`) (relay-in ∪ publish-out, any origin, minus arrival, per-peer dedup) + unit tests
- [X] T037 `PublishInAdmission { OwnerOnly, AnyVerified }` on `NodeState`/`Node::new` + `--publish-in-admission`; gate + severance state tests
- [X] T038 Integration `tests/model_family.rs`: M4 reciprocity + full-coverage flood (predicate-connected 12-node graph); M5 foreign-publisher relay a→b→c over standing links only
- [X] T039 Docs: spec session/FR-013..015, contract, data-model, quickstart model recipes, CLAUDE.md, analysis A11
- [X] T040 Flow-oriented `LinkStore` (sources/sinks facets; role × direction as stable view; zero test churn) — ADR 0036, analysis A12
- [X] T041 ADR hygiene pass (forward-pointing status banners 0017–0034, `docs/decisions/README.md` index) + round-3 cleanup (dead API removed, In-facet insert guarded, doc freshness) — analysis A13

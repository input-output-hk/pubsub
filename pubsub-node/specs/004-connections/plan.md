# Implementation Plan: Logical Connection Management with Autonomous Static Topology

**Branch**: `004-connections` | **Date**: 2026-06-12 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/004-connections/spec.md` — converged across nine clarify rounds (trajectory recorded in its Clarifications section) — plus the agreed plan-input text reproduced verbatim in [Plan input](#plan-input) below.

## Summary

Nodes gain logical, per-(peer, topic) connections — upstream (requested; message
sources) with explicit `AwaitingAccept`/`Active` states, downstream (accepted;
fan-out destinations) as a set — established autonomously by an injected
connection-selection strategy on a setup event (optional one-shot timer, unset by
default, or public-intake injection), torn down by signed control messages and a new
consuming `shutdown`. The receive path becomes connection-gated (connection →
subscription → registration/authorization (013) → signature, severing silently on invalid signatures over Active
upstreams), `PeerId` becomes key-backed (the `PublisherId` pattern, with a mock-stage
alias rule preserving readable fixtures/logs), and the `Effect` type gains its first
inhabitants. Everything dynamic (re-selection, GC, deny path, blacklist, liveness)
stays deferred and is catalogued as documented stale states.

Technical approach per `research.md` R1–R10 and ADRs 0017 (key-backed identity +
signed control messages), 0018 (strategy seam), 0019 (shutdown lifecycle): all
decisions land in the pure core (`state.rs` arms with named handlers), the shell
gains only the setup-timer producer, the effect executor (a `NetworkSender` clone in
the loop task), and `shutdown`'s await-the-loop mechanics.

## Technical Context

**Language/Version**: Rust, edition 2021, `rust-version = 1.75` (workspace unchanged)

**Primary Dependencies**: tokio 1 (rt, sync, time), serde/toml (config), tracing,
thiserror, clap (CLI), sha2 + rand/rand_chacha (mock crypto) — no new dependencies
(no ADR needed under the justified-dependencies standard)

**Storage**: none (in-memory state; TOML config files at the loader edge)

**Testing**: cargo test — synchronous state-machine tests in `src/state.rs` (with a
`ConnectionScript` declarative builder), module tests beside new types, reworked
integration suites under `tests/` with `tests/common` helpers; proptest available
(dev-dependency) where a property claim warrants it

**Target Platform**: library + CLI binary, platform-agnostic (development on macOS/Linux)

**Project Type**: single Rust crate (library with a thin binary), inside the parent
`pubsub/` repo

**Performance Goals**: n/a at this stage (in-memory PoC; correctness over optimization)

**Constraints**: pure/sync transition core (no I/O, no awaits under the state lock);
effects executed outside the lock; reproducible tests (no wall-clock assertions; the
single timer-path test uses an explicitly configured short delay); logs are operator
UX, never a test surface

**Scale/Scope**: small fixed test topologies (2–N in-process nodes); the full graph
per topic is N−1 connections per node per role

## Constitution Check

*GATE: evaluated pre-Phase-0 and re-checked post-Phase-1 (both pass; no Complexity
Tracking entries needed).*

- **I. Correctness Over Optimization — ✅** Every behavior traces: spec FR-001..028 +
  Clarifications; research R1–R10; ADRs 0017/0018/0019; data-model state machines map
  transitions to FRs; the staleness catalog maps every deliberate gap to its spec edge
  case and deferral.
- **II. Test-Driven for Correctness Claims — ✅ (TDD required)** Connection lifecycle,
  control-message verification, and connection-gated delivery are protocol-behavior
  claims; this feature is designated **critical**: tasks must order failing tests
  before implementation (state-machine scripts first, then integration). The spec's
  SC-006 (every transition reachable by feeding events, no timers) is the test-shape
  contract.
- **III. Document Structural Decisions as ADRs — ✅** Authored with this plan:
  ADR 0017 (key-backed `PeerId` + signed control messages), ADR 0018 (strategy seam +
  setup producer), ADR 0019 (shutdown lifecycle, amends 0012 with a recorded
  loop-break carve-out). Tactical choices (names, causes, module layout) are recorded
  in research.md/contracts and exempt.
- **IV. Specifications as Ambiguity Detectors — ✅** Ambiguities were surfaced and
  resolved through the recorded clarify rounds (e.g. the "always accept" over-reading,
  the check-order conflict with the ledger); none silently resolved here. No new
  protocol-doc ambiguity emerged during planning.
- **V. Specifications Are Read-Only — ✅** No edits to `pubsub/docs/` or
  `pubsub/formal_spec/`. The §1.3 supersession note targets
  `specs/event-loop-and-registry-contract.md`, a pubsub-node workstream doc (editable).

Engineering Standards applied: logs-not-a-test-surface (severance/drop events are
operator UX; assertions go through getters and effect lists); neutral operator strings
(`connection_severed`, snake_case causes, no FR citations); parse-at-the-edge (TOML
`connection_setup_delay_ms` → `Option<Duration>` in the loader; alias parsing is the
type's `FromStr`, file I/O stays in the loader); forward-compatible shapes justified
by named consumers (`#[non_exhaustive] ConnectionAction` → deny-path package;
`ConnectionStrategy` → ROADMAP 006/007; `Effect::Misbehaved` → blacklist package);
declarative test construction (`ConnectionScript` sibling of `MembershipScript`);
reproducible tests (no wall-clock assertions; seeded mock scheme).

## Plan input

> The agreed `/speckit-plan` input text is preserved verbatim in
> [`plan-input.md`](./plan-input.md) (committed alongside this plan), per the
> project's verbatim-input convention. All of its mandates are discharged by this
> plan's artifacts; the staleness-model mandate lands in `data-model.md` §3.

## Project Structure

### Documentation (this feature)

```text
specs/004-connections/
├── spec.md              # converged specification (9 clarify rounds)
├── plan.md              # this file
├── plan-input.md        # verbatim /speckit-plan input (agreed record)
├── research.md          # Phase 0 — planning decisions R1–R10
├── data-model.md        # Phase 1 — entities, state machines (Mermaid), staleness catalog
├── quickstart.md        # Phase 1 — lifecycle walkthrough on the new surface
├── contracts/
│   └── connection-protocol.md  # wire layout, validation/drop vocabulary, public-surface delta
├── checklists/requirements.md  # spec quality checklist (from /speckit-specify)
└── tasks.md             # Phase 2 (/speckit-tasks — not created by this command)

docs/decisions/
├── 0017-key-backed-peer-identity-and-signed-connection-control.md
├── 0018-connection-selection-strategy-seam.md
└── 0019-graceful-shutdown-lifecycle.md
```

### Source Code (repository root: `pubsub-node/`)

```text
src/
├── connection.rs        # NEW: UpstreamState, ConnectionStrategy, ConnectToAllCandidates,
│                        #      pub(crate) ConnectionScript test-support (cfg-gated section)
├── message.rs           # + Message::Connection, ConnectionMessage, PlainConnection,
│                        #   ConnectionAction (+ signed_bytes)
├── peer.rs              # PeerId reshaped over PublicKey (alias FromStr/Display; as_str removed)
├── crypto/mock.rs       # + MockCryptoScheme::keypair_from_alias
├── event.rs             # + Event::ConnectionSetup, Event::Shutdown
├── state.rs             # NodeState + upstream/downstream/strategy; new apply arms:
│                        #   handle_connection_message{,_request,_accepted,_terminated},
│                        #   handle_connection_setup, handle_shutdown; Effect::{Send,Misbehaved};
│                        #   handle_signed_message gains the connection gate + severance
├── node.rs              # Node::new(+signer,+strategy, coherence check), shutdown(self),
│                        #   setup_timer_producer, effect executor (NetworkSender clone),
│                        #   upstream_connections()/downstream_connections() getters
├── network.rs           # pub(crate) NetworkHandle::sender() accessor + Node::new doctest updated
├── config.rs            # + connection_setup_delay_ms → Option<Duration>
├── error.rs             # + NodeError::IdentityMismatch
├── lib.rs               # re-export delta per contracts §4
└── main.rs              # construction updated (signer from config-derived alias keypair, strategy)

tests/
├── common/mod.rs        # + alias-keypair fixtures, establishment preamble,
│                        #   await_connection-style helpers
├── two_node_ping.rs     # reworked: establishment preamble (not parity-preserving)
├── topic_filter.rs      # reworked: same
├── n_node_graph.rs      # reworked: same; full-graph assertions (SC-001)
├── topic_validity.rs    # reworked: 013 suite — gains establishment preambles
├── topic_registry_network.rs  # reworked: 013 suite — same
├── candidate_set.rs     # touched only where PeerId construction changes
├── config_loading.rs    # + connection_setup_delay_ms cases
└── connections.rs       # NEW: lifecycle integration (handshake, misbehavior, shutdown,
                         #   restart re-dial, timer path ×1, construction failures incl. N-006)
```

**Structure Decision**: single-crate layout unchanged; one new module
(`src/connection.rs`) for the connection domain types + strategy + test builder,
keeping `state.rs` focused on the transition arms (mirrors how
`subscription_registry/` hosts its domain away from the core).

## Decision summary (binding for /speckit-tasks)

| # | Decision | Where recorded |
|---|---|---|
| 1 | Coherence check before registration via existing `Signer::public_key()`; `NodeError::IdentityMismatch` | R1, ADR 0017, contracts §4 |
| 2 | `PeerId(PublicKey)` + alias `FromStr`/`Display` rule; `as_str` removed | R2, ADR 0017 |
| 3 | `Message::Connection` plain/signed split; tags 0x00/0x01/0x02; `#[non_exhaustive]` action enum | R3, ADR 0017, contracts §1 |
| 4 | Events `ConnectionSetup`/`Shutdown`; control dispatch inside `MessageReceived`; `Effect::{Send, Misbehaved}` | R4 |
| 5 | `ConnectionStrategy` sync trait, `Arc<dyn>` on `NodeState` beside the verifier; diff in `apply` | R5, ADR 0018 |
| 6 | `connection_setup_delay_ms: Option<u64>` TOML → `Option<Duration>`; timer = third owned producer, only when set | R6, ADR 0018 |
| 7 | Receive order: connection gate first, then the merged chain (subscription→topic-registered→publisher-authorized→signature) unchanged; severance at the signature step only; cause vocabulary fixed | R7, contracts §3, data-model §4 |
| 8 | `shutdown(self)` awaits the loop; Shutdown event = terminal marker (recorded carve-out); executor holds `NetworkSender` clone | R8, ADR 0019 |
| 9 | `keypair_from_alias` + `ConnectionScript` + `tests/common` establishment helpers | R9 |
| 10 | Severance log `connection_severed` (warn); drops via `message_dropped` | R10, contracts §3 |

## Post-013 reconciliation (2026-06-12)

Feature 013 (topic registry) merged to `main` after this plan converged; the
reconciliation pass updated this plan and its siblings in place: ADRs renumbered
0016/0017/0018 → **0017/0018/0019** (013 holds 0016); the `Node::new` baseline gains
the topic-registry parameter (contracts §4); the receive chain enumeration names the
merged checks (decision row 7; severance stays signature-only); the two 013 suites
join the rework list; and connection acceptance deliberately validates the
membership-derived set only — revisit-flagged pending the cross-registry
event-ordering invariant raised on the 013 PR (spec Clarifications 2026-06-12,
staleness catalog S7). `plan-input.md` is the pre-merge verbatim record and is
intentionally untouched.

## Post-plan obligations carried to /speckit-tasks (not done here)

- `IMPLEMENTATION_NOTES.md`: mark N-002 and N-006 resolved by this feature; add the
  five new deferral entries (stale-AwaitingAccept GC; Active-connection liveness at
  009; identity-binding hardening at real crypto; misbehavior follow-ups package;
  acceptance-vs-registered-topics revisit, S7) — each cross-referencing the
  data-model staleness catalog rows.
- `specs/event-loop-and-registry-contract.md` §1.3 supersession note (per-connection
  producers deferred to a real connection-oriented transport).
- Rustdoc refresh: `Node`'s doc block still references the removed
  subscribe/unsubscribe mutators (stale on main); rewritten anyway for the new surface.
- Not parity-preserving: the integration-suite rework is chartered work (spec FR-019
  boundary, SC-005), not collateral.

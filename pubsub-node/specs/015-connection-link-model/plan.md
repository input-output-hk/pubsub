# Implementation Plan: Unified connection link model (role + direction) & publishing links

**Branch**: `015-connection-link-model` | **Date**: 2026-07-13 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/015-connection-link-model/spec.md`

> **Model-family rework note (2026-07-13, ADR 0034).** After Denis's executable dissemination models (M1–M5) landed on `main`, parts of this plan were superseded — see the spec's "model-family alignment" Clarifications session and ADR 0034: the **M3 trigger is removed** (standing initiation links are unconditional per `m3/README.md`); the separate `PublishStrategy` seam merged into one role-parameterised **`LinkSelectionStrategy`** family; the store became **flow-oriented** (`LinkStore` sources/sinks, ADR 0036, after an interim cell-structured shape); and fan-out gained kinds (`forward-to-all` | `role-scoped` | `role-agnostic`) as the dissemination-model knob. The Summary and ADR list below describe the design as first planned; `data-model.md`, `contracts/`, and `analysis.md` (A7–A8) reflect what shipped.

## Summary

Replace the `upstream`/`downstream` split on `NodeState` with a single **link store** — `links: BTreeMap<(PeerId, TopicId, LinkRole, LinkDirection), LinkState>` — where `LinkRole ∈ {Relay, Publisher}`, `LinkDirection ∈ {Out, In}` (who dialed), and `LinkState ∈ {AwaitingAccept, Active}` (the former `UpstreamState` lifecycle; `In` links are recorded `Active` at acceptance). The send/receive orientation of a link is a function of **role × direction** (research R2), generalising ROADMAP §1.2's "connection-direction inversion": for `Relay` the dialer receives (Out = message source, In = fan-out destination); for `Publisher` the dialer **sends** (Out = injection target for `Origin::Local` messages only, In = a source of that peer's own published messages).

On top of the re-expressed relay behaviour (behaviour-preserving — US1), the feature adds **publishing links** (the M3 S-link): a fourth strategy seam (`PublishStrategy`) selects publish targets on the same `Heartbeat` dial tick, gated on the M3 trigger (**no relay downstream on the topic** — evaluated deterministically via the verifiable edge predicate, research R6), with its own `publish_degree` and a distinct hash domain. The acceptance path becomes **role-aware**: the connection-control wire messages carry the link role, and inbound publish-intent requests dispatch to a separate publishing-acceptance strategy slot (the same four baseline kinds, instantiated with publish parameters). Fan-out becomes **origin-aware**: `ForwardToAll` forwards over `In`/`Relay` links for every message and additionally over `Active` `Out`/`Publisher` links for `Origin::Local` messages only. The receive gate admits payload from an `Active` `Out`/`Relay` link (as today) or an `In`/`Publisher` link — the latter **bound to the publisher** (only messages published by the link peer, research R5), enforcing "publishing links do not relay" on both ends.

`target_degree` is renamed **`relay_degree`** throughout (config, CLI, seam params — FR-009a), making the degree pair symmetric (`relay_degree` / `publish_degree`). Defaults preserve behaviour exactly: `--publish-strategy none` (no publishing links), `--publish-acceptance-strategy accept-from-all`, relay predicate domain unchanged (FR-012/SC-001).

## Technical Context

**Language/Version**: Rust (workspace toolchain; rust-version 1.75).

**Primary Dependencies**: unchanged — `sha2` for the edge predicate (the publish predicate reuses `strategies::edge` with a distinct domain-separation tag `pubsub/bucketed-pull/publish-edge/v1`; the relay tag stays byte-identical), tokio + tracing, clap at the edge. No new crates.

**Storage**: N/A (in-memory node state).

**Testing**: `cargo test`; **TDD** (link-model migration + origin-restricted forwarding are protocol-behaviour claims → critical per Constitution II). Declarative test construction via `ConnectionScript`, extended with role-carrying steps (`publish_request_from`, `publish_accepted_from`).

**Target Platform**: library crate + CLI binary (`src/main.rs`).

**Project Type**: single Rust project.

**Performance Goals**: selection O(candidates) per topic per seam (one hash per candidate per role); trigger evaluation O(candidates) per topic (one predicate recomputation per candidate). No hot path.

**Constraints**: the state transition stays pure/deterministic — the publish trigger is computed from the `NodeView` (expected relay downstream via the public predicate), not from observed timing; the link store is a `BTreeMap` so iteration order (shutdown notices, snapshots) is deterministic; reproducible from genesis.

**Scale/Scope**: per-node strategy + state-model logic; multi-node metrics stay in the experiment-framework feature.

## Constitution Check

- **I. Correctness Over Optimization** — ✅ Every behaviour traces to an FR in `spec.md`, a Clarifications bullet (sessions 2026-07-10/13), the M3 publishing-link description (logbook 2026-07-09), or `relay-tier-extension-proposal.md` §2.2 (adapted per the spec's substrate note). The role × direction orientation rule is recorded as ADR 0032.
- **II. Test-Driven for Correctness Claims** — ✅ **Critical.** The behaviour-preserving migration (FR-012/SC-001), origin-restricted forwarding (FR-005/FR-006/SC-002), publisher binding on `In`/`Publisher` links (R5), the M3 trigger (FR-009b/SC-003), degree independence (FR-009/SC-004), and dedup across roles (FR-011/SC-005) get tests before implementation.
- **III. Document Structural Decisions as ADRs** — ✅ ADR 0032 (unified link store + role on the wire), ADR 0033 (publishing-link seams: trigger, degree/domain separation, role-aware acceptance) planned below.
- **IV. Specifications as Ambiguity Detectors** — ✅ Two surfaced items: (a) the spec's send-side FR-005 has a receive-side dual the spec does not state (should an `In`/`Publisher` link admit relayed traffic?) — resolved as publisher-binding, recorded in ADR 0033 rather than silently coded; (b) the M3 trigger's "no downstream" is ambiguous between *observed* and *expected* downstream — resolved as expected (deterministic), residual gap documented (R6).
- **V. Specifications Are Read-Only** — ✅ Only `pubsub-node/` code-side artifacts; the extension proposals and logbook are cited, not edited.

**Engineering Standards** — ✅ reproducible (trigger + selection pure functions of the view); ✅ no wall-clock; ✅ correctness asserted via snapshots/getters (a `links` snapshot getter; the existing `upstream_connections`/`downstream_connections` getters preserved as relay-scoped views), never log strings; ✅ parse at the edge (`--publish-degree`, `--publish-strategy`, `--publish-acceptance-strategy` parsed in CLI, typed params inward); ✅ forward-compatible shapes justified by named consumers (`LinkDirection` reserved headroom for 016 — the spec's companion note; role tag on the wire consumed by this feature); ✅ declarative test construction (`ConnectionScript` role steps).

### ADRs

- **ADR 0032 — Unified link store and the role-carrying handshake.** `links: BTreeMap<(PeerId, TopicId, LinkRole, LinkDirection), LinkState>` replacing `upstream`/`downstream`; `UpstreamState` renamed `LinkState` (same two variants, same no-stored-terminal rule); the **role × direction orientation rule**; `direction: Both` NOT stored — 016's bidirectionality emerges as the Out+In pair under a symmetric predicate, so the stored direction stays binary; `ConnectionAction` variants gain a `role` field with a role tag byte appended to `signed_bytes` (pre-release layout change, documented at the encoder).
- **ADR 0033 — Publishing-link seams.** The `PublishStrategy` selection seam (v1 kinds: `none`, `hash-gated`) with the deterministic M3 trigger (expected-relay-downstream-empty via the public predicate — dial-time, not observed); `publish_degree` + the distinct `publish-edge/v1` hash domain (independent draw from relay edges); the role-aware acceptance dispatch (second acceptance slot instantiated from the same four baseline kinds with publish params); publish admissions counted against a publish cap (`⌈publish_degree + c·√publish_degree⌉`), never the relay `OC`; the publisher-binding receive gate on `In`/`Publisher` links (drop cause `relay_over_publish_link`).

## Project Structure

### Documentation (this feature)

```text
specs/015-connection-link-model/
├── plan.md              # this file
├── research.md          # Phase 0 — R1–R10
├── data-model.md        # Phase 1 — link store, roles, transitions
├── quickstart.md        # Phase 1 — configuring a publisher node
├── contracts/
│   └── link-model-and-seams.md   # seam signatures + wire changes
└── tasks.md             # Phase 2 (/speckit-tasks — not created here)
```

### Source Code (repository root)

```text
pubsub-node/
├── src/
│   ├── connection_state.rs        # UpstreamState → LinkState; LinkRole, LinkDirection; ConnectionScript role steps
│   ├── state.rs                   # links store; role-aware handlers; origin-aware record/fanout; publish dial pass
│   ├── message.rs                 # ConnectionAction { …, role }; signed_bytes role tag
│   ├── node.rs                    # links getter; relay-scoped legacy getters; publish seam injection
│   ├── main.rs                    # --relay-degree rename; --publish-strategy/--publish-degree/--publish-acceptance-strategy
│   └── strategies/
│       ├── edge.rs                # publish-edge domain variant; relay bytes unchanged
│       ├── view.rs                # NodeView over the link store (role-scoped accessors)
│       ├── config.rs              # PublishParams; relay_degree renames; publish seam in two-phase build
│       ├── connection/…           # relay_degree rename only
│       ├── acceptance/…           # role-parameterised hash-gated baselines (domain + degree per instance)
│       ├── publish/               # NEW seam: mod.rs (PublishStrategy), none.rs, hash_gated.rs, kind.rs
│       └── fanout/…               # origin-aware targets(); ForwardToAll over In/Relay ∪ Out/Publisher(Local)
└── tests/                         # migration parity, publish-link integration, trigger, dedup-across-roles
```

**Structure Decision**: the fourth seam lands as `strategies/publish/` beside the existing three (ADR 0029 layout); connection lifecycle vocabulary stays in `connection_state.rs` as core domain state.

## Complexity Tracking

No constitution violations. The publisher-binding receive gate (R5) and the expected-vs-observed trigger choice (R6) are surfaced design additions, recorded in ADR 0033 per Principle IV rather than silently resolved.

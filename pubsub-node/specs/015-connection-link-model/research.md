# Research — 015 unified link model & publishing links

Decisions made during planning, with rationale and rejected alternatives. FRs refer to [spec.md](./spec.md).

## R1 — Link store shape

**Decision**: `links: BTreeMap<(PeerId, TopicId, LinkRole, LinkDirection), LinkState>` with `LinkRole { Relay, Publisher }`, `LinkDirection { Out, In }`, `LinkState { AwaitingAccept, Active }` (the former `UpstreamState`, renamed). `In` links are inserted `Active` at acceptance (the acceptor has nothing to await). Terminal outcomes stay removals, not stored states.

**Rationale**: the clarified data model keys links by `(peer, topic, role)` with coexisting roles; direction must additionally be part of the key because today's model already allows the same `(peer, topic)` in both `upstream` and `downstream` (dial + accept between the same pair). A `BTreeMap` (over `HashMap`+`HashSet`) makes shutdown-notice emission and snapshots deterministically ordered — the 005 ordered-structures direction.

**Rejected**: (a) one entry per `(peer, topic)` carrying a role *set* — murky lifecycle (one role `Active` while the other `AwaitingAccept`), per-role cap counting needs unpacking (spec Clarifications 2026-07-13); (b) storing `Direction::Both` — see R10.

## R2 — Orientation is a function of role × direction

**Decision**: what a link *carries* is derived, not stored:

| Role | Out (I dialed) | In (peer dialed) |
|---|---|---|
| `Relay` | message **source** (my upstream; gate: `Active`) | fan-out **destination** (my downstream) |
| `Publisher` | injection **target** — my `Origin::Local` messages only (gate: `Active`) | **source of that peer's own published messages** only |

**Rationale**: generalises ROADMAP §1.2 (for relay links the dialer is the receiver) with the publish inversion the M3 S-link needs (the publisher dials to *push*). One rule, four cells, no per-link mode flags.

## R3 — Role on the wire

**Decision**: every `ConnectionAction` variant (`Request`/`Accepted`/`Terminated`/`Rejected`) gains a `role: LinkRole` field; `PlainConnection::signed_bytes` appends a role tag byte (`0x00` Relay, `0x01` Publisher) after the topic. The signature therefore binds emitter, action kind, topic, **and role**.

**Rationale**: the link is keyed by role on both ends, so every handshake and teardown message must identify which link it concerns; an unauthenticated role would let a peer confuse the two caps. Pre-release layout change — permissible, documented at the encoder per its own contract note; no version tag exists yet by design.

**Rejected**: new action variants per role (8 variants, duplicated arms) — the role is orthogonal to the action kind.

## R4 — Origin-aware fan-out seam

**Decision**: `FanoutStrategy::targets(topic, links, origin, exclude) -> Vec<PeerId>` — the seam receives the message `Origin`. `ForwardToAll` returns every `In`/`Relay` peer on the topic (minus split-horizon `exclude`) for **any** origin, plus every `Active` `Out`/`Publisher` peer when `origin == Local` (FR-005/FR-006).

**Rationale**: the spec fixes origin-awareness in the fan-out seam (Clarifications 2026-07-10); passing `Origin` rather than a bool keeps the signature stable if a future strategy differentiates by publishing peer.

## R5 — Publisher binding on the receive gate (surfaced design addition)

**Decision**: `handle_dissemination` admits a payload iff the deliverer holds an `Active` `Out`/`Relay` link (as today) **or** an `In`/`Publisher` link **and the message's `publisher_id` is the link peer**. A message over a publish link from a different publisher is dropped with the distinct cause `relay_over_publish_link`.

**Rationale**: FR-005 restricts what the sender pushes over a publish link; without the receive-side dual, a misbehaving peer could use its accepted publish link as a general relay path, bypassing the relay-degree topology. This is the receive-side enforcement of "publishing links carry only the publisher's own messages". Surfaced per Principle IV (the spec states only the send side); recorded in ADR 0033.

## R6 — Deterministic M3 trigger: expected downstream, not observed

**Decision**: the publish selection strategy evaluates "no relay downstream on the topic" (FR-009b) as **expected** relay downstream under the current epoch nonce: for each candidate `D` on topic `T`, recompute the public relay predicate in the inbound direction (`is_valid_edge(nonce, T, D, self, B)`); if no candidate would select this node, the trigger fires and publish targets are selected.

**Rationale**: pure and computable at dial time from the `NodeView` — no dependence on delivery timing of inbound requests within the same heartbeat, so runs stay reproducible (spec plan-item, Clarifications 2026-07-13). Under the default seams (`connect-to-all`, `B = 1`) every candidate is expected downstream, so the trigger never fires — behaviour-preservation falls out by construction.

**Residual (documented)**: observed downstream can be smaller than expected (over-capacity rejections, un-synced peers), leaving a node with zero *actual* downstream and no publishing links until a future heartbeat under a changed epoch. Accepted for v1 — the same class of under-fill 005 accepted for relay degree (no retry/back-fill).

## R7 — Publishing acceptance: same kinds, publish parameters, second slot

**Decision**: `NodeState` gains a `publish_acceptance_strategy` slot; the connection-request handler dispatches on the request's carried role. The publishing slot is instantiated from the **same** `AcceptanceStrategyKind` family, constructed with publish parameters: `publish_degree` (for the cap `⌈publish_degree + c·√publish_degree⌉`) and the publish edge domain (for the gate). The hash-gated acceptance structs gain a role/domain parameter at construction; the relay instances keep the relay domain. CLI default: `accept-from-all` (mirrors the relay seam default; behaviour-preserving).

**Rationale**: the clarified answer — policy lives in interchangeable strategies, mechanics in the role dispatch; reusing the four baselines avoids a parallel strategy family and keeps mixed experiment configs expressible. Publish admissions never count against the relay `OC` (FR-008a) — automatic, since the cap scan is role-scoped (R1 key).

## R8 — `relay_degree` rename

**Decision**: `target_degree` → `relay_degree` across config params, CLI (`--target-degree` → `--relay-degree`), strategy fields, and `edge.rs` helper signatures/docs (FR-009a). Hard rename, no deprecated alias — pre-release, single-process mock deployments only.

**Rationale**: symmetric naming against `publish_degree`; an alias would be dead weight with no released operators to migrate.

## R9 — Dedup across roles

**Decision**: no new mechanism. The content-hash `seen` set at the shared record point already suppresses the second copy when a subscriber is reachable over both a publish link and the relay flood (FR-011/SC-005); a test pins it.

## R10 — `Both` is emergent, not stored

**Decision**: the stored direction stays binary (`Out`/`In`). Feature 016's bidirectional link materialises as the **pair** of entries (Out + In) that a symmetric predicate produces when both ends dial each other; no `Both` variant is stored in 015.

**Rationale**: a stored `Both` would need merge/split transitions (what happens when one side tears down?); the pair representation reuses the existing lifecycle unchanged and keeps 015's store shape final for 016. The spec's "`Both` reserved" is honoured at the *concept* level (the public `LinkDirection` enum is `#[non_exhaustive]` so 016 can add a variant if its design ends up needing one, without a breaking change).

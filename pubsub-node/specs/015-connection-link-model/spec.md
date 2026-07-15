# Feature Specification: Unified connection link model (role + direction) & publishing links

**Feature Branch**: `015-connection-link-model`

**Created**: 2026-07-10

**Status**: Draft

**Input**: User description: "Introduce a unified connection Link abstraction (role + direction) replacing the upstream/downstream split, and add seed/publishing links used only for locally-originated messages."

> **Companion feature.** This is the first of two features that grow the connection model. **015 (this feature)** introduces the unified `Link` abstraction and the **publishing** link role. **016 — bidirectional links** then adds the `direction: Both` case via a symmetric edge predicate, building on the abstraction introduced here. The `direction` dimension is defined now but only `Out`/`In` are exercised in 015; `Both` is the forward-compatible seam justified by the 016 consumer (Constitution — forward-compatible interfaces justified by a ROADMAP consumer).

## Context

Today a node's connections are a **directed split** keyed by who dialed (ROADMAP §1.2, "connection-direction inversion"):

- **`upstream`** — `(peer, topic)` entries the node *dialed* (`UpstreamState::AwaitingAccept` → `Active`), owned in [`connection_state`]. These are the node's **message sources**: a peer it pulls from.
- **`downstream`** — a flat `HashSet<(PeerId, TopicId)>` of peers that *dialed the node* and were accepted. These are the node's **fan-out destinations**.

Two assumptions are baked into that split, and this feature relaxes the first while generalising the second:

1. **Every downstream is a uniform relay.** `ForwardToAll` forwards over **every** downstream on the topic irrespective of whether the node *published* the message (`Origin::Local`) or *relayed* one it received (`Origin::Peer`). The fan-out seam is not even told the origin.
2. **A link's role is implied by dial direction** (upstream XOR downstream), so there is nowhere to record that a link exists for a *different purpose* than the uniform relay mesh.

**The M3 publishing link.** The 2026-07-09 design session (see `../../logbook.md`) settled that a node acting as the **root/publisher** of a message needs a distinct link class — a **publishing link** (the M3 model's *S-link*) — used *only* to inject its own published messages into the overlay, **not** to relay messages received from others. The session also fixed the terminology this spec uses: **publishing links** vs **relaying links**. Decoupling injection from relay keeps the relay Request Factor (`target_degree` — renamed `relay_degree` in this feature) from being inflated to also cover publisher reach, and gives a pure publisher (one that holds no upstream) a way to seed dissemination. The protocol antecedent is the **publisher→relay direct link** of the read-only [`../../docs/extensions/relay-tier-extension-proposal.md`](../../docs/extensions/relay-tier-extension-proposal.md) §2.2, adapted to today's hash-gated overlay.

> **Substrate note.** The two extension proposals under `../../docs/extensions/` were written (April 2026) against the AUEB three-layer stack (SecureCyclon peer sampling + Vicinity/Harary dissemination). That substrate has since been **retired** (logbook 2026-07-07); this feature targets the current **hash-gated (bucketed-pull)** overlay of feature 005. The *link-class concepts* (owner-attested push relays) carry over; the Vicinity/SecureCyclon mechanics they cite do not. The companion **local-relays** proposal (trust-based reciprocal handshake) is treated as a **superseded artifact** and is out of scope (its bidirectional case is instead delivered structurally in 016 via a symmetric hash predicate — see Clarifications).

**This feature.** Replace the `upstream`/`downstream` split with a single **`Link`** vocabulary carrying an explicit **role** (`Relay` | `Publisher`) and **direction** (`Out` | `In` | `Both`), and make fan-out **origin-aware** so publishing links fire only for locally-originated messages. The existing hash-gated relay behaviour is re-expressed over the new model with **no observable change** to dissemination; the publishing role is the new capability layered on top.

**Scope**: the unified `Link` model (replacing `UpstreamState` + the flat `downstream` set and the seam view over them), the `Publisher` link role, origin-aware fan-out, and their tests. **Out of scope**: `direction: Both` / bidirectional links (feature 016), the symmetric edge predicate (016), periodic heartbeats / link rotation / teardown (unchanged from 005 — v1 fires one heartbeat), the incentive/chain layer (deposits, sybil bounds, slashing), the real beacon, discovery/view sampling, and the experiment framework.

## Clarifications

### Session 2026-07-10 (feature framing)

- Q: One feature or two? → A: **Two.** 015 (this) delivers the unified `Link` model + publishing role; **016** delivers bidirectional links. They share the `Link` abstraction, introduced here.
- Q: Which notion of "bidirectional" (deferred to 016)? → A: **Symmetric hash-gate only.** Bidirectionality emerges structurally from a symmetric edge predicate (A dials B ∧ B dials A by construction) — no new control flow. The **trust-based reciprocal-handshake** model of `local-relays-extension-proposal.md` is a **superseded SecureCyclon-era artifact** and will not be built.
- Q: Architecture — extend the split or a new abstraction? → A: **New unified `Link` abstraction.** Replace `upstream`/`downstream` with one `Link { peer, topic, direction, role, state }` store the seams operate over, rather than tagging attributes onto the two existing sets.
- Q: How does a publishing link differ at fan-out time? → A: **Origin-aware fan-out.** A `Relay` link fires for every message on its topic (as today); a `Publisher` link fires **only** when the message origin is `Local` (the node published it), never when relaying an `Origin::Peer` message.

### Session 2026-07-13 (clarify pass)

- Q: How is a publishing link established (push-intent dial vs role on an accepted downstream)? → A: **The same way relay links are established** — via a strategy pair for requesting and acceptance in the unidirectional link setting. The publisher runs a link-selection strategy (e.g. a hash-gated / hash-gated-bounded variant, exactly as nodes establish upstream relay peers today) to choose its publishing targets and dials them; the target's acceptance strategy decides admission. Also fixed the canonical term: **publishing links** (role `Publisher`), formerly "seed"/"S-links".
- Q: Publishing degree & predicate separation? → A: **Own degree + distinct hash domain.** A new **`publish_degree`** parameter, independent of the relay degree, and a distinct domain separator in the edge predicate so publish edges are an independent hash draw from relay edges (not the same edge set re-tagged). Both degrees are independently sweepable. Also fixed naming: the existing `target_degree` is **renamed `relay_degree`** so the two parameters are symmetric (`relay_degree` / `publish_degree`).
- Q: When does a publisher form publishing links — always, or conditionally? → A: **Conditionally, on missing relay downstream** (a refinement of the M3 trigger): a node forms publishing links on a topic when it holds **no relay downstream** there — i.e. it was not selected as any peer's upstream, so its published messages have no relay path into the overlay. Publishing dials fire on the **same event as relay upstream dials** — the `Heartbeat` dial tick — not on a new event or on publish. *(Plan item: how "no downstream" is evaluated at dial time — the verifiable edge predicate makes the expected downstream set locally computable, so the trigger can be deterministic rather than waiting on observed acceptance.)*
- Q: Inbound acceptance of a publish-intent request — distinct decision, and what cap? → A: **Policed by the acceptance-strategy seam, kept modular and interchangeable per experiment.** The seam becomes role-aware (the request carries the intended role); the accept/cap policy is strategy business, not a protocol constant. v1 ships a **default publishing-acceptance baseline mirroring the relay one** (hash-gated-bounded analog: publish-domain predicate + a cap derived from `publish_degree`), with publishing admissions **not counting against the relay `OC`** so the two resources stay independently bounded and sweepable.
- Q: Dual-role between the same `(peer, topic)` — one link with a role set, or separate links? → A: **Coexisting links per role.** The link store is keyed by `(peer, topic, role)`: a `Relay` link and a `Publisher` link between the same pair are independent entries with independent lifecycles, caps, and teardown. (The combination is legal: a node may pull from a peer as relay upstream *and* push to it over a publishing link on the same topic.)

### Session 2026-07-13 (model-family alignment — supersedes the trigger)

Denis's executable dissemination models landed on `main` (`formal_spec/hybrid_dissemination/models/`, M1–M5) and are now the authoritative protocol source for this feature; the workstream goal is M3 + M4 + M5 support as configurations of one node (ADR 0034).

- Q: Trigger — reconfirmed? → A: **No — removed.** Publishing links (the model's **standing initiation links**, `publish_degree` ≈ its `s−1`) are established **always**, unconditionally: `m3/README.md` — "each node opens s−1 standing initiation links". Supersedes this session's earlier conditional answer and FR-009b's trigger clause.
- Q: Does a local publish also go over the relay downstream, or exclusively over publishing links? → A: **Both, as selectable fan-out kinds** — the fan-out seam is the dissemination-model knob. `forward-to-all` (default; behaviour-preserving): relay downstream for every message plus the initiation targets for a local origin — the reading under which a publisher, as a forwarder, "relays every message it holds" (m3/README.md). `role-scoped` (the strict M3 partition): local publications over initiation links **only**, relayed traffic over relay links only ("initiation links … are never part of the relay graph"). Experiments cross-validate both against the model's coverage laws.
- Q: Reusable strategies instead of per-role families? → A: **Yes.** One `LinkSelectionStrategy` family (`none` | `connect-to-all` | `hash-gated`) serves both role slots, and one acceptance family serves both acceptance slots — the role is a construction parameter, not a type. The `NodeView` exposes the link store **cell-structured** (`relay_out`/`relay_in`/`publish_out`/`publish_in`) so each fan-out strategy selects — or unions — exactly the fields its model prescribes.
- Q: (Refinement) Which kind is M3? → A: **`forward-to-all` is the M3 semantics** — re-reading `m3/README.md`: relay links carry both relayed traffic and the node's own publications ("a forwarder relays every message it holds to its requesters"), while initiation links are owner-exclusive and never forward relayed traffic. `role-scoped` is a strict-partition **experimental variant prescribed by no published model**, retained as an experiment lever (isolates the initiation links' marginal coverage contribution). Supersedes this session's earlier "both readings" framing.
- Q: (Review) Does the link abstraction need four role×direction cells — can it be simpler with less abstraction, and less test churn on refactors? → A: **Flow-oriented store, stable views** (ADR 0036). The information content is irreducible (M3's owner-binding and the disjoint caps need the publisher/relay distinction on both sides), but the *shape* flattens: internally **two maps** — `sources` (peers I receive from: my pull links ∪ accepted initiation links) and `sinks` (peers I send to: accepted relay downstream ∪ my initiation targets) — each entry carrying two facets (pull/push with lifecycle, or relay/push). The fan-out seam reads **only sinks**; the receive gate reads **only sources**; a peer in both maps is M4's bidirectionality. The **role × direction vocabulary stays** as the mutation API, the public view (`links()`, getters), and the wire tag — so tests bind to stable surfaces and the store can be reshaped again without rewriting them (the test-stability rule this session adds).
- Q: M4 and M5? → A: ~~Roadmap~~ **In-feature** (revised in-review — "fill those gaps, not document them"; ADR 0035): **M4** = the symmetric edge predicate (`--symmetric-edges`, one flag wiring relay selection AND acceptance so the seams cannot disagree; each edge materialises as the Out+In pair on both sides — the R10 emergence, no wire change, and no new fan-out kind: under pair emergence `forward-to-all` already floods all incident links). **M5** = the `role-agnostic` fan-out kind (no link-role distinction: every held message over relay-in ∪ publish-out, any origin, minus arrival) + the `--publish-in-admission` receive-gate policy (`owner-only` default = M3's exclusivity | `any-verified` = M5's k_out semantics); the two must be paired network-wide. Feature 016 remains only for whatever bidirectional work M4 does not already cover.


## User Scenarios & Testing *(mandatory)*

### User Story 1 - Behaviour-preserving migration to the unified Link model (Priority: P1)

The existing hash-gated relay topology and dissemination are re-expressed over the unified `Link` vocabulary with **no observable change**. Every current upstream becomes an `Out`/`Relay` link (with the same `AwaitingAccept`→`Active` lifecycle); every current accepted downstream becomes an `In`/`Relay` link; fan-out over relay links is identical to today's `ForwardToAll`.

**Why this priority**: The abstraction is the foundation for both the publishing role (this feature) and bidirectional links (016). It must land as a pure, behaviour-preserving refactor before any new capability rides on it, so a regression is attributable to the refactor alone.

**Independent Test**: The full existing suite (005/006/014 connection, acceptance, and fan-out tests) passes unchanged against the new model; a relay-only configuration produces byte-identical delivery snapshots to `main` for the same genesis + membership + interval.

**Acceptance Scenarios**:

1. **Given** a node that dialed a hash-gated upstream, **When** the peer's `Accepted` arrives, **Then** the node holds an `Out`/`Relay` link in the `Active` state for that `(peer, topic)` — the former `UpstreamState::Active`.
2. **Given** a node that accepted an inbound relay `Request`, **When** acceptance succeeds, **Then** the node holds an `In`/`Relay` link for that `(peer, topic)` — the former `downstream` entry.
3. **Given** a relay-only node and a published or received message, **When** fan-out runs, **Then** the target set equals today's `ForwardToAll` over the topic's `In`/`Relay` links, minus the split-horizon exclusion.

---

### User Story 2 - Publishing links carry only locally-originated messages (Priority: P1)

A node holding both a `Publisher` link and a `Relay` link to peers on a topic forwards a **message it published** over both, but forwards a **message it relayed** (received from another peer) over the `Relay` link only. The publishing link never participates in the relay flood.

**Why this priority**: This is the feature's headline capability — the origin-restricted publishing link the M3 model requires to decouple publisher injection from relay.

**Independent Test**: Configure a node with one `Publisher` and one `Relay` downstream-direction link on topic `T`. `Node::publish` a message → both targets receive it. Deliver an `Origin::Peer` message on `T` → only the `Relay` target receives a forward; the `Publisher` target does not.

**Acceptance Scenarios**:

1. **Given** a `Publisher` link and a `Relay` link on topic `T`, **When** the node publishes a message on `T` (`Origin::Local`), **Then** both links are fan-out targets.
2. **Given** the same links, **When** the node relays a message received on `T` (`Origin::Peer`), **Then** only the `Relay` link is a target and the `Publisher` link is excluded.
3. **Given** split-horizon on the relay path, **When** a relayed message would echo to its delivering peer, **Then** that peer is still excluded regardless of link role.

---

### User Story 3 - Publishing-link establishment for a publisher (Priority: P2)

A publishing node forms `Publisher` links (standing initiation links) **through the same strategy machinery relay links use**: on the `Heartbeat` dial tick — the same event that fires relay upstream dials — the publish link-selection slot chooses the targets (the hash-gated policy with its own `publish_degree` and hash domain) and the node dials them; each target's acceptance strategy decides admission. The dial is **unconditional** — initiation links exist regardless of the node's relay links (`m3/README.md`; supersedes the trigger).

**Why this priority**: Establishment is required for publishing links to exist in a running node, but the origin-restricted fan-out (US2) is independently testable against publishing links created by test builders, so establishment is P2 behind the model + fan-out behaviour.

**Independent Test**: A node on `T` with more candidates than `publish_degree` forms exactly the strategy-selected `Publisher` links via dial→accept on the heartbeat, whether or not it holds relay links.

**Acceptance Scenarios**:

1. **Given** a publisher on `T`, **When** the `Heartbeat` dial tick runs, **Then** the node dials the strategy-selected initiation targets and, on acceptance, holds `Out`/`Publisher` links — selected by `publish_degree`, independent of `relay_degree` and of any relay-side state.
2. **Given** a target whose acceptance strategy admits the publish-intent request, **When** the dial arrives, **Then** the target records an `In`/`Publisher` link and replies `Accepted` (same handshake shape as relay).
3. **Given** a publisher holding no relay links at all on `T`, **When** it publishes, **Then** its published message still reaches the overlay via the initiation links.

### Edge Cases

- A `(peer, topic)` that is **both** a relay link and a publishing link → two coexisting entries keyed by role, independent lifecycles (per Clarifications 2026-07-13). Tests cover the pair holding both roles simultaneously.
- Publishing link to a peer that is also a relay **downstream**: the published message must not be delivered twice (content-hash dedup at the receiver already covers this — confirm it holds across roles).
- A publishing link whose target never sends `Accepted` (half-open) — same lifecycle/no-retry semantics as a relay `Out` link (ADR 0031: over-capacity `Rejected` drops the pending link; no back-fill).
- Fan-out on a topic with only `Publisher` links and an `Origin::Peer` message → no targets (correct: publishing links do not relay).
- The unified model must not resurrect the fail-open pre-`Synced` admission race (ADR 0031) — inbound link establishment stays gated on `Synced`.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The node MUST represent every connection as a `Link` carrying a `role` (`Relay` | `Publisher`), a `direction` (`Out` | `In`; `Both` reserved for 016), and, for `Out` links, the establishment lifecycle state (`AwaitingAccept` | `Active`).
- **FR-002**: The unified `Link` store MUST replace the separate `upstream` (`UpstreamState`) and `downstream` (`HashSet<(PeerId, TopicId)>`) representations; the strategy `NodeView` MUST expose the link set such that the connection, acceptance, and fan-out seams read what they read today (dial side, inbound count, fan-out targets) without behavioural change for `Relay` links.
- **FR-003**: A dialed hash-gated upstream MUST become an `Out`/`Relay` link created `AwaitingAccept` and advanced to `Active` on the peer's `Accepted` — preserving the current lifecycle and the no-stored-terminal-state rule (removals, not a closed variant).
- **FR-004**: An accepted inbound relay `Request` MUST become an `In`/`Relay` link — the former downstream entry — under the unchanged acceptance policy (membership, hash-gate, cap, readiness gate).
- **FR-005**: Fan-out MUST be **origin-aware**: the seam MUST receive whether the message is `Origin::Local` or `Origin::Peer(...)`. Under the M3 kinds (`forward-to-all`, `role-scoped`) a `Publisher`-role link never carries an `Origin::Peer` message (publishing links do not relay); the M5 kind (`role-agnostic`) deliberately floods both cells for any origin, paired with the `any-verified` gate (FR-013/FR-015). Under the default `forward-to-all`, `Publisher`-role links are included **only** for `Origin::Local` messages while `Relay`-role links are included for both origins.
- **FR-006**: Under the default `forward-to-all` kind, fan-out over `Relay` links MUST remain identical to the pre-015 `ForwardToAll` (every `Relay` link on the topic, minus the split-horizon exclusion), so relay-only dissemination under default configuration is unchanged (SC-001).
- **FR-007**: Split-horizon exclusion MUST apply on the relay path regardless of link role (a node never echoes a relayed message to its delivering peer).
- **FR-008**: Publishing-link establishment MUST use the same strategy-seam machinery as relay links — a link-selection strategy choosing targets on the dial side and an acceptance strategy deciding admission on the inbound side, over the existing dial→accept handshake — with hash-gated variants as the baseline policies.
- **FR-008a**: The acceptance seam MUST be **role-aware**: an inbound request carries its intended link role, and the admission policy per role is an interchangeable strategy. The v1 default publishing-acceptance baseline mirrors the relay one (publish-domain predicate + a cap derived from `publish_degree`); accepted publishing links MUST NOT count against the relay downstream cap `OC`.
- **FR-009**: Publishing-target selection MUST be parameterised by its own **`publish_degree`**, independent of the relay degree, and the publish edge predicate MUST use a distinct domain separator so the publish edge set is an independent hash draw from the relay edge set. A node MUST be able to hold `Publisher` links even when it holds no relay links at all on the topic.
- **FR-009a**: The existing `target_degree` parameter MUST be renamed **`relay_degree`** (config, CLI, and seam parameters), making the degree pair symmetric (`relay_degree` / `publish_degree`); the rename ships with the behaviour-preserving migration.
- **FR-009b**: Publishing dials MUST fire on the same `Heartbeat` dial tick that fires relay upstream dials (no new event, no dial-on-publish), **unconditionally** — the node's standing initiation links exist regardless of its relay-side links (`m3/README.md`; the earlier trigger clause is superseded, Clarifications session "model-family alignment").
- **FR-013**: The fan-out seam MUST be a selectable strategy kind — `forward-to-all` (default; the M3 semantics), `role-scoped` (strict-partition experiment variant), and `role-agnostic` (the M5 semantics — no link-role distinction: every held message over relay-in ∪ publish-out, any origin, minus the arrival link) — reading the cell-structured link store, so dissemination models are configurations rather than code changes (ADR 0034/0035).
- **FR-014**: The hash-gated relay selection and acceptance kinds MUST support a **symmetric** edge-predicate mode (unordered-pair hashing under distinct domain tags), enabled by one shared flag so the two seams cannot disagree; under it every edge materialises as the Out+In pair on both sides (the M4 bidirectional mode, ADR 0035). The publish seams stay directional.
- **FR-015**: The receive gate's inbound-initiation admission MUST be a per-node policy — `owner-only` (default; the M3 exclusivity of FR-005's cross-kind invariant... which under `any-verified` is deliberately waived) or `any-verified` (M5: standing links carry every held message). Severance of an invalidly-signed payload's admitting link applies under either policy.
- **FR-010**: Publishing-link establishment MUST NOT reintroduce the pre-`Synced` fail-open admission race; inbound link creation stays gated on readiness (ADR 0031).
- **FR-011**: Existing content-hash dedup MUST continue to prevent a subscriber that is reachable over both a publishing and a relay link from processing the same published message twice.
- **FR-012**: The migration MUST be behaviour-preserving for existing configurations: with no `Publisher` links configured, observable dissemination, acceptance, and rejection behaviour MUST match the pre-feature node for identical inputs.

### Key Entities

- **Link**: one logical connection identified by `(peer, topic, role, direction)` — stored as role × direction cells keyed by `(peer, topic)` — carrying (for outbound) the lifecycle `state`. A `Relay` and a `Publisher` link between the same pair coexist as independent entries, as do the two directions of one role. Replaces `UpstreamState` and the flat downstream set.
- **LinkRole**: `Relay` (participates in the full dissemination flood — publish and relay) vs `Publisher` (a publishing link — carries only the node's own `Origin::Local` published messages into the overlay; the M3 S-link, formerly referred to as "seed link").
- **LinkDirection**: `Out` (node dialed), `In` (peer dialed), `Both` (reserved — the symmetric bidirectional case delivered in 016).
- **Origin** (existing): `Local` (this node published) vs `Peer(PeerId)` (relayed) — now an input to the fan-out decision, not only to `ReceivedDelivery`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With no publishing links configured, the delivery snapshot for a fixed genesis + membership + interval is identical to the pre-feature node across the existing test topologies (behaviour-preserving migration).
- **SC-002**: For a node with one publishing and one relay downstream-direction link on a topic, a published message reaches both peers and a relayed message reaches only the relay peer — in 100% of runs (deterministic).
- **SC-003**: A publisher successfully injects its published message into the overlay via its standing initiation links, established unconditionally — including when it holds zero relay links on the topic.
- **SC-004**: Publishing-link count is controllable independently of the relay degree: sweeping `relay_degree` does not change the publishing set, and sweeping `publish_degree` does not change the relay set.
- **SC-005**: No message is delivered to a subscriber twice when it is reachable via both a publishing and a relay link.

## Assumptions

- The controlled substrate answers every dial (no timeout/no-response path), as in 005 — establishment outcomes are `Accepted`/`Rejected`, not timeouts.
- v1 fires a single `Heartbeat` at readiness and never fires `Epoch`; periodic re-establishment, rotation, and teardown remain deferred (unchanged from 005).
- Origin is already tracked on the receive path (`ReceivedDelivery.origin`, ADR 0021); this feature threads it into the fan-out seam rather than introducing it.
- The `direction: Both` variant is defined but unreachable in 015; its only consumer is feature 016.
- Publisher identity/authorisation is unchanged (topic-registry `is_publisher_authorized` on the dissemination path); publishing links change *who a published message is pushed to*, not *who may publish*.
- The read-only protocol docs under `../../docs/` are authoritative for intent; where their SecureCyclon-era mechanics conflict with the current hash-gated overlay, the overlay wins and the divergence is noted (Constitution Principle V — surface ambiguity, do not edit the source docs).

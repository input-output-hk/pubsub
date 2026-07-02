# Feature Specification: Seeded bounded connection-selection and acceptance strategies

**Feature Branch**: `005-peer-view`

**Created**: 2026-06-29

**Status**: Draft

**Input**: User description: "Add seeded, bounded connection-selection and acceptance strategies to the pubsub node, replacing the full-mesh connect-to-all / accept-from-all so a node forms a bounded partial topology. A node selects at most a configured upstream degree of upstream peers per topic, chosen by deterministic seed-based key-hashing (randomness encapsulated inside the strategy object so the state-transition stays deterministic and reproducible from a seed), and accepts inbound requests up to a configured downstream degree, sending an explicit rejection when over capacity. A rejected dial is back-filled by re-invoking the existing ConnectionSetup event (no new round or timer event). Includes the tests for these strategies. Builds on a separate prerequisite determinism/purity refactor (strategies moved to apply arguments, ordered data structures replacing HashSet, deterministic scheduling, and a flag decoupling ConnectionSetup from Synced) owned by the co-developing architect. The experiment/testing framework that drives these strategies to measure delivery percentiles, propagation depth, and convergence is a SEPARATE feature added on top later — out of scope here."

## Context

The node today connects to **every** discovered candidate on every joined topic (the `ConnectToAllCandidates` selection policy) and accepts **every** membership-valid inbound request (`AcceptFromAllCandidates`). The result is a complete per-topic mesh: a published message reaches all subscribers in one hop, so dissemination behaves trivially and there is no partial topology to study.

This feature replaces those with **bounded** policies: a node selects at most a configured upstream degree of upstream peers per topic and accepts at most a configured downstream degree of inbound connections, forming a partial topology. Selection is **seed-reproducible** (the randomness is encapsulated in the strategy object and derived by key-hashing, so it is repeatable from a recorded seed) and **variable** (different seeds explore different topologies). When a dial is rejected for over-capacity, the dialer simply drops the pending upstream (it stops waiting for an acceptance); there is **no retry and no back-fill** in this feature, so the realized upstream degree may settle below target. Re-forming connections is left to the future heartbeat/reshuffle layer, and a retry-to-a-minimum (back-fill) policy is a separate future strategy family.

**Scope**: only the bounded selection/acceptance strategies and their tests. The experiment/testing framework that *drives* these strategies to measure delivery percentiles, propagation depth, and convergence is a **separate feature, added on top later** — out of scope here. The broader **determinism/purity refactor** (moving strategies to `apply` arguments, deterministic scheduling, and a flag decoupling `ConnectionSetup` from `Synced`) is a separate workstream owned by the co-developing architect; this feature does **not** hard-depend on it — it applies ordered data structures to the state it introduces/touches itself, keeps its strategy objects pure, and coordinates with that workstream to avoid conflicting edits.

## Clarifications

### Session 2026-06-29

- Q: For how long does a peer rejected by a dial stay excluded from re-selection? → A: ~~Sticky for the run — once rejected, a peer is never re-dialed for that topic this run; back-fill only moves to lower-ranked untried candidates.~~ **Superseded 2026-07-02** (see below): back-fill and the sticky failed-set were removed; a rejected dial now only drops the pending upstream, with no retry/back-fill.
- Q: What does "rejected" mean — a timeout, or an active rejection by the peer candidate? → A: An **active, explicit over-capacity rejection** sent by the peer candidate (an acceptee already at its downstream degree). There is **no timeout / no-response path** in this feature: the round/timer mechanism is deliberately excluded, and in the controlled, lossless, manually-stepped substrate every dial is answered with `Accepted` or the explicit rejection. (Timeout/no-response would only arise with loss or offline peers — a later feature.)
- Q: What is the default seed when none is supplied? → A: **0** (fixed), keeping behaviour deterministic.
- Q: Does this feature hard-depend on the separate determinism/purity refactor, or apply ordered structures itself? → A: It keeps its strategy objects pure and coordinates with the strategies-as-arguments relocation (the co-developing architect's workstream) to avoid conflicts, so it does **not** hard-depend on it. SC-004's uniformity tolerance is pinned to a chi-square gate at p < 0.001.

### Session 2026-07-02

- Q: Should the bounded connection strategy handle rejection by retrying/back-filling to reach the degree, or start without that? → A: **Start without.** Remove the back-fill machinery entirely — the sticky `failed_upstream` set, the `rejections_received` counter/getter, and the viable-candidate diff in setup. On an over-capacity `Rejected`, the dialer's **only** action is to remove the matching pending (`AwaitingAccept`) upstream. The realized upstream degree may settle below target; re-forming connections is deferred to the future heartbeat/reshuffle layer. Retry-to-a-minimum (back-fill) becomes a **separate future strategy family** (working name `BackfillingSeededBoundedConnection`). The acceptance-side bound and the explicit `Rejected` signal are unchanged.
- Q: Rename the connection strategy to carry its seam noun? → A: Yes — `SeededBoundedSelection` → **`SeededBoundedConnection`**, paralleling `BoundedAcceptance` (acceptance) and `SeededBoundedFanout` (fan-out, 015).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Reproducible bounded upstream selection (Priority: P1)

A node configured with an upstream degree bound and a seed selects at most that many upstream peers per topic from its candidate set, forming a partial topology rather than a full mesh. Re-running with the same seed and membership reproduces an identical selection.

**Why this priority**: This is the core capability — without a bounded, reproducible partial topology there is nothing for the later experiment framework to measure, and results would be either degenerate (full mesh) or irreproducible. Everything else builds on it.

**Independent Test**: Construct a node (or a small set of nodes) with candidate sets larger than the upstream degree bound, under seed s; capture the selected upstream set. Rebuild identically under s; the selection is identical, and no node selects more than the bound per topic.

**Acceptance Scenarios**:

1. **Given** a topic with more candidates than the upstream degree bound, **When** a node selects, **Then** it selects exactly the bound's worth of upstream peers on that topic.
2. **Given** a topic with candidates at or below the bound, **When** a node selects, **Then** it selects all of them (the bound is a ceiling, not a quota).
3. **Given** the same seed, node identity, topic, and candidate set, **When** selection runs in two separate runs (including on different machines), **Then** the selected sets are identical.
4. **Given** no seed supplied at startup, **When** selection runs, **Then** a fixed default seed is used and behaviour stays deterministic.

---

### User Story 2 - Bounded inbound acceptance with explicit rejection (Priority: P2)

A node accepts verified, membership-valid inbound requests only up to a configured downstream degree per topic. Beyond the bound it sends an explicit rejection (distinct from a termination/misbehaviour severance). A dialer whose request is rejected simply drops that pending upstream — it stops waiting for an acceptance. There is no retry or back-fill; the realized upstream degree may settle below target until the future heartbeat/reshuffle layer re-forms connections.

**Why this priority**: Bounding inbound degree gives a second topology lever and is the inbound mirror of the dial-side bound. P2 because the dial-side bound (US1) alone already yields a partial topology.

**Independent Test**: Drive a node more inbound requests than its downstream degree on a topic — exactly the bound's worth are accepted, the rest dropped with the over-capacity cause and an explicit rejection sent, with no severance. Separately, reject a dialer's request — the dialer removes the matching pending upstream and takes no further action (no retry/back-fill); its realized upstream degree may fall below the bound.

**Acceptance Scenarios**:

1. **Given** a node below its downstream degree on a topic, **When** a verified membership-valid request arrives, **Then** it is accepted.
2. **Given** a node at its downstream degree on a topic, **When** a further verified request arrives, **Then** it is dropped with the over-capacity cause, an explicit rejection is sent, and no downstream entry is added.
3. **Given** a dial rejected for over-capacity, **When** the rejection is processed, **Then** the dialer removes the matching pending upstream and takes no further action (no retry, no back-fill).
4. **Given** rejections leave fewer accepted upstreams than the degree, **When** the topology settles, **Then** the node stays at under-fill (realized upstream degree below the bound), observably and without error.

---

### User Story 3 - Seed-varied, identity-unbiased selection (Priority: P3)

Across a sweep of distinct seeds, selections differ from one another, and no candidate is systematically preferred or excluded — over many seeds each candidate is equally likely to be selected.

**Why this priority**: Reproducibility (US1) makes a single run repeatable; this makes a *sweep* trustworthy, so later experiments can claim distributions rather than single-topology anecdotes. It is a statistical-quality property layered on the mechanism.

**Independent Test**: Over many distinct seeds on a fixed candidate set larger than the bound, record selections. Distinct seeds yield differing selections, and per-candidate selection frequency across the sweep is approximately uniform within sampling tolerance.

**Acceptance Scenarios**:

1. **Given** a candidate set larger than the upstream degree, **When** selection runs under two distinct seeds, **Then** the selected sets differ.
2. **Given** a large sweep of seeds, **When** per-candidate selection frequencies are aggregated, **Then** they are approximately equal across candidates.
3. **Given** equally-ranked candidates under a seed, **When** the bound forces a choice, **Then** the tie is broken deterministically so the run stays reproducible.

---

### Edge Cases

- **Fewer candidates than the bound**: select all; the bound is an upper limit.
- **Bound of zero**: upstream degree zero ⇒ no upstream connections on that topic (valid for a receive-only configuration); downstream degree zero ⇒ accept no downstream.
- **Equal-ranked candidates**: resolved by a deterministic, stable tie-break on candidate identity — never by incidental data-structure iteration order.
- **Rejected dial**: "rejected" means an **active, explicit over-capacity rejection** from the peer candidate — never a timeout (there is no no-response path). The dialer's only response is to remove the matching pending (`AwaitingAccept`) upstream; there is no retry or back-fill. Every dial in the controlled, lossless substrate is answered with `Accepted` or the explicit rejection.
- **Fewer accepted upstreams than the bound**: settle at under-fill — a measurable outcome, not an error (no back-fill compensates).
- **Membership fixed at selection time**: selection operates over the candidate set known at readiness; dynamic re-selection on membership *change* and epochal rotation are out of scope (see Assumptions).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The dial-side selection policy MUST select at most a configured upstream degree of upstream peers per topic, instead of all candidates.
- **FR-002**: When a topic has candidates at or below the upstream degree, the policy MUST select all of them.
- **FR-003**: Selection MUST be deterministic: given the same seed, node identity, topic, and candidate set, the selected set MUST be identical across repeated runs and across machines, independent of data-structure iteration order.
- **FR-004**: The system MUST accept an optional seed at startup; absent a seed, a fixed default seed of **0** MUST be used so behaviour stays deterministic.
- **FR-005**: A single network seed MUST govern the run; each node MUST derive its own selection from that seed combined with its own identity (and topic), so distinct nodes select differently while the whole topology is reproducible from the one seed.
- **FR-006**: Distinct seeds MUST be able to produce distinct selections for candidate sets larger than the upstream degree.
- **FR-007**: Selection MUST be unbiased with respect to candidate identity: aggregated over many seeds, every candidate has an equal probability of selection.
- **FR-008**: Tie-breaking between equally-ranked candidates MUST be deterministic and stable (resolved on candidate identity).
- **FR-009**: The state-transition function MUST draw no randomness and depend on no wall-clock; the selection randomness MUST be encapsulated within the strategy object (the seed as a field), keeping the transition deterministic.
- **FR-010**: The inbound acceptance policy MUST accept verified, membership-valid requests up to a configured downstream degree per topic, and MUST reject further requests once the bound is reached.
- **FR-011**: An over-capacity rejection MUST be recorded with a distinct cause and MUST send an explicit rejection signal to the requester — distinct from a termination/misbehaviour severance and NOT treated as misbehaviour. A membership-invalid request remains a silent drop (unchanged).
- **FR-012**: The upstream degree and downstream degree MUST each be configurable as a single uniform value, applied identically across all nodes and topics for the run, supplied at startup alongside the seed.
- **FR-013**: The bounded policies MUST be additive: the existing unbounded connect-to-all and accept-from-all behaviours MUST remain available and selectable, so non-bounded runs are unaffected.
- **FR-014**: On a dial rejected for over-capacity (an active, explicit rejection from the peer — there is no timeout/no-response path), the node's ONLY action MUST be to remove the matching pending (`AwaitingAccept`) upstream, so the dialer stops awaiting an acceptance. This feature performs NO retry and NO back-fill — it introduces no failed-peer set and no re-selection on rejection. Re-forming connections (and any retry-to-a-minimum policy) is deferred to a future strategy family and the future heartbeat/reshuffle layer.
- **FR-015**: When rejections leave the realized upstream degree below the bound, the node MUST settle at under-fill (not error); the under-filled outcome MUST be observable through the upstream snapshot.
- **FR-016**: Dial outcomes MUST be observable through state getters/snapshots (not logs) — specifically each node's current upstream and downstream sets — so topology can be asserted and (later) measured.
- **FR-017**: Selection and any new connection state introduced or touched by this feature MUST use deterministic, ordered structures (e.g. `BTreeSet`/`BTreeMap`) so a given seed reproduces identical results across runs and machines. This feature applies ordered structures to its own state within this PR rather than depending on a separate global refactor to do so.
- **FR-018**: The strategy objects MUST be pure and free of hidden state — their only configuration is the seed/bounds set at construction — keeping them compatible with the planned strategies-as-arguments refactor. This feature does NOT itself depend on that relocation: it MAY retain the current strategy injection and migrate when the refactor lands.
- **FR-019**: Strategy construction MUST proceed in two explicit phases (ADR 0028). Phase 1 resolves each seam's strategy *key* into its kind (an absent selector resolves to the seam default; an unknown key is rejected when arguments are parsed). Phase 2 binds each seam's own parameters and constructs the strategies, with each kind validating the parameters it requires via a fallible build that returns a typed error when a required parameter is absent. Each kind MUST see only its own seam's parameters (no shared grab-bag spanning all seams). The CLI/edge layer MUST stay lean — one aggregate build call that maps the typed error once — with no per-strategy validation, repetition, or branching at the edge.

### Key Entities *(include if feature involves data)*

- **Seed**: a single network-level value (default when absent) that, combined with a node's identity and a topic, deterministically governs that node's selection; encapsulated as a field of the selection strategy object.
- **Out-degree bound**: a single run-level value — the maximum upstream peers a node selects per topic, uniform across nodes.
- **In-degree bound**: a single run-level value — the maximum inbound connections a node accepts per topic, uniform across nodes.
- **Bounded selection policy**: the dial-side strategy object yielding the bounded upstream set from (seed, identity, topic, candidates).
- **Bounded acceptance policy**: the inbound strategy object admitting requests up to the downstream degree and rejecting the rest.
- **Rejection signal**: an explicit connection-control action sent by an over-capacity acceptee, distinct from termination/misbehaviour; on receipt the dialer drops the matching pending upstream (no further action).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Re-running with the same seed and membership reproduces an identical selection 100% of the time.
- **SC-002**: No node holds more than the configured upstream degree upstream per topic, nor more than the downstream degree downstream per topic, in 100% of runs.
- **SC-003**: For a candidate set larger than the upstream degree, distinct seeds produce distinct selections.
- **SC-004**: Over a sweep of at least 1,000 seeds on a fixed candidate set, per-candidate selection frequency is uniform within sampling tolerance — a chi-square goodness-of-fit against the uniform expectation does not reject at p < 0.001 (a deliberately strict, low-flake threshold).
- **SC-005**: Selecting the existing unbounded policies reproduces today's full-mesh behaviour exactly; enabling the bounded policies changes no other code path.
- **SC-006**: A dial rejected for over-capacity removes the matching pending upstream and produces no further effects (no retry/back-fill); the resulting under-fill is observable through the upstream snapshot.

## Assumptions

- **Relationship to the determinism/purity refactor (coordinated, not a hard dependency).** The broader refactor (strategies-as-`apply`-arguments, deterministic event-loop scheduling, decouple flag) is a separate workstream owned by the co-developing architect. This feature applies **ordered structures to the state it introduces/touches** itself (FR-017) and keeps its strategy objects pure (FR-018), so it can land without waiting on that refactor; the two workstreams coordinate to avoid conflicting edits to shared files.
- **Hash-derived selection, not a stateful generator.** Reproducibility and the deterministic-transition guarantee are met by deriving selection from a stable keyed hash of (seed, node identity, topic, candidate identity); no generator state is drawn during a transition.
- **Per-network seed, per-node derivation.** One seed governs the run; per-node diversity comes from folding the node identity into the derivation.
- **No retry/back-fill.** On rejection the dialer only drops the pending upstream; the realized degree may under-fill. Re-forming connections is the future heartbeat/reshuffle layer's job, and a **retry-to-a-minimum (back-fill) strategy** is a separate future family (working name `BackfillingSeededBoundedConnection`) — deliberately out of this feature so we first observe the no-retry baseline.
- **New connection-control message.** Over-capacity rejection introduces an explicit rejection action, distinct from `Terminated`/misbehaviour severance.
- **Membership fixed at selection time.** Selection operates over the candidate set known at readiness; dynamic re-selection on membership change and epochal rotation are deferred.
- **Fan-out unchanged.** Forward-to-all remains; a bounded/seeded fan-out variant is deferred to the experiment that needs it (feature 015).
- **Tests included.** This feature ships the unit/integration tests for the strategies (selection determinism/bound/unbiasedness; acceptance bound + rejection; rejection drops the pending upstream). Test-first per the constitution's TDD rule for protocol-behaviour features.
- **Out of scope**: the experiment/testing framework (topology builder, delivery-percentile/latency/propagation metrics) — a separate later feature; the push-based golden-node (M2) model and its registry messages; the edge-vs-golden mode flag; adversarial/Byzantine node behaviour; retry/back-fill and any real-interval/production re-dial driver.
- **Forward note — framework strategy assignment (deferred, informs the framework feature).** The experiment framework's builder is intended to assign strategies via a **network-level default applied to all nodes, overridable per node** (e.g. a golden node at a higher degree). The per-node `Arc<dyn …>` injection at construction already supports this (default object for most nodes, swapped object for overrides); per-node *bound overrides* relax this feature's uniform-per-run rule and arrive with that framework/golden-node work. 005 only provides the strategy types + standalone-node params; it does not build the default/override config.

## Dependencies

- **Coordinated with the determinism/purity refactor (separate branch, NOT a hard dependency)**: the broader strategies-as-arguments + deterministic-scheduling + decouple-`ConnectionSetup`-from-`Synced` work is the co-developing architect's. This feature applies ordered structures to its own state and keeps strategies pure, so it does not block on that refactor — the workstreams coordinate to avoid conflicting edits to shared files (strategy injection sites, `NodeState`).
- Builds on the `004-connections` seams: the dial-side `ConnectionStrategy` (v1 `ConnectToAllCandidates`) and inbound `ConnectionAcceptanceStrategy` (v1 `AcceptFromAllCandidates`).
- Relies on the candidate sets folded from the subscription registry (`008`) and the registration/readiness model (`013`/`014`).
- Governed by the constitution's deterministic state-transition principle (FR-009, FR-017, FR-018).

# Feature Specification: Seeded bounded connection-selection and acceptance strategies

**Feature Branch**: `005-peer-view`

**Created**: 2026-06-29

**Status**: Draft

**Input**: User description: "Add seeded, bounded connection-selection and acceptance strategies to the pubsub node, replacing the full-mesh connect-to-all / accept-from-all so a node forms a bounded partial topology. A node selects at most a configured out-degree of upstream peers per topic, chosen by deterministic seed-based key-hashing (randomness encapsulated inside the strategy object so the state-transition stays deterministic and reproducible from a seed), and accepts inbound requests up to a configured in-degree, sending an explicit rejection when over capacity. A rejected dial is back-filled by re-invoking the existing ConnectionSetup event (no new round or timer event). Includes the tests for these strategies. Builds on a separate prerequisite determinism/purity refactor (strategies moved to apply arguments, ordered data structures replacing HashSet, deterministic scheduling, and a flag decoupling ConnectionSetup from Synced) owned by the co-developing architect. The experiment/testing framework that drives these strategies to measure delivery percentiles, propagation depth, and convergence is a SEPARATE feature added on top later — out of scope here."

## Context

The node today connects to **every** discovered candidate on every joined topic (the `ConnectToAllCandidates` selection policy) and accepts **every** membership-valid inbound request (`AcceptFromAllCandidates`). The result is a complete per-topic mesh: a published message reaches all subscribers in one hop, so dissemination behaves trivially and there is no partial topology to study.

This feature replaces those with **bounded** policies: a node selects at most a configured out-degree of upstream peers per topic and accepts at most a configured in-degree of inbound connections, forming a partial topology. Selection is **seed-reproducible** (the randomness is encapsulated in the strategy object and derived by key-hashing, so it is repeatable from a recorded seed) and **variable** (different seeds explore different topologies). When a dial is rejected for over-capacity, the dialer back-fills by re-selecting the next-ranked candidate on the next re-invocation of the existing `ConnectionSetup` event — no new round or timer.

**Scope**: only the bounded selection/acceptance strategies and their tests. The experiment/testing framework that *drives* these strategies to measure delivery percentiles, propagation depth, and convergence is a **separate feature, added on top later** — out of scope here. The broader **determinism/purity refactor** (moving strategies to `apply` arguments, deterministic scheduling, and a flag decoupling `ConnectionSetup` from `Synced`) is a separate workstream owned by the co-developing architect; this feature does **not** hard-depend on it — it applies ordered data structures to the state it introduces/touches itself, keeps its strategy objects pure, and coordinates with that workstream to avoid conflicting edits.

## Clarifications

### Session 2026-06-29

- Q: For how long does a peer rejected by a dial stay excluded from re-selection? → A: Sticky for the run — once rejected, a peer is never re-dialed for that topic this run; back-fill only moves to lower-ranked untried candidates, and under-fill is terminal when untried candidates run out. No reset on membership change or per-`ConnectionSetup`.
- Q: What does "rejected" mean — a timeout, or an active rejection by the peer candidate? → A: An **active, explicit over-capacity rejection** sent by the peer candidate (an acceptee already at its in-degree). There is **no timeout / no-response path** in this feature: the round/timer mechanism is deliberately excluded, and in the controlled, lossless, manually-stepped substrate every dial is answered with `Accepted` or the explicit rejection. The failed set is populated only by explicit rejections. (Timeout/no-response would only arise with loss or offline peers — a later feature.)
- Q: What is the default seed when none is supplied? → A: **0** (fixed), keeping behaviour deterministic.
- Q: Does this feature hard-depend on the separate determinism/purity refactor, or apply ordered structures itself? → A: It applies **ordered structures (`BTreeSet`/`BTreeMap`) to the state it introduces/touches within this PR** and keeps its strategy objects pure, so it does **not** hard-depend on the strategies-as-arguments relocation (that stays the co-developing architect's workstream; the two coordinate to avoid conflicts). SC-004's uniformity tolerance is pinned to a chi-square gate at p < 0.001.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Reproducible bounded upstream selection (Priority: P1)

A node configured with an out-degree bound and a seed selects at most that many upstream peers per topic from its candidate set, forming a partial topology rather than a full mesh. Re-running with the same seed and membership reproduces an identical selection.

**Why this priority**: This is the core capability — without a bounded, reproducible partial topology there is nothing for the later experiment framework to measure, and results would be either degenerate (full mesh) or irreproducible. Everything else builds on it.

**Independent Test**: Construct a node (or a small set of nodes) with candidate sets larger than the out-degree bound, under seed s; capture the selected upstream set. Rebuild identically under s; the selection is identical, and no node selects more than the bound per topic.

**Acceptance Scenarios**:

1. **Given** a topic with more candidates than the out-degree bound, **When** a node selects, **Then** it selects exactly the bound's worth of upstream peers on that topic.
2. **Given** a topic with candidates at or below the bound, **When** a node selects, **Then** it selects all of them (the bound is a ceiling, not a quota).
3. **Given** the same seed, node identity, topic, and candidate set, **When** selection runs in two separate runs (including on different machines), **Then** the selected sets are identical.
4. **Given** no seed supplied at startup, **When** selection runs, **Then** a fixed default seed is used and behaviour stays deterministic.

---

### User Story 2 - Bounded inbound acceptance with explicit rejection and back-fill (Priority: P2)

A node accepts verified, membership-valid inbound requests only up to a configured in-degree per topic. Beyond the bound it sends an explicit rejection (distinct from a termination/misbehaviour severance). A dialer whose request is rejected marks that peer failed and, on the next `ConnectionSetup` re-invocation, re-selects the next-ranked untried candidate — backfilling toward its out-degree until the bound is met or candidates are exhausted.

**Why this priority**: Bounding inbound degree gives a second topology lever and is the inbound mirror of the dial-side bound; back-fill keeps realized out-degree close to target despite rejections. P2 because the dial-side bound (US1) alone already yields a partial topology.

**Independent Test**: Drive a node more inbound requests than its in-degree on a topic — exactly the bound's worth are accepted, the rest dropped with the over-capacity cause and an explicit rejection sent, with no severance. Separately, reject a dialer's request and re-invoke `ConnectionSetup` — the dialer re-selects the next-ranked candidate; with candidates exhausted it settles at under-fill.

**Acceptance Scenarios**:

1. **Given** a node below its in-degree on a topic, **When** a verified membership-valid request arrives, **Then** it is accepted.
2. **Given** a node at its in-degree on a topic, **When** a further verified request arrives, **Then** it is dropped with the over-capacity cause, an explicit rejection is sent, and no downstream entry is added.
3. **Given** a dial rejected for over-capacity, **When** `ConnectionSetup` is re-invoked, **Then** the dialer re-selects the next-ranked untried candidate over the viable set (candidates minus failed peers).
4. **Given** the viable candidates are exhausted below the out-degree, **When** selection re-runs, **Then** the node settles at under-fill (realized out-degree below the bound), observably and without error.

---

### User Story 3 - Seed-varied, identity-unbiased selection (Priority: P3)

Across a sweep of distinct seeds, selections differ from one another, and no candidate is systematically preferred or excluded — over many seeds each candidate is equally likely to be selected.

**Why this priority**: Reproducibility (US1) makes a single run repeatable; this makes a *sweep* trustworthy, so later experiments can claim distributions rather than single-topology anecdotes. It is a statistical-quality property layered on the mechanism.

**Independent Test**: Over many distinct seeds on a fixed candidate set larger than the bound, record selections. Distinct seeds yield differing selections, and per-candidate selection frequency across the sweep is approximately uniform within sampling tolerance.

**Acceptance Scenarios**:

1. **Given** a candidate set larger than the out-degree, **When** selection runs under two distinct seeds, **Then** the selected sets differ.
2. **Given** a large sweep of seeds, **When** per-candidate selection frequencies are aggregated, **Then** they are approximately equal across candidates.
3. **Given** equally-ranked candidates under a seed, **When** the bound forces a choice, **Then** the tie is broken deterministically so the run stays reproducible.

---

### Edge Cases

- **Fewer candidates than the bound**: select all; the bound is an upper limit.
- **Bound of zero**: out-degree zero ⇒ no upstream connections on that topic (valid for a receive-only configuration); in-degree zero ⇒ accept no downstream.
- **Equal-ranked candidates**: resolved by a deterministic, stable tie-break on candidate identity — never by incidental data-structure iteration order.
- **Rejected dial**: "rejected" means an **active, explicit over-capacity rejection** from the peer candidate — never a timeout (there is no no-response path). The peer is marked failed and excluded from the viable set **for the rest of the run** (sticky — never re-dialed for that topic this run); the next `ConnectionSetup` re-selects the next-ranked untried candidate. Every dial in the controlled, lossless substrate is answered with `Accepted` or the explicit rejection.
- **Candidates exhausted below the bound**: settle at under-fill — a measurable outcome, not an error.
- **Membership fixed at selection time**: selection and re-selection operate over the candidate set known at readiness; dynamic re-selection on membership *change* and epochal rotation are out of scope (see Assumptions).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The dial-side selection policy MUST select at most a configured out-degree of upstream peers per topic, instead of all candidates.
- **FR-002**: When a topic has candidates at or below the out-degree, the policy MUST select all of them.
- **FR-003**: Selection MUST be deterministic: given the same seed, node identity, topic, and candidate set, the selected set MUST be identical across repeated runs and across machines, independent of data-structure iteration order.
- **FR-004**: The system MUST accept an optional seed at startup; absent a seed, a fixed default seed of **0** MUST be used so behaviour stays deterministic.
- **FR-005**: A single network seed MUST govern the run; each node MUST derive its own selection from that seed combined with its own identity (and topic), so distinct nodes select differently while the whole topology is reproducible from the one seed.
- **FR-006**: Distinct seeds MUST be able to produce distinct selections for candidate sets larger than the out-degree.
- **FR-007**: Selection MUST be unbiased with respect to candidate identity: aggregated over many seeds, every candidate has an equal probability of selection.
- **FR-008**: Tie-breaking between equally-ranked candidates MUST be deterministic and stable (resolved on candidate identity).
- **FR-009**: The state-transition function MUST draw no randomness and depend on no wall-clock; the selection randomness MUST be encapsulated within the strategy object (the seed as a field), keeping the transition deterministic.
- **FR-010**: The inbound acceptance policy MUST accept verified, membership-valid requests up to a configured in-degree per topic, and MUST reject further requests once the bound is reached.
- **FR-011**: An over-capacity rejection MUST be recorded with a distinct cause and MUST send an explicit rejection signal to the requester — distinct from a termination/misbehaviour severance and NOT treated as misbehaviour. A membership-invalid request remains a silent drop (unchanged).
- **FR-012**: The out-degree and in-degree MUST each be configurable as a single uniform value, applied identically across all nodes and topics for the run, supplied at startup alongside the seed.
- **FR-013**: The bounded policies MUST be additive: the existing unbounded connect-to-all and accept-from-all behaviours MUST remain available and selectable, so non-bounded runs are unaffected.
- **FR-014**: On a dial rejected for over-capacity (an active, explicit rejection from the peer — there is no timeout/no-response path), the node MUST mark that peer failed for the topic and MUST NOT re-dial it for that topic for the remainder of the run (the failed set is sticky — no reset on membership change or per-`ConnectionSetup`). A subsequent `ConnectionSetup` re-invocation MUST re-select the next-ranked untried candidate over the viable set (candidates minus failed peers), working toward the out-degree until the bound is met or candidates are exhausted. Re-dial MUST be driven by re-invoking the existing `ConnectionSetup` event — NO new round/tick event and NO wall-clock timer.
- **FR-015**: When the viable candidates are exhausted before the out-degree is met, the node MUST settle at under-fill (realized out-degree below the bound) rather than erroring; the under-filled outcome MUST be observable.
- **FR-016**: Dial outcomes MUST be observable through state getters/snapshots (not logs) — at minimum the count of explicit rejections and each node's current upstream set — so behaviour can be asserted and (later) measured.
- **FR-017**: Selection and any new connection state introduced or touched by this feature MUST use deterministic, ordered structures (e.g. `BTreeSet`/`BTreeMap`) so a given seed reproduces identical results across runs and machines. This feature applies ordered structures to its own state within this PR rather than depending on a separate global refactor to do so.
- **FR-018**: The strategy objects MUST be pure and free of hidden state — their only configuration is the seed/bounds set at construction — keeping them compatible with the planned strategies-as-arguments refactor. This feature does NOT itself depend on that relocation: it MAY retain the current strategy injection and migrate when the refactor lands.

### Key Entities *(include if feature involves data)*

- **Seed**: a single network-level value (default when absent) that, combined with a node's identity and a topic, deterministically governs that node's selection; encapsulated as a field of the selection strategy object.
- **Out-degree bound**: a single run-level value — the maximum upstream peers a node selects per topic, uniform across nodes.
- **In-degree bound**: a single run-level value — the maximum inbound connections a node accepts per topic, uniform across nodes.
- **Bounded selection policy**: the dial-side strategy object yielding the bounded upstream set from (seed, identity, topic, candidates).
- **Bounded acceptance policy**: the inbound strategy object admitting requests up to the in-degree and rejecting the rest.
- **Rejection signal**: an explicit connection-control action sent by an over-capacity acceptee, distinct from termination/misbehaviour; it marks the peer failed so the next `ConnectionSetup` back-fills.
- **Failed-peer set**: per-node, per-topic record of peers a dial was rejected by (explicit over-capacity rejections only), excluded from the viable candidate view before selection and sticky for the run (never reset within a run).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Re-running with the same seed and membership reproduces an identical selection 100% of the time.
- **SC-002**: No node holds more than the configured out-degree upstream per topic, nor more than the in-degree downstream per topic, in 100% of runs.
- **SC-003**: For a candidate set larger than the out-degree, distinct seeds produce distinct selections.
- **SC-004**: Over a sweep of at least 1,000 seeds on a fixed candidate set, per-candidate selection frequency is uniform within sampling tolerance — a chi-square goodness-of-fit against the uniform expectation does not reject at p < 0.001 (a deliberately strict, low-flake threshold).
- **SC-005**: Selecting the existing unbounded policies reproduces today's full-mesh behaviour exactly; enabling the bounded policies changes no other code path.
- **SC-006**: A dial rejected for over-capacity is back-filled to the next-ranked candidate on the next `ConnectionSetup` re-invocation; under-fill on exhaustion is observable.
- **SC-007**: The count of explicit rejections is observable via a state getter.

## Assumptions

- **Relationship to the determinism/purity refactor (coordinated, not a hard dependency).** The broader refactor (strategies-as-`apply`-arguments, deterministic event-loop scheduling, decouple flag) is a separate workstream owned by the co-developing architect. This feature applies **ordered structures to the state it introduces/touches** itself (FR-017) and keeps its strategy objects pure (FR-018), so it can land without waiting on that refactor; the two workstreams coordinate to avoid conflicting edits to shared files.
- **Hash-derived selection, not a stateful generator.** Reproducibility and the deterministic-transition guarantee are met by deriving selection from a stable keyed hash of (seed, node identity, topic, candidate identity); no generator state is drawn during a transition.
- **Per-network seed, per-node derivation.** One seed governs the run; per-node diversity comes from folding the node identity into the derivation.
- **Re-dial = `ConnectionSetup` re-invocation.** The heartbeat is the existing `ConnectionSetup` event (no new round/tick, no timer). Re-running it re-selects over the viable set, so back-fill needs no new event; the decouple-from-`Synced` flag (prerequisite) lets a driver fire it explicitly.
- **New connection-control message.** Over-capacity rejection introduces an explicit rejection action, distinct from `Terminated`/misbehaviour severance.
- **Membership fixed at selection time.** Selection and back-fill operate over the candidate set known at readiness; dynamic re-selection on membership change and epochal rotation are deferred.
- **Fan-out unchanged.** Forward-to-all remains; a bounded/seeded fan-out variant is deferred to the experiment that needs it.
- **Tests included.** This feature ships the unit/integration tests for the strategies (selection determinism/bound/unbiasedness; acceptance bound + rejection; back-fill). Test-first per the constitution's TDD rule for protocol-behaviour features.
- **Out of scope**: the experiment/testing framework (topology builder, delivery-percentile/latency/propagation metrics) — a separate later feature; the push-based golden-node (M2) model and its registry messages; the edge-vs-golden mode flag; adversarial/Byzantine node behaviour; any real-interval/production re-dial driver.
- **Forward note — framework strategy assignment (deferred, informs the framework feature).** The experiment framework's builder is intended to assign strategies via a **network-level default applied to all nodes, overridable per node** (e.g. a golden node at a higher degree). The per-node `Arc<dyn …>` injection at construction already supports this (default object for most nodes, swapped object for overrides); per-node *bound overrides* relax this feature's uniform-per-run rule and arrive with that framework/golden-node work. 005 only provides the strategy types + standalone-node params; it does not build the default/override config.

## Dependencies

- **Coordinated with the determinism/purity refactor (separate branch, NOT a hard dependency)**: the broader strategies-as-arguments + deterministic-scheduling + decouple-`ConnectionSetup`-from-`Synced` work is the co-developing architect's. This feature applies ordered structures to its own state and keeps strategies pure, so it does not block on that refactor — the workstreams coordinate to avoid conflicting edits to shared files (strategy injection sites, `NodeState`).
- Builds on the `004-connections` seams: the dial-side `ConnectionStrategy` (v1 `ConnectToAllCandidates`) and inbound `ConnectionAcceptanceStrategy` (v1 `AcceptFromAllCandidates`).
- Relies on the candidate sets folded from the subscription registry (`008`) and the registration/readiness model (`013`/`014`).
- Governed by the constitution's deterministic state-transition principle (FR-009, FR-017, FR-018).

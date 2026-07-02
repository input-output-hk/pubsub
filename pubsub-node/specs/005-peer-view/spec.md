# Feature Specification: Verifiable hash-gated connection selection & bounded acceptance

**Feature Branch**: `005-peer-view`

**Created**: 2026-06-29 (redesigned 2026-07-02)

**Status**: Draft

**Input**: User description: "Add seeded, bounded connection-selection and acceptance strategies to the pubsub node, replacing the full-mesh connect-to-all / accept-from-all so a node forms a bounded partial topology. A node selects at most a configured upstream degree of upstream peers per topic, chosen by deterministic seed-based key-hashing (randomness encapsulated inside the strategy object so the state-transition stays deterministic and reproducible from a seed), and accepts inbound requests up to a configured downstream degree, sending an explicit rejection when over capacity. A rejected dial is back-filled by re-invoking the existing ConnectionSetup event (no new round or timer event). Includes the tests for these strategies. Builds on a separate prerequisite determinism/purity refactor (strategies moved to apply arguments, ordered data structures replacing HashSet, deterministic scheduling, and a flag decoupling ConnectionSetup from Synced) owned by the co-developing architect. The experiment/testing framework that drives these strategies to measure delivery percentiles, propagation depth, and convergence is a SEPARATE feature added on top later — out of scope here."

> **Redesign note (2026-07-02).** After review with the formal engineers (Ezequiel, Denis), the mechanism moved from *seeded pseudo-random sampling* to the **bucketed-pull** model (`docs/extensions/bucketed-pull.md`): an upstream edge exists iff a **verifiable per-round hash-bucket predicate** holds, so the acceptor independently verifies a request and an adversary cannot exhaust a victim's serving slots by spamming. Per-topic bucket count `B` scales with the topic's size against a **fixed fanout `RF`**; acceptance caps downstream at `OC = ⌈RF + c·√RF⌉`. Fan-out stays `ForwardToAll`. The original seeded/degree-parameter requirements are superseded by the Functional Requirements below; the pivot is recorded under Clarifications → Session 2026-07-02.

## Context

Today a node connects to **every** candidate on every joined topic (`ConnectToAllCandidates`) and accepts **every** membership-valid request (`AcceptFromAllCandidates`) — a full mesh with trivial dissemination and no defence against a peer that spams connection requests to exhaust a victim's capacity.

This feature adopts the **bucketed-pull** overlay (`docs/extensions/bucketed-pull.md`), verifiable and adversary-resistant:

- **Selection (dial side).** For a joined topic `T` at the current interval `I`, node `D` dials candidate `U` **iff** the bucket predicate holds:
  `H(genesis, T, D, U, I) mod B == 0`, where `B = max(1, round(|candidates_T| / RF))`.
  `RF` (random fanout) is a **fixed** configured constant, so the expected out-degree per topic is ≈ `RF` regardless of topic size, and small topics (where `B` floors to 1) connect to **all** candidates automatically.
- **Acceptance (inbound side).** On a verified `Request` from `D` on `T`, acceptor `U` accepts iff **(a)** the same predicate `H(genesis, T, D, U, I) mod B == 0` holds (the request is legitimate this interval — verified by recomputing one hash), **(b)** `T` is registered, **(c)** `D` and `U` share interest in `T` (both members), and **(d)** `U` holds fewer than `OC = ⌈RF + c·√RF⌉` downstream on `T`. Otherwise it rejects.
- **Fan-out.** Unchanged: `ForwardToAll` disseminates each new message to every downstream on its topic.

**Why verifiable + bucketed.** A valid edge needs the predicate to hold, and each `(D, U)` pair satisfies it with probability `1/B` under the hash; an adversary sharing a victim's descriptor across sybils gets only its `1/B` share of the victim's slots per interval — the same density an honest node gets, so there is **no amplification** (`bucketed-pull.md` §Concentration). The predicate is a pure, public function; both peers compute it, so the acceptor verifies rather than trusts.

**View.** The model has a per-peer discovery **view** `H_v` that selection samples within. **v1 uses `view = the full candidate set`** (no discovery-layer sampling yet), so `B` is derived from the full per-topic candidate count. The `H_v` knob is deferred to the discovery/experiment layer, which can sub-sample the view before the predicate; the seam is shaped so that drops in without change.

**Small topics.** No special case: `B = max(1, round(|candidates_T| / RF))` floors at 1, and `mod 1 == 0` is always true, so a topic with ≤ ~`RF` candidates connects to all of them (the graceful degradation `bucketed-pull.md` §Small-topic regime describes). A fixed `RF` (Denis's conservative option) makes this automatic and avoids the `ln`-based degeneracy of a size-derived degree.

**Interval.** The single dial-trigger event `ConnectionSetup` is renamed **`Heartbeat { interval }`**, carrying an advancing 0-based counter. `(genesis, interval)` stand in for the model's per-round beacon `nonce_R` (a real unbiasable beacon — block hash / VRF — is deferred). **v1 fires one interval**; periodic heartbeats and cross-interval connection rotation/teardown are a later feature (the interval is threaded through the seam so that layer drops in without reshaping it).

**Scope**: the two verifiable strategies + their tests + the `Heartbeat` rename. Fan-out stays `ForwardToAll` (bounded fan-out dropped). **Out of scope**: discovery/view sampling (`H_v`), periodic heartbeats + rotation/teardown, the real beacon, the **incentive/chain layer** (deposits `D`, sybil bound `K`, on-chain identity, over-capacity slashing reports), golden/relay tiers, and the experiment framework.

## Clarifications

### Session 2026-06-29

- Q: Default seed when none supplied? → A: **0** (fixed). *(Repurposed 2026-07-02 as the public genesis nonce.)*
- Q: "rejected" — a timeout or an active rejection? → A: An **active, explicit** rejection. No timeout/no-response path: the controlled substrate answers every dial.

### Session 2026-07-02 (redesign → bucketed-pull)

- Q: Selection mechanism? → A: **Verifiable hash-bucket predicate** (`bucketed-pull.md`), replacing seeded PRNG sampling. `H(genesis, T, requester, candidate, interval) mod B == 0`; the acceptor recomputes to verify. Rationale: an adversary cannot exhaust a victim's slots (only its `1/B` hash share is valid per interval).
- Q: Degree control? → A: **Fixed `RF`** (Denis's conservative option — sized for the expected max N, swept by the framework). `B = max(1, round(|candidates_T| / RF))` ⇒ expected out-degree ≈ `RF`. **Small topics handled automatically** by `B=1` (connect-to-all); no `ln`-degeneracy, no network-size estimation.
- Q: Accept cap + scope? → A: `OC = ⌈RF + c·√RF⌉` (variance buffer, `c` default 3, configurable), applied **per topic**.
- Q: Predicate inputs / directionality? → A: `H(genesis, T, requester, candidate, interval)`, ordered `(requester, candidate)` so it is directional; both sides compute the identical tuple. `H` = SHA-256 over a canonical length-prefixed encoding (fixed algorithm, cross-machine stable; not `DefaultHasher`).
- Q: View `H_v` in v1? → A: **`view = full candidate set`** (no sampling); `B` derives from the full per-topic candidate count. `H_v` sampling deferred to the discovery/experiment layer.
- Q: `nonce_R`? → A: Deferred; **`(genesis, interval)` stand in** for v1.
- Q: Interval mechanics? → A: `Event::ConnectionSetup` → **`Event::Heartbeat { interval: u64 }`**, driver-fired (no wall-clock). **v1 fires one interval**; rotation/teardown deferred.
- Q: Acceptance gate — is the bucket predicate on top of membership/topic checks? → A: **Yes, additive.** Accept iff predicate ∧ topic-registered ∧ shared-interest ∧ under `OC`.
- Q: Silent drop vs explicit `Rejected`? → A: A **failure of the predicate, topic-registration, or shared-interest is a silent drop** (no reply — leaks nothing to an adversary). Only **over-capacity of an otherwise-legitimate request** sends an explicit `Rejected`. (Honest dialers only request predicate-valid peers, so they never hit the silent-drop path — both sides compute the same predicate.)
- Q: Scope of the incentive layer? → A: **Out.** Deposits, sybil-count bounds, on-chain identity, slashing/over-capacity reports are the chain/incentive layer, not these strategies.
- Q: Retry/back-fill? → A: **None** (as before). On `Rejected` the dialer drops the pending upstream; realized degree may under-fill. Retry is a future strategy family.
- Q: Names? → A: `HashGatedConnection` (dial), `VerifiableBoundedAcceptance` (accept), `ForwardToAll` (fan-out); shared predicate `strategies::edge::is_valid_edge`; event `Heartbeat`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Verifiable hash-gated upstream selection (Priority: P1)

A node forms its per-topic upstream edges from the bucket predicate over `(genesis, T, self, candidate, interval)`, giving a bounded partial topology whose expected degree is ≈ `RF`. Re-running with the same genesis, membership, and interval reproduces an identical edge set.

**Independent Test**: With more candidates than `RF`, capture a node's upstreams for a genesis + interval; recompute — identical. Sweep candidate-set size and confirm expected degree tracks `RF`. With ≤ ~`RF` candidates, all are selected (`B=1`).

**Acceptance Scenarios**:

1. **Given** more candidates than `RF`, **When** selection runs, **Then** exactly the candidates with `H(genesis, T, self, candidate, interval) mod B == 0` are selected (`B = max(1, round(|candidates_T|/RF))`), expected count ≈ `RF`.
2. **Given** ≤ ~`RF` candidates on the topic, **When** selection runs, **Then** `B = 1` and all candidates are selected (connect-to-all).
3. **Given** identical genesis, identity, topic, candidates, interval, **When** selection runs twice (incl. different machines), **Then** the selected sets are identical.
4. **Given** no genesis supplied, **When** selection runs, **Then** the default genesis 0 is used and behaviour stays deterministic.

---

### User Story 2 - Verifiable bounded acceptance (Priority: P1)

On a verified `Request`, a node accepts only if the request is legitimate this interval (the predicate holds), the topic is registered, requester and acceptor share interest, and the acceptor is under its `OC` cap on the topic. It rejects otherwise — a silent drop for a predicate/topic/interest failure, an explicit `Rejected` for over-capacity.

**Independent Test**: Send a predicate-valid, membership-valid request under cap → accepted. Send one whose predicate fails this interval → silently dropped. Drive past `OC` predicate-valid requests → the extra refused with the over-capacity cause + `Rejected`. Send on an unregistered topic / from a non-member → dropped.

**Acceptance Scenarios**:

1. **Given** a predicate-valid, membership-valid request on a registered topic under cap, **When** it arrives, **Then** it is accepted (downstream recorded, `Accepted` sent).
2. **Given** a request whose predicate does **not** hold for the interval, **When** it arrives, **Then** it is silently dropped (no acceptance, no reply) — an adversary cannot force an edge.
3. **Given** a node at `OC` on the topic, **When** a further predicate-valid request arrives, **Then** it is dropped with the over-capacity cause and an explicit `Rejected` is sent (not misbehaviour).
4. **Given** a request on an unregistered topic or from a non-member, **When** it arrives, **Then** it is silently dropped (membership-invalid).

---

### User Story 3 - Adversary cannot exhaust connection slots (Priority: P2)

An id spamming requests cannot occupy more of a victim's downstream slots than its `1/B` hash share: only predicate-satisfying requests are accepted, the same density an honest peer has.

**Independent Test**: For a fixed genesis + interval, enumerate the requests an attacker id can get accepted at a victim; confirm the accepted fraction matches the `1/B` density and predicate-failing requests are all rejected.

**Acceptance Scenarios**:

1. **Given** an id whose predicate does not hold for `(id, victim, T, I)`, **When** it spams requests, **Then** every one is dropped — no slot taken.
2. **Given** a sweep of intervals, **When** accepted requests from one id are counted, **Then** the accepted fraction ≈ the honest `1/B` density (no amplification).

---

### Edge Cases

- **Small topic** (`|candidates_T| ≤ ~RF`): `B=1` ⇒ every candidate selected/accepted (connect-to-all) — no special case, and (per the doc) bucketing gives no meaningful security here; delivery leans on relay/local tiers (out of scope).
- **No candidates** (only self on the topic): no edges.
- **Predicate false for a candidate this interval**: simply not an upstream — expected (degree is probabilistic around `RF`).
- **Illegitimate / membership-invalid request**: silent drop (no reply, no leak); no `Rejected`.
- **Over-capacity of a legitimate request**: dropped with over-capacity cause + explicit `Rejected`; dialer drops the pending upstream. No retry/back-fill.
- **Order independence**: the predicate is evaluated per candidate independently, so selection is order-independent; ordered structures are kept for deterministic effect emission.
- **Single interval (v1)**: the heartbeat fires once; no rotation/teardown across intervals.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The dial-side policy MUST select, per joined topic, exactly the candidates for which `H(genesis, self_id, candidate_id, topic, interval) mod B == 0`, where `B = max(1, round(|candidates_on_topic| / RF))`.
- **FR-002**: The edge predicate MUST be a pure, deterministic, verifiable function of the public values `(genesis, requester_id, candidate_id, topic, interval)`, ordered `(requester, candidate)` (directional); it MUST use a fixed hash (SHA-256 over a canonical length-prefixed encoding — not `DefaultHasher`), draw no ambient randomness, depend on no wall-clock, and reproduce identically across machines.
- **FR-003**: `RF` (random fanout) MUST be a **fixed** configured constant applied uniformly for the run (not derived from network size), so expected out-degree per topic ≈ `RF` and small topics connect to all candidates automatically via `B=1`.
- **FR-004**: The system MUST accept an optional **genesis** nonce at startup (public, folded into the predicate); absent one, a fixed default of **0** MUST be used.
- **FR-005**: The predicate MUST fold the node's own identity and the topic in, so distinct nodes and topics yield distinct edge sets while the whole topology is reproducible from the genesis.
- **FR-006**: Selection and acceptance MUST use the **current interval**, supplied by an `Event::Heartbeat { interval }` carrying an advancing 0-based counter. The core MUST NOT advance the interval by wall-clock; a driver fires the heartbeat. The node MUST retain the current interval so the acceptor verifies against it.
- **FR-007**: The inbound acceptance policy MUST accept a verified `Request` from `requester` on `topic` iff ALL hold: (a) `H(genesis, requester, self_id, topic, interval) mod B == 0`; (b) the topic is registered; (c) requester and acceptor share interest in the topic (both members); (d) the acceptor holds fewer than `OC = ⌈RF + c·√RF⌉` downstream on the topic (`c` a configured buffer constant, default 3). Otherwise it MUST reject.
- **FR-008**: A **failure of (a), (b), or (c)** MUST be a **silent drop** (no reply — leaks nothing). Only **over-capacity** (a–c hold, (d) fails) MUST send an explicit `Rejected` (distinct cause, NOT misbehaviour). Because both peers compute the same predicate, an honest dialer's request always satisfies (a) at the acceptor, so honest nodes see only `Accepted` or over-capacity `Rejected`.
- **FR-009**: On a `Rejected`, the dialer's ONLY action MUST be to remove the matching pending (`AwaitingAccept`) upstream. NO retry, NO back-fill, NO failed-peer state; realized degree MAY settle below `RF`. Re-forming connections is deferred to the heartbeat-rotation layer + a future retry strategy family.
- **FR-010**: The policies MUST be additive: the unbounded `connect-to-all` / `accept-from-all` behaviours MUST remain available and selectable.
- **FR-011**: The view is the **full candidate set** in this feature (no discovery-layer sampling); `B` derives from the full per-topic candidate count. The seam MUST be shaped so a later discovery/experiment layer can sub-sample a view `H_v` before the predicate without a seam change.
- **FR-012**: This feature MUST fire the heartbeat for a **single interval** (at readiness). Periodic heartbeats and cross-interval connection **rotation/teardown** are out of scope; `Heartbeat { interval }` MUST be shaped so that layer drops in without reshaping the seam.
- **FR-013**: Topology MUST be observable through state getters/snapshots (not logs) — each node's current upstream and downstream sets — so behaviour (degree ≈ RF, verifiability, under-fill, `OC` bound) can be asserted and measured.
- **FR-014**: Selection/acceptance and connection state MUST use deterministic, ordered structures (`BTreeSet`/`BTreeMap`) for order-stable effects/snapshots; the strategy objects MUST be pure and free of hidden state (genesis/RF/`c` are fields, interval is an input), compatible with the strategies-as-arguments refactor.
- **FR-015**: Strategy construction MUST follow the two-phase pattern (ADR 0028): phase-1 key → kind (default on absent, unknown rejected at parse); phase-2 binds each seam's own parameters (genesis, `RF`, `c`) and builds, validating required params via a fallible build; the edge maps one typed error.
- **FR-016**: The incentive/chain layer (deposits, sybil-count bounds, on-chain identity, over-capacity slashing reports) MUST NOT be implemented by these strategies — the bucket predicate + `OC` cap + membership/topic gates are the overlay mechanics only.

### Key Entities *(include if feature involves data)*

- **Genesis**: a public network-level nonce (default 0), folded into the predicate; a field of the strategy objects.
- **Interval**: an advancing 0-based counter carried by `Heartbeat`; an input to selection and acceptance. v1 uses a single interval; stands in (with genesis) for the model's `nonce_R`.
- **`RF` (random fanout)**: a fixed configured constant — the target expected out-degree per topic.
- **Bucket count `B`**: `max(1, round(|candidates_on_topic| / RF))` — derived per topic; `B=1` ⇒ connect-to-all.
- **`OC` (accept cap)**: `⌈RF + c·√RF⌉` downstream per topic (`c` default 3).
- **Edge predicate**: `is_valid_edge(genesis, topic, requester, candidate, interval, B) = H(…) mod B == 0` — the pure verifiable gate shared by both seams.
- **`HashGatedConnection`** / **`VerifiableBoundedAcceptance`**: the dial and accept strategies.
- **Rejection signal**: an explicit control action sent only on over-capacity of a legitimate request; the dialer drops the matching pending upstream on receipt.

## Success Criteria *(mandatory)*

- **SC-001**: Re-running selection with the same genesis, membership, topic, and interval reproduces an identical upstream set 100% of the time (incl. across machines).
- **SC-002**: For the same `(requester, candidate, topic, interval)`, the acceptor's predicate result equals the dialer's in 100% of cases — the edge is verifiable (capacity aside).
- **SC-003**: Over a sweep of ≥ 1,000 intervals (or genesis values) on a fixed candidate set with `B>1`, per-candidate selection frequency is uniform within tolerance — chi-square goodness-of-fit does not reject at p < 0.001.
- **SC-004**: Expected out-degree per topic tracks `RF` across candidate-set sizes (within tolerance); no node exceeds `OC` downstream per topic in 100% of runs.
- **SC-005**: A single id spamming a victim has its accepted fraction bounded by the `1/B` density — no amplification; all predicate-failing requests are dropped.
- **SC-006**: Below the small-topic point (`|candidates_T| ≤ ~RF`, `B=1`) both seams reproduce connect-to-all / accept-all; selecting the unbounded policies is likewise unchanged (no other code path affected).
- **SC-007**: A `Rejected` removes the matching pending upstream and produces no further effects (no retry/back-fill); resulting under-fill is observable through the upstream snapshot.

## Assumptions

- **Public, verifiable predicate.** Genesis + interval are public; security is from the predicate being identically computable by both peers and pseudo-uniform per pair (`1/B`), not from secrecy.
- **Fixed `RF`.** Conservative fixed fanout (Denis's option 1) — handles small topics automatically (`B=1`), no `ln`-degeneracy, no size estimation. `RF` and the `OC` buffer `c` are config (swept by the framework); placeholder defaults `RF≈8`, `c=3`.
- **View = full candidate set (v1).** No discovery sampling yet; `H_v` deferred; the seam supports sub-sampling later.
- **`(genesis, interval)` stand in for `nonce_R`.** Real unbiasable beacon deferred.
- **Single interval (v1).** Periodic heartbeats + rotation/teardown deferred; the `Heartbeat { interval }` shape supports them.
- **No retry/back-fill.** Dialer drops the pending upstream on `Rejected`.
- **Fan-out unchanged** (`ForwardToAll`).
- **Genesis reuses `--seed`** (renamed conceptually to the genesis nonce; default 0).
- **Coordinated with** the determinism/purity refactor (strategies pure + ordered structures), not hard-dependent.
- **Out of scope**: incentive/chain layer (deposits/slashing/identity), real beacon, discovery/view sampling, periodic rotation/teardown, golden/relay tiers, experiment framework.

## Dependencies

- Builds on `004-connections` seams (`ConnectionStrategy`/`ConnectToAllCandidates`, `ConnectionAcceptanceStrategy`/`AcceptFromAllCandidates`); renames the `ConnectionSetup` dial trigger to `Heartbeat { interval }`; threads the interval through both seams.
- Relies on the candidate sets folded from the subscription registry (`008`) — including per-topic candidate counts for `B` — and the registration/readiness model (`013`/`014`).
- Realises the overlay mechanics of `docs/extensions/bucketed-pull.md` (the incentive/chain layer of that doc is out of scope).
- Governed by the constitution's deterministic state-transition principle (FR-002, FR-006, FR-014).

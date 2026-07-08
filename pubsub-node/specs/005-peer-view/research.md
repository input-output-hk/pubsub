# Phase 0 Research: Verifiable hash-gated connection-selection and bounded acceptance

Decisions resolving the Technical Context unknowns. Per item: **Decision / Rationale / Alternatives**. Redesigned 2026-07-02 from seeded-PRNG sampling to the verifiable bucketed-pull model (`docs/extensions/bucketed-pull.md`, ADR 0024/0025/0030).

## R1 — Selection mechanism: verifiable per-interval hash-bucket predicate

**Decision**: An upstream edge exists iff a **verifiable hash-bucket predicate** holds. For a joined topic `T` at interval `I`, node `D` dials candidate `U` iff `is_valid_edge(genesis, T, D, U, I, B) = H(genesis, T, D, U, I) mod B == 0`, with `B = max(1, round(|candidates_T| / target_degree))`. `target_degree` is a **fixed** configured target (connection) degree (a field of the strategy), so expected out-degree per topic ≈ `target_degree` and a topic with `≤ ~target_degree` candidates has `B = 1`, admitting every edge (connect-to-all). The predicate is a pure, public function — the acceptor recomputes it to **verify** (R4), so no PRNG and no state is carried.

**Rationale**: verifiability + adversary-resistance are the point (superseding the earlier seeded PRNG). Because each `(D, U)` pair satisfies the predicate with probability `1/B`, an adversary sharing a victim's descriptor across sybils gets only its `1/B` share of the victim's slots per interval — no amplification (`bucketed-pull.md` §Concentration). Determinism rests on a **fixed hash** (SHA-256) + **ordered inputs** for stable effect emission; the predicate is order-independent by construction (FR-002). Folding `D`'s identity and `T` in gives per-node/per-topic diversity while the whole topology is reproducible from the genesis (FR-005). A fixed `target_degree` (Denis's conservative option) handles small topics automatically via `B = 1` and avoids the `ln`-degeneracy of a size-derived degree.

**Alternatives**: seeded PRNG sampling (`ChaCha20Rng` partial Fisher–Yates over the candidate set) — the original design, **superseded**: sampling is not verifiable by a third party, so an acceptor could not independently check a request and an adversary could exhaust a victim's slots. Keyed-hash *ranking* (lowest-k by digest) — rejected (needs a bound `k` = a degree parameter; the bucket test needs only a fixed `target_degree`, and scales `B` with topic size). Wall-clock/entropy input — rejected (not reproducible/verifiable).

## R2 — Hash and canonical encoding

**Decision**: `H` = **SHA-256** (`sha2`, already used by `crypto::MessageHash`) over a **domain-separated, length-prefixed canonical encoding** of `(genesis, topic, requester, candidate, interval)`, reducing the leading 8 bytes modulo `B`. A domain-separation tag keeps the edge predicate's hash domain distinct from `MessageHash`. Explicitly NOT `std::hash::DefaultHasher`.

**Rationale**: FR-002 requires the acceptor's predicate result to equal the dialer's across machines. SHA-256 is a fixed, cross-version-stable algorithm; length-prefixing each variable-width component prevents distinct tuples colliding via concatenation. `DefaultHasher` is unspecified and not stable across platforms/compiler versions — a correctness defect (Principle I). No new dependency: `sha2` is already in tree.

**Alternatives**: a dedicated keyed-hash crate (e.g. `siphasher`) — viable but needs a justified-dependency ADR; unnecessary since SHA-256 is in tree and cross-machine stable. `DefaultHasher` — rejected (non-portable).

## R3 — Rejection handling: drop the pending upstream only (no retry/back-fill in 005)

**Decision**: On an explicit over-capacity `Rejected`, the dialer's **only** action is to remove the matching pending `AwaitingAccept` entry, so it stops waiting for an `Accepted` that will never arrive. There is no failed-peer set, no rejection counter, and no back-fill: the strategy selects straight over the current `candidates` view (no candidates-minus-failed diff), and the trait stays a pure predicate over that view. The realized upstream degree may therefore settle below `target_degree` after rejections; re-forming connections is deferred to a future heartbeat-rotation layer. Retry-to-a-minimum (back-fill) is a **separate future strategy family**, explicitly out of scope for 005.

**Rationale**: the minimal reaction keeps the transition pure and deterministic while avoiding new persistent ordered state. Back-fill semantics (which peers to exclude, when to reset, how to re-select) are policy choices better isolated in their own strategy family than baked into `HashGatedConnection`; deferring them keeps 005 focused on verifiable selection + explicit rejection (FR-009).

**Alternatives**: a sticky failed-set + back-fill driven by re-dialing (the earlier design) — dropped (added persistent state and re-dial policy that belong to a dedicated future strategy family + the heartbeat-rotation layer). A new round/tick event — the `Heartbeat { interval }` seam (R7) is shaped to carry cross-interval rotation later, but v1 fires a single interval and does not re-form.

## R4 — Acceptance seam: verify + reason-bearing decision over current downstream

**Decision**: Evolve `ConnectionAcceptanceStrategy` from `accepts -> bool` to a reason-bearing `admit(emitter, topic, &view) -> Admission { Accept, RejectMembership, RejectIllegitimate, RejectOverCapacity }`, where the read-only node state is grouped into a `NodeView { subscriptions, candidates, downstream, interval }` — the decision reads the current downstream (to count the per-topic cap) and the current interval (to recompute the verifiable predicate) from it. Add `ConnectionAction::Rejected { topic }` (acceptor → dialer). Handler: `Accept` → record downstream + `Accepted` (unchanged); `RejectMembership` → silent drop (`membership_validation_failed`); `RejectIllegitimate` → silent drop (`illegitimate_request`); `RejectOverCapacity` → drop (`downstream_capacity_reached`) + send `Rejected` (no severance). `VerifiableBoundedAcceptance { genesis, self_id, target_degree, cap_buffer }` recomputes `is_valid_edge(genesis, topic, emitter, self_id, interval, B)` and caps downstream-on-topic at `OC = ⌈target_degree + c·√target_degree⌉`; `AcceptFromAllCandidates` maps onto `Accept`/`RejectMembership` only.

**Rationale**: the acceptor must **verify** the request itself (not trust the dialer) — both sides compute the same predicate, so an adversary cannot force an edge the hash disallows. A bare `bool` can't distinguish the three refusals: a membership failure and a predicate failure are silent drops (leaking nothing — a reply would tell a non-legitimate requester a slot exists), while over-capacity of a legitimate request sends an explicit signal (FR-008). The Principle-IV ambiguity (the seam note's "no signature change") is surfaced in ADR 0025/0030. `Rejected` is a normal capacity outcome, not misbehaviour.

**Alternatives**: move the capacity/verify check into the handler (splits the policy across strategy + handler) — rejected. Overload `Terminated` for rejection — rejected (conflates refusal with tearing down a live link). A reply on predicate failure — rejected (leaks slot existence to an adversary).

## R5 — Uniformity validation (SC-003 tolerance)

**Decision**: Validate the predicate's pseudo-uniformity with a seeded loop / `proptest` (already a dev-dependency) over a sweep of ≥1,000 intervals (or genesis values) on a fixed candidate set with `B > 1`; assert per-candidate (or per-interval) selection frequency lies within a tolerance band — the accepted fraction ≈ `1/B`. Set the band from a chi-square goodness-of-fit at a fixed low significance (fail only if p < 0.001, keeping the test non-flaky), or an equivalent ±relative margin from the sweep size.

**Rationale**: quantifies the spec's "within tolerance" into a concrete, reproducible, low-flake check (the sweep is fixed). A hash is pseudo-uniform, so an exact-uniformity assertion would be flaky.

**Alternatives**: exact uniformity — rejected (flaky). Manual inspection — rejected (not a test).

## R6 — Relationship to the determinism/purity refactor (coordination, not a hard dependency)

**Decision**: 005 keeps **ordered structures (`BTreeSet`/`BTreeMap`) on the state it introduces or touches** within this PR (so effect emission is order-stable) and keeps its strategy objects **pure** (genesis / `target_degree` / `c` as construction fields, interval an input). It retains the **current strategy injection** (`Arc<dyn …>` on the node) and does **not** depend on the broader strategies-as-`apply`-arguments relocation; strategies migrate to the argument shape when the co-developing architect's refactor lands.

**Rationale**: the pure strategy objects work identically whether injected or passed as arguments, so 005 need not block on the refactor. Keeping ordered structures on 005's own state is small and keeps it consistent. This avoids a hard cross-workstream gate while the two coordinate on shared-file edits.

**Coordination (not a fallback)**: align ordered-structure type choices and avoid conflicting edits to shared files (`NodeState`, strategy injection sites) with the co-developing architect (tasks T003). No artifact of 005 waits on the refactor merging.

## R7 — ROADMAP alignment

**Decision**: This realises the ROADMAP `005-PeerView`/`006-Epochal-dialer` region using the existing 008 candidate view as the v1 view (no separate `PeerView`/`PeerSource`; no discovery-layer `H_v` sampling yet) and `Event::Heartbeat { interval }` re-invocation as the interval/epochal re-dial (no wall-clock `EpochalPickN { every: Duration }`). `(genesis, interval)` stand in for the model's per-round beacon `nonce_R` (a real unbiasable beacon deferred). v1 fires one interval; periodic heartbeats + rotation/teardown, the push-based golden-node (M2) model, and the edge/golden mode flag remain later features.

**Rationale**: 008 already folds per-topic candidates into the node; a parallel `PeerView` abstraction would duplicate it with no current consumer (Principle I). The logical/explicit `Heartbeat` step satisfies the deterministic-transition rule (driver-fired, no wall-clock) and threads the interval so the rotation layer drops in without reshaping the seam.

## Open items carried to design

- Exact post-refactor `apply` signature + ordered-type choices (R6) — pin with the co-developing architect before implement.
- Self-id / genesis / `target_degree` / `c` delivery to the strategies: baked in at construction (the strategy instance is per node), interval threaded as an argument — confirmed in data-model.
- SC-003 tolerance constant (R5) — fixed at test-authoring time.
- View `H_v` sub-sampling seam (FR-011) — deferred to the discovery/experiment layer; v1 view = full candidate set.

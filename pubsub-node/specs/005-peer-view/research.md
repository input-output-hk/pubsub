# Phase 0 Research: Seeded bounded connection-selection and acceptance strategies

Decisions resolving the Technical Context unknowns. Per item: **Decision / Rationale / Alternatives**.

## R1 — Randomness mechanism: deterministic keyed-hash ranking

**Decision**: Select the bounded upstream set by ranking candidates on a stable keyed hash of `(seed, self_id, topic, candidate_id)` and taking the lowest-k. No RNG is instantiated; selection is a pure function of inputs. The seed is a field of the strategy object (encapsulated randomness, FR-009).

**Rationale**: keeps the transition deterministic and reproducible across runs/machines (FR-003); a single network seed folded with `self_id` gives per-node diversity (FR-005) and uniformity over a seed sweep (FR-007). Matches the epoch-nonce/ring idea from the SecureCyclon work.

**Alternatives**: stateful seeded PRNG in the strategy — rejected (non-deterministic across calls on an evolving candidate set; hidden state). Wall-clock/entropy seed — rejected (not reproducible).

## R2 — Stable digest choice

**Decision**: Use the in-tree `sha2` (`Sha256`, already used by `crypto::MessageHash`) over a length-prefixed canonical encoding of the ranking tuple. Explicitly NOT `std::hash::DefaultHasher`.

**Rationale**: FR-003 requires identical selection across machines; `DefaultHasher` is unspecified and not stable across platforms/compiler versions — a correctness defect (Principle I), not a perf concern. No new dependency.

**Alternatives**: a dedicated keyed-hash crate (e.g. `siphasher`) — viable but needs a justified-dependency ADR; deferred unless the in-tree digest is unsuitable.

## R3 — Failed-peer set: sticky for the run, `ConnectionSetup`-driven back-fill

**Decision**: The node keeps a per-`(peer, topic)` **failed** set (ordered). Before selection, the node hands the strategy the viable view (candidates minus failed). On an explicit over-capacity `Rejected`, the node removes the dead `AwaitingAccept` entry, adds the peer to the failed set (**sticky — never reset within the run**, per Clarifications), and a subsequent `ConnectionSetup` re-invocation re-runs selection — back-fill falls out of recomputation (dropping a failed peer shifts the top-k to the next-ranked). Under-fill is terminal when the viable set is exhausted below the bound.

**Rationale**: simplest deterministic, monotone rule (Clarifications Q1=A); keeps the dial-trait signature stable (the strategy stays a pure top-k over the viable set); `ConnectionSetup` is already the re-triggerable re-dial hook, so no new event/timer is needed (Clarifications Q2 / FR-014).

**Alternatives**: reset failed-set on membership change or per-`ConnectionSetup` — rejected (Clarifications: oscillation, lifecycle coupling). A new round/tick event — rejected (re-dial = `ConnectionSetup` re-invocation).

## R4 — Acceptance seam: reason-bearing decision over current downstream

**Decision**: Evolve `ConnectionAcceptanceStrategy` from `accepts -> bool` to a reason-bearing `admit(...) -> Admission { Accept, RejectMembership, RejectOverCapacity }`, taking the current downstream view so the downstream degree can be enforced. Add `ConnectionAction::Rejected { topic }` (acceptor → dialer). Handler: `Accept` → record downstream + `Accepted` (unchanged); `RejectMembership` → silent drop (today's behaviour); `RejectOverCapacity` → drop with a distinct cause + send `Rejected` (no severance). `BoundedAcceptance { downstream_degree }` implements the cap; `AcceptFromAllCandidates` maps onto `Accept`/`RejectMembership`, never `RejectOverCapacity`.

**Rationale**: a bare `bool` can't distinguish membership failure (silent drop) from capacity rejection (explicit signal, FR-011). The Principle-IV ambiguity (the seam note's "no signature change") is surfaced in ADR 0025. `Rejected` is a normal capacity outcome, not misbehaviour.

**Alternatives**: move the capacity check into the handler (splits the policy across strategy + handler) — rejected. Overload `Terminated` for rejection — rejected (conflates refusal with tearing down a live link).

## R5 — Unbiasedness validation (SC-004 tolerance)

**Decision**: Validate FR-007 with a `proptest` (already a dev-dependency) or seeded loop over ≥1,000 distinct seeds on a fixed candidate set larger than the bound; assert per-candidate selection frequency lies within a tolerance band. Set the band from a chi-square goodness-of-fit at a fixed low significance (fail only if p < 0.001, keeping the test non-flaky), or an equivalent ±relative margin from the sweep size.

**Rationale**: quantifies the spec's "small configurable margin" into a concrete, reproducible, low-flake check (the seed list is fixed). A hash is pseudo-uniform, so an exact-uniformity assertion would be flaky.

**Alternatives**: exact uniformity — rejected (flaky). Manual inspection — rejected (not a test).

## R6 — Relationship to the determinism/purity refactor (coordination, not a hard dependency)

**Decision**: 005 applies **ordered structures (`BTreeSet`/`BTreeMap`) to the state it introduces or touches** within this PR (so its own results are reproducible) and keeps its strategy objects **pure** (seed/bounds as construction fields). It retains the **current strategy injection** (`Arc<dyn …>` on the node) and does **not** depend on the broader strategies-as-`apply`-arguments relocation; strategies migrate to the argument shape when the co-developing architect's refactor lands.

**Rationale**: the seeded/pure strategy objects work identically whether injected or passed as arguments, so 005 need not block on the refactor. Applying ordered structures to 005's own state is small and keeps it consistent. This avoids a hard cross-workstream gate while the two coordinate on shared-file edits.

**Coordination (not a fallback)**: align ordered-structure type choices and avoid conflicting edits to shared files (`NodeState`, strategy injection sites) with the co-developing architect (tasks T003). No artifact of 005 waits on the refactor merging.

## R7 — ROADMAP alignment

**Decision**: This realises the ROADMAP `005-PeerView`/`006-Epochal-dialer` region using the existing 008 candidate view (no separate `PeerView`/`PeerSource`) and `ConnectionSetup` re-invocation as the epochal re-dial (no wall-clock `EpochalPickN { every: Duration }`). The push-based golden-node (M2) model and the edge/golden mode flag remain later features.

**Rationale**: 008 already folds per-topic candidates into the node; a parallel `PeerView` abstraction would duplicate it with no current consumer (Principle I). The logical/explicit `ConnectionSetup` step satisfies the deterministic-transition rule.

## Open items carried to design

- Exact post-refactor `apply` signature + ordered-type choices (R6) — pin with the co-developing architect before implement.
- Self-id delivery to the strategy: baked in at construction (the strategy instance is per node) — confirmed in data-model.
- SC-004 tolerance constant (R5) — fixed at test-authoring time.

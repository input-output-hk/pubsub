# Phase 0 Research: Seeded bounded connection-selection and acceptance strategies

Decisions resolving the Technical Context unknowns. Per item: **Decision / Rationale / Alternatives**.

## R1 — Randomness mechanism: seeded PRNG sampling over canonically-ordered candidates

**Decision**: Select the bounded upstream set by **seeded pseudo-random sampling over a canonically-ordered candidate set**. For each joined topic, take a uniform `upstream_degree`-subset of the candidates via a partial Fisher–Yates shuffle (`rand::seq::SliceRandom::partial_shuffle`) driven by `rand_chacha::ChaCha20Rng`. The PRNG is **re-seeded per call** from `(seed, self_id, topic)`, so no state is carried across calls and selection stays a pure function of inputs. The seed is a field of the strategy object (encapsulated randomness, FR-009).

**Rationale**: determinism now rests on two things — a **fixed PRNG algorithm** (ChaCha20Rng, stable across platforms/versions, unlike `rand`'s `StdRng`) and **canonically ordered inputs** (`BTreeSet`/`BTreeMap`), so the sample is independent of iteration order (FR-003). A single network seed folded with `self_id` gives per-node diversity (FR-005), and partial Fisher–Yates yields a uniform subset over a seed sweep (FR-007). Matches the epoch-nonce/ring idea from the SecureCyclon work.

**Alternatives**: keyed-hash *ranking* of candidates (lowest-k by digest) — rejected (a PRNG shuffle needs no ranking or tie-break and samples uniformly by construction). A stateful seeded PRNG carried across calls — rejected (non-deterministic on an evolving candidate set; hidden state); re-seeding per call keeps the strategy a pure `&self` function. Wall-clock/entropy seed — rejected (not reproducible).

## R2 — PRNG and seed derivation

**Decision**: Reuse the in-tree `rand_chacha::ChaCha20Rng` (already a dependency, used by `crypto::mock`) as the sampler. Its 32-byte seed is derived with `sha2` (`Sha256`, already used by `crypto::MessageHash`) as a **key-derivation step** over a length-prefixed canonical encoding of `(tag, seed, self_id, topic)`; peers are then picked by the PRNG, not ranked by the digest. Explicitly NOT `std::hash::DefaultHasher` as the KDF.

**Rationale**: FR-003 requires identical selection across machines. `ChaCha20Rng` is a fixed, cross-version-stable algorithm (unlike `StdRng`, whose backing generator is unspecified); the SHA-256 KDF gives a well-distributed 32-byte seed from the tuple. `DefaultHasher` is unspecified and not stable across platforms/compiler versions — a correctness defect (Principle I) if used as the KDF. The canonical order comes from the ordered (`BTree`) inputs, not the digest. No new dependency: both `rand`/`rand_chacha` and `sha2` are already in tree.

**Alternatives**: a dedicated keyed-hash crate (e.g. `siphasher`) as the KDF — viable but needs a justified-dependency ADR; deferred unless the in-tree digest is unsuitable.

## R3 — Rejection handling: drop the pending upstream only (no retry/back-fill in 005)

**Decision**: On an explicit over-capacity `Rejected`, the dialer's **only** action is to remove the matching pending `AwaitingAccept` entry, so it stops waiting for an `Accepted` that will never arrive. There is no failed-peer set, no rejection counter, and no back-fill: the strategy selects straight over the current `candidates` view (no candidates-minus-failed diff), and the trait signature stays a pure top-k over that view. The realized upstream degree may therefore settle below target after rejections; re-forming connections is deferred to a future heartbeat/reshuffle layer. Retry-to-a-minimum (back-fill) is a **separate future strategy family** (working names `BackfillingSeededBoundedConnection` / `RetryingSeededBoundedConnection`), explicitly out of scope for 005.

**Rationale**: the minimal reaction keeps the transition pure and deterministic while avoiding new persistent ordered state (FR-017). Back-fill semantics (which peers to exclude, when to reset, how to re-select) are policy choices better isolated in their own strategy family than baked into `SeededBoundedConnection`; deferring them keeps 005 focused on bounded selection + explicit rejection (FR-014).

**Alternatives**: a sticky failed-set + `ConnectionSetup`-driven back-fill (the earlier design) — dropped in the PR-73 simplification (added persistent state and re-dial policy that belong to a dedicated future strategy family). A new round/tick event — rejected (out of scope; a future heartbeat/reshuffle layer owns re-forming).

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

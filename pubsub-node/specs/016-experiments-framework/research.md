# Research — 016-experiments-framework (Phase 0)

Decisions consolidated from the pre-spec design discussion (the experiments
program design ledger), the clarify session, and the plan-input direction.
No `NEEDS CLARIFICATION` markers remained in the Technical Context; entries
below record each decision with rationale and rejected alternatives so the
trace survives the discussion that produced it.

## R1 — Driver model: synchronous wavefront over the pure core

**Decision**: the driver calls `apply` directly on driver-owned `NodeState`s
and processes deliveries in waves (round r's applies produce round r+1's
deliveries); quiescence = a wave yielding no sends. No tokio, channels, or
`InMemoryNetwork` in the measurement path.

**Rationale**: runs the *same* transition function, state, strategy seams,
and message vocabulary as the real node — fidelity is inherited, not
approximated; wall-clock-free exact quiescence; wave index = the
synchronous-round hop count the analytical models also use, so depth is a
topology property and directly comparable. Delivery order cannot change the
receiver set (content-hash dedup + fire-once + static topology ⇒
interleaving-invariant), so the wavefront is a canonical order among
equivalent ones. Realistic timing skew is a named later extension (random
per-edge delays under a discrete-event scheduler) that would refine latency
only.

**Alternatives rejected**: driving real async `Node`s on `InMemoryNetwork`
(non-reproducible interleaving; the readiness gate makes terminal topology
timing-dependent); seeded-random sequential dispatch (destroys the round
unit — depth becomes scheduling noise matching neither the models nor real
time, while adding no realism to set-valued metrics).

## R2 — Determinism is driver-owned: content-keyed wave canonicalisation

**Decision**: each wave's collected sends are stably sorted by a canonical
content-derived key (sender, addressee, message identity) before routing;
all driver-side extraction/tally structures are ordered or index-keyed. The
core's hash-based connection collections are NOT converted in this feature —
delegated to the in-flight connection-link strategies work (spec
Clarifications 2026-07-18).

**Rationale**: byte-determinism must not rest on an invariant owned by
another module; the sort is O(W log W) per wave (invisible next to `apply`
work), keeps the guarantee local, and makes the future core ordering change
output-invariant for recorded experiments (content-derived keys yield the
same order from any input permutation). Avoids colliding with the
connection-link PR that reshapes exactly those collections.

**Alternatives rejected**: converting core collections here (in-flight PR
conflict; cross-PR blocking); relying on arrival order (pre/post-refactor
output drift; non-local guarantee).

## R3 — Parallel execution: `std::thread::scope` worker pool

**Decision**: workers pull run indices from a shared queue and write into a
pre-sized results vector; worker count = the parallelism knob = the
in-flight-population memory bound. Records are written and aggregates folded
in run-index order.

**Rationale**: canonical order by construction; no new dependency; direct
control of peak memory; runs are coarse uniform units, so work-stealing buys
nothing. Float summation is not reorder-stable, so the canonical fold order
is load-bearing for byte-identical aggregates.

**Alternatives rejected**: `rayon` (generality unused; less direct in-flight
control; new dependency).

## R4 — Encoding: `serde_json`, optional, feature-tied (ADR 0034)

**Decision**: add `serde_json` as an optional dependency activated by the
`experiments` feature; record/aggregate types derive `Serialize` and contain
only order-stable containers (`Vec`, `BTreeMap` — never a `HashMap` field).

**Rationale**: hand-rolled JSON escaping/float formatting is exactly where
byte-reproducibility bugs live; ryu float output is deterministic
shortest-form; value-determinism ∘ deterministic-encoding ⇒ byte-identical
artifacts, so the file-level test surface shrinks to one or two anchors.
Justified Dependencies standard satisfied by ADR 0034.

**Alternatives rejected**: hand-rolled writers (relocates the risk into our
own formatting code); non-optional dependency (would compile for all users
of the default build).

## R5 — SCC: hand-rolled **iterative** Kosaraju in its own module

**Decision**: iterative (explicit-stack) Kosaraju over the extracted
digraph; condensation, source/sink components, goodness, and
min-publisher-coverage from the same pass; small replaceable interface in
the `graph` module.

**Rationale**: ~50 auditable lines on structures we build anyway; the same
algorithm family the formal folder's validator cross-checks with; recursion
overflows at N = 20k–100k, hence iterative. Correctness anchored by
scripted-topology tests and the drain-vs-reachability cross-check (SC-003).

**Alternatives rejected**: `petgraph` (graph-representation conversion layer
for one algorithm; dependency where trust concentration matters most);
recursive DFS (stack overflow at scale).

## R6 — Seed derivation: SHA-256 over (master_seed, run_index)

**Decision**: `run_seed = truncate(SHA-256(master_seed || run_index))`;
per-run sub-seeds (key generation, class assignment, churn draw, publisher
choice, sampler seed) derive from the run seed with domain-separating
labels. The rule is recorded in the manifest.

**Rationale**: pre-derived seeds are independent of execution order (the
parallelism prerequisite); `sha2` is already a dependency; domain separation
prevents accidental correlation between draws; the manifest record makes the
sweep self-describing.

**Alternatives rejected**: sequential RNG draws for run seeds (order-
dependent — breaks run-granularity parallelism); splitmix-style ad-hoc
mixing (unlabelled, undocumented rule).

## R7 — Uncertainty: raw counts + Wilson 95%, fixed (clarify session)

**Decision**: probability estimates always carry (successes, total runs);
the reported interval is Wilson score at 95%, closed form, no configuration
knob.

**Rationale**: well-defined nonzero width at p̂ ∈ {0, 1} — the all-good
sample is our common case; the formal folder's ±1σ standard-error convention
degenerates there, and stays derivable from the counts; the documented M2
comparison carries a methodology note on the difference (to raise with the
formal-methods team — Principle IV).

**Alternatives rejected**: ±1σ standard error as the reported field
(zero-width at the extremes); Clopper–Pearson (conservative; needs Beta
quantiles); configurable level (a knob without a consumer).

## R8 — Goodness: SCC reduction, parameterised dispatch, M2-only v1

**Decision**: good ⟺ the extracted up-honest propagation digraph is one SCC
(post-churn primary, pre-churn diagnostic); min publisher-coverage =
(smallest condensation-sink component − 1)/(up-honest − 1); the computation
is parameterised as (propagation digraph, per-publisher seed set) behind a
`DisseminationModel` dispatch that also owns extraction; v1 ships the enum
with the M2 variant only (extraction = downstream out-edges, seeds =
{publisher}).

**Rationale**: "every publisher reaches everyone" is literally strong
connectivity — one O(V+E) pass replaces H per-publisher drains; reach sizes
shrink toward condensation sinks, giving min-coverage from the same pass;
the criterion is the formal folder's own full-coverage definition, so the
comparison compares like with like. The dispatch seam's consumers are named
(program stages; the in-flight publisher-links feature), satisfying the
forward-compatible-interfaces standard.

**Alternatives rejected**: per-publisher BFS/drains (O(H·(V+E)) or H drains);
inferring goodness from the sampled drain (universal vs sampled — a sink is
exposed only by its own drain, ≈ 1/H of samples); hard-coding seeds =
{publisher} (blocks M3's seed-set variant already derived).

## R9 — Determinism test layering (plan-input decision)

**Decision**: workhorse = pure in-memory value equality (`RunRecord`/
`Aggregates` derive `Eq`; run twice and compare; workers 1 vs K and
compare); one focused serialization test pins encoding; one or two
integration tests byte-diff a tiny sweep's files (twice-written + differing
worker count) as the SC-001 artifact-level anchor.

**Rationale**: byte-identity factors as value-determinism ∘ deterministic
encoding; value-level failures self-diagnose (which field differs); two
same-process runs already exercise `HashMap` iteration variation (each map
instance is independently seeded); file-level diffs are the contract's
statement, not the daily idiom.

## R10 — Experiments-only strategies: silent relay + uniform sampler

**Decision**: two strategy instances live in the experiments module (never
protocol CLI kinds): `SilentRelay` (fan-out selecting no targets — the
models' silent worst-case adversary) and `UniformSampler` (dial
min(target_degree, |candidates|) uniformly without replacement, seeded from
the master seed — the formal M2 selection family).

**Rationale**: silence is the dissemination-optimal attack (floor theorem),
so the one shipped adversary is the true worst case; the sampler is needed
because hash-gated selection yields binomial realised degree, not
exactly-RF picks — without it the M2 comparison conflates the selection-
family gap with instrument error. RNG stays out of the protocol's honest
strategies (the D2-5 purity deferral untouched); under the driver's
canonical call order the seeded sampler is deterministic. Degeneracy rule
(min with available) mirrors the hash-gated small-topic connect-to-all
floor (clarify session).

**Alternatives rejected**: a protocol-side bounded dial kind (N-031: "a
bound is meaningless on the dial side" — a cap doesn't select, and protocol
selection must stay verifiable/RNG-free); comparing with hash-gated
selection only (family mismatch becomes an unexplainable deviation).

## R11 — Churn semantics: driver-state mark, post-formation, no events

**Decision**: the seeded churn draw marks honest nodes down after the dial
drain, before publish; down nodes are not stepped, are excluded from
denominators and publisher choice, and remain registered and present in
peers' connection state; no drain follows the draw (nothing is emitted).

**Rationale**: models "formed connections, then failed"; down ≠ unregistered
and v1 has no liveness detection (N-012 untouched), so failure is observable
only as sends into the void (`sent-to-down` tally); the attacker count is
unchanged, capturing churn's amplification of a fixed adversary budget; the
pre-churn SCC pass is near-free and gives paired churn-vs-formation
attribution no separate churn-free sweep can.

## R12 — Single post-churn publish drain per run (default)

**Decision**: one publish drain per run (seeded up-honest publisher);
`publishes-per-run` knob (default 1) repeats the phase with fresh messages;
no pre-churn publish drain.

**Rationale**: across seeds the sampled publisher covers
(topology × publisher) jointly; all-publisher variants come from graph
analytics exactly while relays are all-or-nothing (the pre-churn drain would
re-derive graph-computable numbers at O(E) cost, and is a counterfactual
besides); repeated drains need no state reset (distinct content hashes) and
become the estimator only once probabilistic behaviours make analytics
bounds-only.

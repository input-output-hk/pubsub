# Feature Specification: Deterministic experiments framework

**Feature Branch**: `016-experiments-framework`

**Created**: 2026-07-17

**Status**: Draft

**Input**: User description (verbatim):

> Feature 016 builds the deterministic experiments framework the dissemination
> experiment program (docs/experiments-program.md) runs on: a feature-gated
> `experiments` module inside the pubsub-node crate that drives the pure core
> (`NodeState` + `apply`) directly — no tokio, no channels, no
> `InMemoryNetwork` — so that a whole multi-node run is a reproducible
> function of its configuration and a single master seed. This is the
> 2026-06-30 logbook decision (pure/deterministic framework, no async in the
> measurement path) and the payoff of the pure-transition design: the driver
> runs the same `apply`, `NodeState`, strategy seams, and message vocabulary
> as the real node, replacing only tokio scheduling with a deterministic
> scheduler.
>
> Driver model. The harness owns a population of participants keyed by
> `PeerId` — classed at build time as honest or adversarial, with a later
> seeded churn draw marking some honest nodes down; "up-honest" throughout
> means honest and not down (the participant and churn paragraphs below give
> the details) — and a central round-based wavefront scheduler. Each participant
> holds a `NodeState` (with its injected connection / acceptance / fan-out
> strategy triad); the driver calls `apply` directly, routes each returned
> `Effect::Send { to, message }` to the addressee as the corresponding
> message-received event, and processes deliveries in waves: round r is the
> set of in-flight deliveries; applying them yields the sends that form round
> r+1; a round that produces no new sends is exact quiescence (no polling, no
> sleeps). The scheduler is message-kind-agnostic — connection control and
> dissemination payloads route identically — so connection handshakes run
> through the same machinery; the metrics never inspect kinds either,
> attributing traffic by phase instead (everything in the dial drain is
> handshake cost, everything in the publish drain is dissemination cost —
> separate drains on a settled topology never mix the two). `Effect::Misbehaved` is consumed by the driver
> (the severance already happened inside `apply`) and tallied as an optional
> metric. Round-based processing makes the headline metrics order-independent:
> coverage because a full drain fixes the set of receivers, and hop distance
> because a node's first-receipt round equals its BFS depth from the publisher
> in the established topology. Delivery order cannot change what is delivered
> — with content-hash dedup, fire-once relaying, and a topology static during
> the drain, the receiver set is invariant under any interleaving — so the
> wavefront is a canonical order among equivalent ones, chosen because its
> rounds give depth a well-defined unit: the synchronous-round hop count the
> analytical models also use (their hops columns share the fire-once,
> equal-hop-time convention), a topology property rather than a wall-clock
> latency claim. Realistic timing skew (a wave-X receiver relaying after a
> wave-X+1 one) would refine latency only, not the set-valued metrics, and
> its principled form — random per-edge delays under a discrete-event
> scheduler — is a named later driver extension, out of scope here.
>
> Run phases, each a separate drain of the one scheduler:
> (1) Registration/sync — populate each node's subscriptions / candidates /
> registered topics, either faithfully (feed membership, topic-registry, and
> Synced events through `apply`, exercising the real fold logic) or by direct
> state pre-population (fast path for large-N sweeps); both modes supported.
> In the faithful mode the driver delivers every registry fold before any
> Synced, and injects all Synced events as one wave: each node's rising-edge
> Heartbeat then fires against the full membership, and — because inbound
> Requests before Synced are dropped (`not_synced`, ADR 0031's readiness gate,
> with no retry/back-fill in v1) — every Request lands only after all nodes
> are synced. The all-synced barrier is free by construction; no `dial_on_sync`
> config flag is needed (N-020 stays open, untouched).
> (2) Dial — `Event::Heartbeat` dials against the epoch nonce already on
> state (the genesis value; in the faithful mode the Synced rising edge fires
> the first Heartbeat, the fast path injects Heartbeat directly). Drain the
> Request/Accepted handshake waves to topology quiescence, establishing
> upstream/downstream. ADR 0031's two-phase advance-then-dial sequence
> (`Event::Epoch { nonce }` folds the new randomness context effect-free,
> then Heartbeat re-dials) is the seam multi-epoch runs will use; v1 runs
> never advance the nonce.
> (3) Publish — a publisher chosen by the master seed (from the up-honest
> nodes) receives a publish event; drain the dissemination waves to quiescence.
> (4) Measure — read metrics directly from the driver-owned states.
> Run anatomy and cost: one run = one registration pass (event folds or
> direct pre-population), one dial drain (O(E) messages), the churn
> draw, the SCC passes (two when churn > 0, one otherwise — O(V+E), negligible
> next to a drain), and one publish drain by default. One publish per seed
> suffices because across seeds the seeded publisher samples
> (topology × publisher) jointly, and the all-publisher variants of the
> metrics come from graph analytics rather than repeated drains while relays
> are all-or-nothing. A publishes-per-run knob (default 1) may repeat
> phase 3 with fresh messages — distinct content hashes mean no state reset —
> which becomes the estimator once probabilistic behaviours make graph
> analytics bounds-only.
> Single-epoch only: one epoch nonce, one dial phase, then publish. The
> `Epoch` event exists and folds, but advancing it without connection
> teardown only grows the mesh (add-only dialing, N-011), so multi-epoch
> runs wait for the rotation feature (the experiment program's multi-round
> prerequisite); repeated Heartbeats within the epoch (the retry primitive)
> are permitted and idempotent at quiescence.
>
> Participants. Two classes in this feature: honest nodes and Level-1
> adversaries — the same `NodeState`/`apply` with an adversarial strategy
> bundle (protocol-compliant behaviour, hostile policy). The driver routes
> transitions uniformly and never branches on class when delivering events or
> collecting sends; the class partition exists only for metrics (adversaries
> are excluded from coverage denominators). Two strategy instances ship with
> the framework, both experiments-only (feature-gated with the module, never
> protocol CLI kinds): a silent relay (a fan-out strategy that selects no
> targets — the analytical models' silent worst-case adversary), enough to
> exercise the Level-1 seam and the cause-decomposition metric end to end;
> and a uniform sampler dial strategy (dial exactly target_degree candidates
> sampled uniformly, seeded from the master seed) — the formal M2 model's
> exact selection family, needed for the comparison demonstration since the
> protocol's hash-gated selection yields binomial realised degree, not
> exactly-RF picks. The sampler's seed makes it deterministic under the
> driver's canonical call order; the protocol's own honest strategies stay
> hash-derived and RNG-free, so the D2-5 purity deferral is untouched. Level-2 protocol-violating
> participants (free wire behaviour bypassing `apply`) and coordinated-attacker
> shared state are planned by the experiment program's adversary stages but out
> of scope here; the participant storage must not preclude adding them.
>
> Honest churn is a first-class simulation parameter, distinct from the
> adversary class: a seeded draw removes a configured number or proportion of
> honest nodes ("down") after topology formation and before publish — modelling
> nodes that formed connections and then failed. Going down generates no
> events and needs no drain: down nodes stay registered and stay in their
> peers' connection state (down is not unregistered, and v1 has no liveness
> detection — dead links are discovered only by sends into the void, which
> the sent-to-down tally counts); churn is a driver-state mark, not a
> protocol action. Down nodes neither relay nor
> count as eligible receivers (metric denominators use up-honest nodes; the
> publisher is drawn from up-honest nodes), while the attacker count k is
> unchanged — capturing that churn amplifies a fixed attacker budget's
> effective power. Graph analytics run on the up-honest subgraph; the pre-churn
> pass is also recorded (an extra SCC pass is negligible) so each run's failure
> attributes to churn vs formation, as a paired per-topology comparison a
> separate churn-free sweep cannot give.
>
> Measurement uses two instruments. The publish-drain (inject a publish, drain,
> observe) is the general one — it works under any strategy mix. The second is
> realised-graph analytics: metrics computed directly from the established
> topology with no dissemination drain, exact whenever the installed fan-outs
> are deterministic all-or-nothing (honest ForwardToAll; silent adversaries =
> dead out-edges), with the publish-drain as cross-check. When probabilistic
> attacker fan-outs are later installed, honest-subgraph analytics remain valid
> as conservative bounds: any forwarding by an attacker only adds deliveries
> over the silent worst case (dedup makes extra copies harmless), so
> honest-subgraph coverage and goodness are floors and depth a ceiling.
> Formation-time attacker influence (occupied pull slots, filled caps) is
> already captured because the analytics read the realised graph.
>
> Publish-drain metrics (delivery completeness is the phase-2 headline):
> - Coverage: the fraction of eligible receivers — up-honest subscribed nodes
>   excluding the publisher — whose received set contains the published
>   message (by content hash). The publisher's local record is not a trial:
>   every denominator element is a node that had to be reached over the
>   network, matching M2's per-target framing (the source is never an eclipse
>   target). Comparisons against reach-set-style closed forms that count the
>   source convert by an exact affine map in the analysis layer.
> - Propagation depth: per-node first-receipt round, observed by the driver
>   (the core records origin but not time); the maximum (longest hop path)
>   equals rounds-to-quiescence; the per-node distribution is retained, not
>   just the max.
> - Non-receiver cause decomposition: for each honest subscriber that missed
>   the message, classify why from the realised topology (all upstream sources
>   adversarial or down, no upstream at all, or no up-honest path from the
>   publisher) —
>   the mechanism, not just the rate. Computed from driver-owned state, never
>   from log output (logs are operator UX, not a measurement surface).
> - Message cost: total dissemination sends during the drain (the driver counts
>   the Send effects it routes, split by recipient class — honest /
>   adversarial / down — so the formal models' honest-to-honest counting
>   convention maps directly) and suppressed arrivals (deliveries whose hash
>   the recipient had already seen — driver-side accounting from its own
>   first-receipt records; suppression emits no effect and drops are log-only).
>   The identity sends = first receipts + suppressed + sent-to-down is asserted
>   per run as a consistency check. Waves are order-independent (BFS depth);
>   sends/suppressed are order-sensitive only through within-wave
>   split-horizon ties on mutual edges — a bounded correction, deterministic
>   under the driver's canonical order.
>
> Realised-graph analytics:
> - Topology shape: in-degree and out-degree distributions (realised degree vs
>   target_degree), and the honest-sink count (out-degree 0 in the extracted
>   digraph; |downstream| = 0 under M2) — sink statistics quantify the
>   ignition risk that M3's standing initiation links buy out, and calibrate
>   its s parameter.
> - Good-topology: a topology is good iff every up-honest publisher would reach
>   every up-honest node — equivalently, the up-honest relay digraph is one
>   strongly connected component (one SCC pass, no drains). Recorded per run
>   post-churn (primary) and pre-churn (diagnostic), each as the
>   boolean plus its graded refinements: min coverage over all up-honest
>   publishers (also free from the same pass — under the excluded-publisher
>   convention, (smallest condensation-sink component − 1) / (up-honest − 1),
>   so a sink publisher reads 0) and the fraction of publishers achieving full
>   delivery (the one genuinely heavier metric — per-component reach sizes on
>   the condensation — so opt-in at very large N). Goodness is universal over
>   publishers while a run's drain samples one, so good = false alongside a
>   fully-delivering drain is expected, not inconsistent (an eclipsed
>   publisher's own drain succeeds — eclipse hurts reception, not emission; a
>   sink is exposed only by its own drain, ≈ 1/H of samples) — which is why
>   goodness is computed from the graph, never inferred from the sampled
>   drain. The goodness computation is parameterised as (propagation digraph,
>   per-publisher seed set) rather than hard-coding seeds = {publisher}: this
>   feature instantiates the M2 shape (seed = the publisher; good = one SCC),
>   and the later dissemination models reuse the same pass — M3's standing
>   initiation links make the seed set {publisher} ∪ its honest initiation
>   targets over the relay-only digraph (the publisher is itself an initial
>   holder: at the publish record point it sends over both link kinds, since
>   content-hash dedup forecloses any later re-forward of a returning copy;
>   good ⟺ every publisher's seeds hit every source component of the relay
>   condensation; formal criterion shared with
>   formal_spec/hybrid_dissemination/models/, whose validator already
>   cross-checks via Kosaraju). The dissemination model is therefore an
>   explicit experiment parameter — named in the config, recorded in the
>   manifest, dispatched on by the goodness/metrics module — so a results
>   file states which model's goodness its good column means. The dispatch
>   owns the propagation-graph extraction from node state, not just the
>   goodness rule: which link records become directed propagation edges is
>   model-specific (M3 excludes its publish-only seed links from the relay
>   digraph; M4's single accepted link yields both directed edges — one dial,
>   no reverse dial; M5's out-links are ordinary relay edges, unioned in), and
>   the graph-derived metrics — degree/sink statistics, miss-cause
>   classification — are defined over the extracted propagation digraph, not
>   over the raw upstream/downstream fields. v1 ships the dispatch structure
>   with only the M2 implementation (extraction is simply downstream =
>   out-edges, seeds = the publisher); each later model adds its variant when
>   its protocol feature lands, and how node state represents the new link
>   kinds (e.g. a seed-links field, empty under M2/M4) is that feature's
>   decision — the framework requires only that extraction be well-defined.
>
> Aggregates: across an experiment's R runs (and, along a sweep's axes, per
> experiment) the harness reports distributions and percentiles,
> not just means — coverage histogram, missed-count distribution, depth
> histogram, message-cost histograms and means (sends, suppressed/duplication
> ratio), P(full coverage), P(good topology) with a binomial confidence
> interval, and histograms of the graded good-topology refinements.
> For very large populations a reduced-recording mode may keep only
> first-receipt round + seen-hash per node instead of full delivery records
> (material only when a publishes-per-run above 1 multiplies the per-node
> records).
>
> Reproducibility. The honest topology is already a deterministic function of
> (genesis epoch nonce, identity keys, membership, bucket count) via 005's
> hash-gated selection — no honest-side RNG exists. One master seed derives
> the remaining randomness in a run — identity-key generation (via the seeded
> mock crypto scheme), class assignment, publisher choice, and per-participant
> strategy seeds where a strategy takes one (the experiments-only uniform
> sampler does; future attacker strategies will) — so any run can be
> replayed exactly (replay the exact run that lost coverage and trace it).
> Whole-run determinism requires one core change, in scope: the two iterated
> core collections still hash-based become ordered — upstream
> (HashMap -> BTreeMap) and downstream (HashSet -> BTreeSet); both are walked
> inside `apply` (fan-out, heartbeat, shutdown, admit_prelude), so their
> per-process-random iteration order currently leaks into effect order.
> subscriptions and candidates are already ordered and PeerId already has Ord;
> lookup-only collections (registered_topics, seen) stay hashed, their
> iteration order being unobservable. Behavior-preserving for the node
> (snapshot getters already sort). The node's strategy seams, injection shape
> (constructor-injected, per 005), and public API are otherwise unchanged.
>
> Execution structure and vocabulary. Three levels: a run (one seed — one
> topology, one publish), an experiment (R runs at one fixed parameter set),
> and a sweep (a set of experiments serving one question, e.g. one curve of
> metric vs parameter; a single-experiment sweep is the everyday case). The
> run is a pure function (params, seed) -> run record — no I/O, no shared
> state — mirroring the node's own pure-core design; the experiment is a
> parallel map over its R seeds plus an order-canonical fold; the sweep
> flattens all (experiment, seed) pairs into one run-granularity work pool
> (so a heavy experiment spreads across all workers) and owns all I/O.
> Parallel execution must not perturb outputs: run seeds are pre-derived
> (run_seed = H(master_seed, run_index), so seeds are independent of
> execution order), records are written in canonical run-index order, and
> aggregation folds in that same order (float summation is not
> reorder-stable) — same master seed, same files, at any worker count. A
> parallelism-degree knob bounds peak memory (each in-flight run holds a
> population of node states).
>
> Output contract (three artifacts per sweep; data only, no plotting):
> (1) the sweep manifest — tool commit, master seed and the seed-derivation
> rule, fixed parameters, axes, and the expanded experiment list that run
> records reference by index; (2) run records — one JSONL row per run: seed,
> population as drawn, dial-phase tallies, graph-analytic results (good
> pre/post churn, min publisher-coverage, sinks, SCC shape, degree
> histograms), and publish-drain results (coverage, received/missed, depth
> plus its in-run histogram, miss-cause counts, sends/suppressed/sent-to-down)
> — scalars and degree/depth-bounded vectors only, nothing O(N); (3) the
> aggregates file — per experiment: cross-run histograms, means/percentiles,
> and P(good) with a binomial confidence interval. Invariant: the aggregates
> file is a pure function of the run records, so external tooling can
> recompute and diff it; in-harness aggregation is convenience, never the
> sole holder of information. Opt-in per-run detail (the O(N) per-node table:
> first-receipt wave, first-delivery origin, degrees, miss cause, class) is
> off by default and regenerable exactly from the run's seed.
>
> Placement and config. The experiments module is feature-gated (an
> `experiments` cargo feature, off by default) because the driver needs the
> crate-internal `apply` / `NodeState` / `Effect` surface; nothing new is
> exported from the library's public API. Full-word naming throughout — the
> module, cargo feature, and front-end binary are named `experiments`, no
> `sim` abbreviation. A thin front-end binary/example parses sweep
> configuration at the edge (dissemination model (v1: M2 only), population
> size, class counts, honest-churn count or proportion, topic, strategy
> parameters, master seed, axes, runs-per-experiment, publishes-per-run,
> parallelism degree, detail flags); the experiments API itself takes
> already-parsed values
> (parse-at-the-edge). v1 experiments run on a single topic and the population
> is its membership — a scope statement, not a design constraint (the core and
> driver are topic-agnostic); multi-topic configurations arrive with the
> program's heterogeneity experiments.
>
> Framework validation (this feature's acceptance is the instrument, not the
> science):
> - Known-topology checks: the harness can build scripted topologies (line,
>   star, full mesh) whose coverage and depth are hand-computable, and the
>   measured metrics must match exactly (full mesh with all-honest
>   ForwardToAll: 100% coverage, depth 1).
> - Determinism: same configuration + master seed produces identical metrics
>   and terminal states across repeated runs and process restarts.
> - Silent-adversary demo: with silent relays present, coverage and the cause
>   decomposition reflect the severed paths on a scripted topology with a
>   hand-computable answer.
> - Two-instrument cross-check: on validation runs, publish-drain coverage
>   equals graph-analytic reachability, and the per-run accounting identity
>   (sends = first receipts + suppressed + sent-to-down) holds — two
>   independent computations of the same quantities agreeing in-process.
> - M2-comparison demonstration (informs, does not gate): a worked example
>   configuration running the experiments-only uniform sampler (the formal
>   model's exact selection family) with accept-from-all and forward-to-all
>   at the formal M2 operating point (models/comparison.md: N = 20 000,
>   mu = 0.2, RF = 24). The full-size point is executed manually and its
>   results documented (it costs real wall-clock time, so it never enters the
>   per-commit test suite); a suite-sized smoke variant of the same config —
>   tiny N, a handful of runs, seconds — runs in the automated tests and
>   asserts pipeline health only (config parses, sweep executes, artifacts
>   well-formed, identities and determinism hold), never numeric agreement.
>   The manual run is compared against the formal simulators' published
>   values: message counts, copies per honest
>   node, and hop depth at the operating point (means — cheap, their cells
>   used 40-200 graphs); P(good) at a bulk-regime point (P(bad) ~ 1e-2..1e-3,
>   where R ~ 1e4 runs measures it — the operating point's P(bad) ~ 7e-5
>   would need ~1e6 runs of a 20k-node simulation, the same 1/p wall the
>   formal Monte-Carlo faces; the analytical law owns that tail). Counting
>   conventions map via the recipient-class sends split. Demonstrates end to
>   end that the M2 model class is representable and the whole pipeline runs;
>   deviations are recorded and explained, not pass/fail — the deterministic
>   checks above remain the acceptance gates. (Re-running the same point
>   hash-gated to quantify the bucketing family gap is experiment territory,
>   not part of the demonstration.)
> These internal anchors carry the driving-fidelity burden deliberately: the
> team's independent M2-style simulations and the closed forms validate the
> statistical layer, but agreement with an idealisation cannot certify that
> the harness measured the prototype's actual protocol — especially since the
> program's purpose is to find deviations from the analytics. No differential
> test against the real async shell is included: the protocol logic lives
> inside `apply`, which the driver calls literally, and the shell's terminal
> topology is timing-dependent under the readiness gate (pre-sync dials are
> dropped without retry), so such a test buys little and needs orchestration;
> it can be added later if a concrete driving-fidelity doubt arises.
>
> Dependency: 005-peer-view (PR #73) is merged substrate — the driver feeds
> its Epoch/Heartbeat vocabulary and its hash-gated / bounded strategies are
> what Stage-1..3 experiment configurations install. The framework's own
> acceptance additionally validates on full-mesh and scripted topologies,
> which need nothing from 005.
>
> Out of scope: the experiments themselves (E1–E13 — the single worked
> M2-comparison example above demonstrates the instrument, it is not the
> program); multi-epoch runs and
> connection rotation/teardown (N-011/N-012 untouched — Epoch advancement
> without teardown is documented, not exercised); epoch-nonce beacon/agreement
> (N-030, deferred to the chain-anchored beacon layer); the golden push tier
> and M3's standing initiation links (s-1 publish-only seeding links per node,
> never relayed over — formal_spec/hybrid_dissemination/models/m3; both ride
> the same role-inverted push-connection primitive, a later feature whose
> messages the kind-agnostic scheduler will route unchanged, and whose
> publish-only fan-out variant is a strategy concern, not a scheduler
> concern); Level-2 adversaries, attacker
> coordination state, and seeded-stateful attacker strategies;
> deposit/slashing; multi-topic experiment configurations (the Stage-4
> heterogeneity experiments); any real network transport, persistence, or
> wall-clock time (rounds are the only time unit).
>
> References to read before specifying:
> - docs/experiments-program.md (the consumer: stages, experiments, metrics).
> - ../formal_spec/hybrid_dissemination/models/ (READ-ONLY, Principle V): the
>   model family M1-M5, the shared full-coverage ("good graph") criterion in
>   its README, and comparison.md (M3 = the bandwidth-optimal target model).
> - ../logbook.md (repo root), 2026-06-30 entry — the team decisions this
>   feature realises: pure/deterministic framework (no async in the
>   measurement path) and delivery completeness as the phase-2 headline
>   metric.
> - specs/005-peer-view/ + ADR 0030 (verifiable edge predicate) + ADR 0031
>   (epoch/round split, readiness gate — the dial vocabulary and barrier
>   semantics the driver relies on).
> - docs/decisions/0018-connection-selection-strategy-seam.md,
>   0021-fanout-strategy-seam-dedup-and-message-origin.md,
>   0023-connection-acceptance-strategy-seam.md (the three seams the
>   strategy bundles plug into).
> - docs/decisions/0020-cross-registry-consistency-and-readiness.md (Synced
>   readiness + rising-edge dial the registration phase reuses).
> - specs/event-loop-and-registry-contract.md (event vocabulary the driver
>   feeds).
> - specs/IMPLEMENTATION_NOTES.md N-011/N-012 (add-only connections — why
>   single-epoch), N-020 (Synced-triggers-dial coupling — deliberately
>   untouched), N-028 (005's deferral catalogue — its experiment/testing-
>   framework bullet is the deferral this feature resolves; its determinism-
>   refactor bullet relates to the deferred strategies-as-apply-arguments
>   question), N-031 (decomposed acceptance baselines — why the dial seam has
>   no bounded kind; the uniform sampler here is experiments-only).

## Context

The prototype's core build is done; phase 2 pivots from building the node to
**using it as an instrument** — running the dissemination experiment program
(`docs/experiments-program.md`, Stages 1–6 / E1–E13) to measure delivery
completeness, propagation depth, dissemination cost, and topology health, and
to compare them against the team's analytical models
(`../formal_spec/hybrid_dissemination/models/`, M1–M5). This feature builds
that instrument: an in-crate, feature-gated `experiments` module that drives
the node's own pure core deterministically, plus the thin front end, metrics,
and output pipeline the program needs. It resolves the experiment/testing-
framework deferral recorded in N-028 and realises the 2026-06-30 logbook
decisions (pure/deterministic framework; delivery completeness as the phase-2
headline metric).

The feature is the instrument, not the science: the experiments themselves
stay out of scope, and the single worked M2-comparison example exists to
demonstrate the instrument end to end, not to run the program. Scope
boundaries, dependencies, and the full design rationale are carried in the
Input above; the requirements below normatively restate them.

## Clarifications

### Session 2026-07-17

- Q: What happens when a long sweep is interrupted (Ctrl-C, crash)? → A: No
  resume in v1 — interrupted sweeps are re-run from scratch (determinism
  makes that cheap and correct); records stream in canonical order, so an
  interrupted run-records file is a readable prefix, but carries no
  completion claim.
- Q: Does the M2-comparison worked example ship the bulk-regime P(good)
  configuration too, or only the operating point? → A: Ship both — the
  operating point (cost/latency means) and a named bulk-regime point from
  m2's full-coverage validation grid (the P(good)-vs-law check), so the
  documented comparison is fully reproducible from shipped configurations.
- Q: Uniform sampler when target_degree ≥ available candidates? → A: Degrade
  gracefully — sample min(target_degree, |candidates|), dialing all
  candidates when fewer (mirrors the hash-gated small-topic connect-to-all
  degeneracy); experiment scenarios are not expected to hit it, but the
  smoke variant is well-defined without scaled-down parameters.
- Q: Which uncertainty convention for P(good)? → A: Raw counts (good runs,
  total runs) always, plus a Wilson score interval at 95% as the reported
  uncertainty — fixed convention, no knob. The formal models folder reports
  plain ±1σ binomial standard errors, which degenerate to zero width at
  all-good samples (our common case); their convention stays derivable from
  the counts, and the documented M2 comparison carries a methodology note on
  this difference to raise with the formal-methods team.

### Session 2026-07-18

- Q: Does 016 still perform the core ordered-collection conversion
  (upstream/downstream to ordered types)? → A: No — removed from this
  feature. The in-flight connection-link strategies work (PR #77 family)
  refactors exactly those link-record collections and incorporates the
  ordering there (coordination note to that PR). 016 stays deterministic
  without it: the driver canonicalises each wave's collected sends before
  routing and builds its extraction structures in sorted form, so byte-
  identical outputs do not depend on the core's iteration order. 016 now
  touches the node core only for crate-internal read/construction access.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Run one reproducible experiment and read its delivery metrics (Priority: P1)

A protocol researcher configures a single experiment — dissemination model
(M2), population size, adversary count, churn, strategy parameters, topic,
runs-per-experiment R, and a master seed — runs it from the front end, and
receives the three output artifacts: a manifest describing exactly what ran,
one record per run with the delivery metrics (coverage, depth, miss causes,
message cost) and topology metrics (degrees, sinks, good-topology verdicts),
and an aggregates file with distributions, percentiles, and P(good) with a
confidence interval. Running the same configuration and master seed again —
on the same code — reproduces the same artifacts byte for byte.

**Why this priority**: this is the framework's reason to exist — without a
single reproducible, measurable run there is no instrument. Every other story
builds on it.

**Independent Test**: configure a small experiment (e.g. N = 100, a few
silent adversaries, churn > 0, R = 20), run it twice with the same master
seed, and verify (a) the three artifacts are produced and well-formed,
(b) the two executions' artifacts are identical, (c) the per-run accounting
identity holds on every run.

**Acceptance Scenarios**:

1. **Given** a valid experiment configuration and master seed, **When** the
   front end runs it, **Then** the sweep manifest, run records, and
   aggregates file are produced, and every run record contains the seed,
   population as drawn, dial-phase tallies, graph-analytic results (pre- and
   post-churn when churn > 0), and publish-drain results.
2. **Given** the same configuration and master seed, **When** the experiment
   is executed twice (including across process restarts), **Then** the output
   artifacts are byte-identical.
3. **Given** any completed run, **When** its record is inspected, **Then**
   sends = first receipts + suppressed + sent-to-down holds, and the record
   contains only scalars and degree/depth-bounded vectors (nothing sized by
   the population).
4. **Given** a configuration with churn > 0, **When** a run completes,
   **Then** coverage and goodness denominators count only up-honest nodes
   excluding the publisher, the publisher is an up-honest node, and both
   pre-churn and post-churn goodness verdicts are recorded.

---

### User Story 2 - Sweep a parameter to produce a curve (Priority: P2)

The researcher configures a sweep — one or more axes (e.g. population size,
adversary fraction, churn) over a fixed base configuration — and runs it. The
sweep executes all (experiment, seed) pairs, in parallel across workers,
and emits per-experiment aggregates keyed to the manifest's experiment list,
so each point of the metric-vs-parameter curve is one experiment's
distribution summary. Parallelism never changes the output.

**Why this priority**: the program's questions are mostly curves (coverage vs
N, tolerance vs adversary count); the sweep is the unit of a figure, and
same-manifest provenance is what makes a curve's points comparable.

**Independent Test**: run a two-axis sweep at small scale with worker counts
1 and 8; verify per-experiment aggregates exist for every grid point, rows
reference experiments by index, and the two executions' artifacts are
identical.

**Acceptance Scenarios**:

1. **Given** a sweep with A × B grid points and R runs each, **When** it
   completes, **Then** the manifest lists A × B experiments, the run-records
   file has A × B × R rows in canonical run-index order, and the aggregates
   file has one entry per experiment.
2. **Given** the same sweep configuration and master seed, **When** executed
   with different parallelism degrees, **Then** all three artifacts are
   byte-identical.
3. **Given** an experiment whose runs all produce good topologies, **When**
   aggregates are computed, **Then** P(good) is reported with a binomial
   confidence interval (not as a bare 1.0).

---

### User Story 3 - Replay and dissect a pathological run (Priority: P3)

A sweep shows one run with unexpectedly low coverage. The researcher takes
that run's seed from its record, replays exactly that run with per-run detail
enabled, and receives the per-node table (first-receipt wave, first-delivery
origin, degrees, miss cause, class) to trace which cluster missed the message
and why — e.g. the miss-cause decomposition shows nodes whose upstream
sources were all adversarial or down.

**Why this priority**: determinism's payoff is the dissection workflow;
without replay-by-seed, outliers in a sweep are anecdotes rather than
evidence.

**Independent Test**: run an experiment containing at least one non-full-
coverage run (e.g. with silent adversaries on a sparse scripted topology);
replay that run's seed with detail on; verify the replayed record equals the
original and the per-node table's miss causes are consistent with the
recorded topology.

**Acceptance Scenarios**:

1. **Given** a run record from a previous sweep and the same code, **When**
   its seed is replayed with per-run detail enabled, **Then** the run record
   fields are identical to the original and the per-node table is emitted.
2. **Given** a replayed run with misses, **When** the per-node table is
   inspected, **Then** every missed up-honest node carries a cause consistent
   with the realised topology (all-upstreams-adversarial-or-down,
   no-upstream, or no-up-honest-path).

---

### User Story 4 - M2-comparison demonstration (Priority: P4)

A researcher runs the shipped worked-example configuration — the
experiments-only uniform sampler with accept-from-all and forward-to-relays
at the formal M2 operating point (N = 20 000, μ = 0.2, RF = 24) — manually, and
documents the measured message counts, copies per honest node, and hop depth
alongside the formal simulators' published values, plus P(good) at the
shipped bulk-regime parameter point (both comparison configurations ship
with the example). A tiny smoke variant of the same configuration
runs inside the automated test suite and asserts pipeline health only.

**Why this priority**: it demonstrates the M2 model class is representable
end to end and anchors the instrument against the team's independent
simulations — but it informs rather than gates, so it follows the three
stories that define the instrument itself.

**Independent Test**: execute the smoke variant (seconds); verify config
parses, the sweep executes, artifacts are well-formed, and identities and
determinism hold. Execute the full-size point manually once; verify the
comparison table can be filled from the artifacts.

**Acceptance Scenarios**:

1. **Given** the worked example configuration at full size, **When** run
   manually, **Then** the artifacts contain the quantities the comparison
   needs (honest-to-honest sends via the recipient-class split, copies per
   honest node, depth distribution), and the documented comparison records
   agreement or explained deviation — it is not a pass/fail gate.
2. **Given** the smoke variant, **When** the automated suite runs, **Then**
   it completes in seconds and asserts pipeline health only, never numeric
   agreement with the formal model.

---

### Edge Cases

- **Not-good topology with a fully delivering drain**: expected, not a
  contradiction — goodness quantifies over all publishers, the drain samples
  one (an eclipsed publisher's own drain succeeds; a sink is exposed only by
  its own drain). Both results are recorded as-is.
- **Sink publisher sampled**: the drain delivers to nobody; coverage reads 0
  under the excluded-publisher convention; min publisher-coverage reads 0;
  quiescence is immediate.
- **Churn draw larger than the honest population, or leaving zero up-honest
  nodes**: rejected at configuration validation (no publisher can be drawn).
- **All-adversarial or single-node populations**: rejected at configuration
  validation (no eligible receivers — denominators would be empty).
- **Churn = 0**: one SCC pass only; pre-churn and post-churn verdicts
  coincide and are recorded once.
- **All R runs good**: P(good) is still reported with its confidence interval
  (an all-good sample of R runs certifies only P(bad) ≲ 3/R).
- **publishes-per-run > 1**: each publish uses a fresh message (distinct
  content hash), so no state reset is needed; per-publish metrics are
  recorded per drain.
- **Opt-in heavy metric off**: the full-delivery publisher fraction is
  absent from the record (not zero, not null-as-value) when not computed.
- **`experiments` feature disabled**: the library's public API, behaviour,
  and test suite are unchanged; nothing of the framework is compiled in.
- **Repeated Heartbeats within the dial phase**: idempotent at quiescence —
  re-dialing the same expected set adds nothing once established.
- **Interrupted sweep**: no resume in v1 — the partial artifacts are a valid
  canonical-order prefix with no completion claim; the sweep is re-run from
  scratch (same master seed reproduces it exactly).

## Requirements *(mandatory)*

### Functional Requirements

**Placement & gating**

- **FR-001**: The framework MUST live in an `experiments` module inside the
  pubsub-node crate, compiled only under an `experiments` cargo feature that
  is off by default; with the feature disabled, the library's public API,
  behaviour, and non-ignored test results MUST be unchanged.
- **FR-002**: Public naming for the module, cargo feature, and front-end
  binary MUST use the full word `experiments` (no `sim` abbreviation).
- **FR-003**: The framework MUST drive the crate's real pure core — the same
  state-transition function, node state, strategy seams, and message
  vocabulary the node uses — with no async runtime, channels, or in-memory
  network in the measurement path.

**Driver & scheduler**

- **FR-004**: The driver MUST own a population of participants keyed by peer
  id, classed at build time as honest or adversarial, and MUST route every
  participant's transitions uniformly — never branching on class when
  delivering events or collecting sends.
- **FR-005**: The driver MUST process deliveries as a round-based wavefront:
  round r is the set of in-flight deliveries; applying them yields the sends
  forming round r+1; a round producing no new sends is quiescence, detected
  exactly (no polling, no sleeps, no timeouts).
- **FR-006**: The scheduler MUST route all message kinds identically
  (connection control and dissemination payloads), so handshakes run through
  the same machinery; severance effects are consumed by the driver and
  tallied.
- **FR-007**: All driver-side iteration and delivery MUST follow a canonical
  deterministic order, so that a whole run is a deterministic function of
  (configuration, master seed) — including the within-wave tie-breaks that
  bound the order-sensitivity of the send/suppressed tallies.

**Run phases**

- **FR-008**: The registration phase MUST support both setup modes: faithful
  (feeding membership, topic-registry, and readiness events through the real
  fold logic) and direct state pre-population (fast path); in the faithful
  mode all registry folds MUST be delivered before any readiness event and
  all readiness events MUST be injected as one wave, so every dial lands
  after all nodes are synced.
- **FR-009**: The dial phase MUST establish the topology by draining the
  handshake waves to quiescence, dialing against the epoch nonce already on
  state (the genesis value); v1 runs MUST NOT advance the epoch nonce
  (single-epoch runs).
- **FR-010**: The publish phase MUST inject a publish at one publisher drawn
  by the master seed from the up-honest nodes and drain the dissemination
  waves to quiescence; a publishes-per-run parameter (default 1) MAY repeat
  the publish phase with fresh messages, without any state reset.

**Participants, strategies & churn**

- **FR-011**: The framework MUST support Level-1 adversaries: participants
  running the honest transition with an adversarial strategy bundle,
  excluded from coverage/goodness denominators but otherwise
  indistinguishable to the driver. Participant storage MUST NOT preclude a
  future protocol-violating (Level-2) participant kind.
- **FR-012**: The framework MUST ship a silent-relay fan-out strategy
  (selects no targets), experiments-only — available to experiment
  configurations, never a protocol CLI kind.
- **FR-013**: The framework MUST ship a uniform-sampler dial strategy —
  dial min(target_degree, |candidates|) candidates sampled uniformly without
  replacement (all candidates when fewer than target_degree are available),
  from a seed derived from the master seed — experiments-only, never a
  protocol CLI kind; the protocol's own strategy kinds and injection shape
  are unchanged.
- **FR-014**: Honest churn MUST be a first-class parameter (count or
  proportion): a seeded draw marks the configured number of honest nodes
  down after topology formation and before publish. Down nodes MUST NOT be
  stepped or relay, MUST be excluded from eligible receivers and publisher
  choice, and MUST remain registered and present in peers' connection state
  (going down generates no events and requires no drain); the adversary
  count is unaffected by churn.

**Metrics — publish drain**

- **FR-015**: Coverage MUST be the fraction of eligible receivers — up-honest
  subscribed nodes excluding the publisher — whose received set contains the
  published message by content hash.
- **FR-016**: The driver MUST record each node's first-receipt wave; depth
  metrics MUST report the per-node distribution and the maximum (longest hop
  path, equal to waves-to-quiescence), counted in synchronous-round hops
  from the publisher (wave 0).
- **FR-017**: For every eligible receiver that missed the message, the
  framework MUST classify the cause from driver-owned state (never from log
  output): all upstream sources adversarial or down; no upstream at all; or
  no up-honest path from the publisher.
- **FR-018**: The framework MUST count total dissemination sends split by
  recipient class (honest / adversarial / down) and suppressed arrivals
  (deliveries whose hash the recipient had already seen, accounted by the
  driver), and MUST assert the identity
  sends = first receipts + suppressed + sent-to-down on every run.

**Metrics — realised-graph analytics**

- **FR-019**: The framework MUST compute topology-shape metrics from the
  extracted propagation digraph: in-degree and out-degree distributions and
  the honest-sink count (out-degree 0).
- **FR-020**: The framework MUST compute the good-topology verdict — every
  up-honest publisher reaches every up-honest node, evaluated as strong
  connectivity of the extracted up-honest propagation digraph via a
  strongly-connected-components pass, with no dissemination drain — recorded
  post-churn (primary) and, when churn > 0, also pre-churn (diagnostic).
- **FR-021**: Alongside the goodness boolean the framework MUST record min
  coverage over all up-honest publishers, computed from the same
  components pass as (smallest condensation-sink component − 1) /
  (up-honest − 1); the fraction of publishers achieving full delivery MUST
  be computable as an opt-in metric (absent from records when not computed).
- **FR-022**: The goodness computation MUST be parameterised as (propagation
  digraph, per-publisher seed set), with the dissemination model as an
  explicit experiment parameter recorded in the manifest and dispatched on
  by the goodness/metrics module; the dispatch owns propagation-graph
  extraction from node state, and all graph-derived metrics MUST be defined
  over the extracted digraph. v1 MUST ship the dispatch structure with
  exactly one implementation: M2 (extraction = downstream out-edges, seed =
  the publisher, good = one strongly connected component).

**Aggregates & statistics**

- **FR-023**: Per experiment, the framework MUST report distributions and
  percentiles, not just means: coverage histogram, missed-count
  distribution, depth histogram, message-cost histograms and means, P(full
  coverage), P(good topology), and histograms of the graded goodness
  refinements. Probability estimates MUST be reported as raw counts
  (successes, total runs) plus a Wilson score interval at the fixed 95%
  level — well-defined at all-good/all-bad samples, where a plain ±1σ
  standard error degenerates to zero width; any other convention (including
  the formal folder's ±1σ) is derivable from the counts.

**Reproducibility & execution**

- **FR-024**: One master seed MUST derive all randomness in a sweep —
  identity-key generation (seeded mock crypto), class assignment, churn
  draw, publisher choice, and per-participant strategy seeds — via a
  documented derivation (run seeds pre-derived from master seed and run
  index, independent of execution order), so any run is exactly replayable
  from its recorded seed.
- **FR-025**: A run MUST be a pure function (parameters, seed) → run record,
  performing no I/O and sharing no state with other runs; an experiment is a
  parallel map over its R seeds plus an order-canonical fold; a sweep
  flattens all (experiment, seed) pairs into one run-granularity work pool
  and owns all I/O.
- **FR-026**: Parallel execution MUST NOT perturb outputs: records are
  written in canonical run-index order and aggregation folds in that same
  order, producing byte-identical artifacts at any worker count; a
  parallelism-degree parameter MUST bound the number of in-flight runs.
- **FR-027**: The driver MUST NOT depend on the core's collection iteration
  order for determinism: each wave's collected sends are canonicalised into
  a deterministic order before routing, and all driver-side extraction and
  tally structures are built in sorted or index-keyed form — so whole-run
  byte-determinism holds while the core's connection-record collections
  remain hash-based. The ordered-collection conversion of those core
  collections is delegated to the in-flight connection-link strategies work
  (coordination note), and this feature makes no core changes beyond the
  crate-internal access the experiments module needs.

**Output contract**

- **FR-028**: A sweep MUST emit exactly three data artifacts (no plotting):
  a manifest (tool commit, master seed and derivation rule, and the
  expanded experiment list — one fully resolved parameter set per
  experiment, referenced by index; axes and fixed parameters appear
  expanded there), run
  records (one row per run, streamed in canonical order, containing only
  scalars and degree/depth-bounded vectors — nothing sized by the
  population), and a per-experiment aggregates file. There is no
  interruption resume in v1: a partial output is a valid prefix without a
  completion claim, and interrupted sweeps are re-run.
- **FR-029**: The aggregates file MUST be a pure function of the run
  records, so external tooling can recompute and diff it.
- **FR-030**: Per-run per-node detail (first-receipt wave, first-delivery
  origin, degrees, miss cause, class) MUST be available behind an opt-in
  flag, off by default.

**Configuration & validation support**

- **FR-031**: Sweep configuration MUST be parsed at the edge (front-end
  binary); the experiments API MUST take already-parsed values. v1
  configurations run on a single topic whose membership is the population;
  configurations that leave no eligible receivers or no up-honest publisher
  MUST be rejected at validation.
- **FR-032**: The framework MUST be able to construct scripted topologies
  with hand-computable metrics (e.g. line, star, full mesh) for its own
  validation, via the direct pre-population setup mode.
- **FR-033**: The framework MUST ship the M2-comparison worked example as
  two configurations plus a smoke variant: (a) the operating point (uniform
  sampler + accept-from-all + forward-to-relays at N = 20 000, μ = 0.2,
  RF = 24) for the cost/latency means; (b) a named bulk-regime point (P(bad)
  ~ 1e-2..1e-3, taken from m2's full-coverage validation grid) for the
  P(good)-vs-law check — both executed manually with the comparison
  documented; and (c) a suite-sized smoke variant that runs in the automated
  tests asserting pipeline health only (never numeric agreement). The
  documented comparison MUST include a short uncertainty-methodology note —
  the formal folder's ±1σ standard errors vs this framework's counts +
  Wilson 95%, why the difference matters at all-good samples, and how the
  conventions map — as an item to raise with the formal-methods team.

### Key Entities

- **Participant**: one simulated node — the real node state plus its injected
  strategy triad; classed honest or Level-1 adversarial (metrics-only
  distinction); honest participants may additionally be marked down by the
  churn draw. Storage is shaped to admit a future protocol-violating kind.
- **Run**: one seed — one topology formation, churn draw, publish, and
  measurement; a pure function of (parameters, seed) producing one run
  record.
- **Experiment**: R runs at one fixed parameter set; the unit of statistical
  aggregation (one point on a curve).
- **Sweep**: a set of experiments serving one question (typically one curve);
  the unit of execution, output, and provenance.
- **Sweep manifest**: the self-description artifact — code identity, master
  seed and derivation rule, and the expanded experiment list (one fully
  resolved parameter set per experiment).
- **Run record**: one row per run — seed, population as drawn, dial tallies,
  graph-analytic results (pre/post churn), publish-drain results; bounded
  size independent of population.
- **Aggregates file**: per-experiment distributions, percentiles, and
  P(good) with confidence interval; derivable from the run records.
- **Propagation digraph**: the directed relay graph extracted from node
  state by the dissemination-model dispatch; the object all graph-derived
  metrics are defined over.
- **Dissemination model**: the explicit experiment parameter naming which
  extraction + seed-set + goodness rule applies (v1: M2 only).
- **Silent relay / uniform sampler**: the two experiments-only strategy
  instances shipped with the framework (adversarial fan-out; M2-faithful
  dial).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Two executions of the same sweep configuration and master seed
  — including across process restarts and different worker counts — produce
  byte-identical manifest, run-records, and aggregates artifacts.
- **SC-002**: On the scripted validation topologies, measured coverage and
  depth equal the hand-computed values exactly (e.g. full mesh, all honest:
  coverage 1.0, depth 1; a line of L nodes published from one end: coverage
  1.0, depth L − 1), including a silent-adversary scenario whose coverage
  and cause decomposition match the hand-computed severed paths.
- **SC-003**: On every run, publish-drain coverage equals graph-analytic
  reachability from the publisher, and the accounting identity
  sends = first receipts + suppressed + sent-to-down holds.
- **SC-004**: Any run replayed from its recorded seed on the same code
  reproduces its run record exactly, and (with detail enabled) yields the
  per-node table.
- **SC-005**: Default run records stay bounded regardless of population
  size: a record's size is governed by maximum degree and depth, not by N,
  and contains no per-node listing.
- **SC-006**: The M2-comparison smoke variant completes inside the automated
  suite in under 30 seconds; the full-size operating point (N = 20 000, R ≥
  40) completes a manual execution on a contemporary developer machine in
  under one hour and yields every quantity the documented comparison table
  needs (honest-to-honest sends, copies per honest node, depth
  distribution).
- **SC-007**: An experiment's aggregates report P(good) as raw counts plus a
  Wilson 95% interval in all cases — including all-good and all-bad samples,
  where the interval has nonzero width.
- **SC-008**: With the `experiments` feature disabled, the crate's build,
  public API surface, and test results are unchanged from before the
  feature.

## Assumptions

- **Substrate**: 005-peer-view is merged on `main` (PR #73); the framework
  branches from `docs-experiments-program` because it cites
  `docs/experiments-program.md`, which lands with PR #74 — that PR merges
  before this feature's PR.
- **Population & topic**: v1 experiments run on a single registered topic;
  every population member subscribes to it, and the topic is registered
  open (any node may publish), so the seeded publisher is always authorized.
- **Adversary model in v1**: Level-1 only, and every shipped fan-out is
  all-or-nothing (forward-to-all or silent), so realised-graph analytics are
  exact, not bounds, for all v1 configurations.
- **Depth semantics**: depth is the synchronous-round hop count (the
  convention the formal models' hops columns use), a topology property —
  not a wall-clock latency claim; timing-skew modelling (random per-edge
  delays, discrete-event scheduling) is a named later driver extension.
- **Single-epoch runs**: the epoch nonce stays at its genesis value for the
  whole run; multi-epoch experiments wait on the connection-rotation
  feature.
- **Churn semantics**: down ≠ unregistered — churned nodes stay in
  registries and in peers' connection state; there is no liveness detection
  in v1.
- **Statistical reach**: Monte-Carlo estimates probabilities down to the
  bulk regime (~1e-2..1e-3 with R ~ 1e4); security-grade tails belong to
  the analytical laws (the 1/p sampling wall).
- **Feature id**: 016 — the id 015 is burned (committed docs reference a
  former, dropped feature 015).

## Dependencies

- **005-peer-view (merged)**: the Epoch/Heartbeat event vocabulary, the
  readiness gate, the strategy kinds experiment configurations install, and
  the `NodeView`-based strategy seams.
- **docs/experiments-program.md (PR #74, open)**: the consumer document —
  stages E1–E13 and the program's metric definitions; must merge before this
  feature's PR.
- **formal_spec/hybrid_dissemination/models/ (READ-ONLY per Principle V)**:
  the shared good-graph criterion, the M2 operating point, and the published
  values the demonstration compares against.
- **Deliberately untouched**: N-020 (readiness-dial coupling), N-011/N-012
  (add-only connections — the reason runs are single-epoch), the strategy
  injection shape (constructor-injected, per 005), and the node core
  entirely — the experiments module needs only crate-internal
  read/construction access; the ordered-collection conversion of the
  connection records is delegated to the in-flight connection-link
  strategies work (per Clarifications, Session 2026-07-18).

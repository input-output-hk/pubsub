# Dissemination experiment program

**Status:** program of record. This document describes the dissemination experiments we run on the
deterministic experiments framework: the order they run in, the analytical results each compares
against, what the framework produces that the analysis cannot, and — per experiment — what has been
executed so far. It replaces the original proposal draft: the framework exists (the `experiments`
module and binary), the first comparison is executed and documented
([`docs/experiments/m2-comparison.md`](experiments/m2-comparison.md)), and the design decisions the
proposal left open have been taken; where one shapes an experiment, the outcome is stated in place.

## 1. Purpose and scope

The framework drives populations of the crate's real node cores — the same transition function,
strategy seams, and message vocabulary the node runs — under a deterministic round-based scheduler.
We use it to characterise dissemination behaviour as the network grows and as adversaries deviate
from the protocol: chiefly the **fraction of nodes that receive a published message**, the **hop
depth** of delivery, whether the realised topology is **good** (every publisher can reach every
up-honest node), and — for every node that misses — **why**.

Analytical reference points exist for much of this (Section 2). The framework's value is twofold: it
**validates** those results in regimes they only describe asymptotically, and it **produces results
where no closed form exists** — finite networks, the actual topology the protocol builds, adversaries
richer than the analytical worst case, and multi-round dynamics. Section 3 makes that division
explicit; Section 5 applies it experiment by experiment.

In scope: single-topic dissemination over the model-family topologies (pull relaying, initiation
links, bidirectional links, k-in/k-out), the golden push tier, adversarial nodes, and the
deposit/slashing dynamics enforcement introduces. Out of scope: a real network transport, on-chain
integration, peer discovery/IP layers, and storage — the registry provides a global view, and the
in-process model is sufficient for the statistics we care about.

## 2. Reference models (the analytical yardsticks)

**The M1–M5 dissemination-model family** (`formal_spec/hybrid_dissemination/models/`) is the primary
reference set. Each model fixes a link discipline over N nodes with k = μN silent adversaries and
publishes validated coverage laws, cost/latency values, and Monte-Carlo grids
(`models/comparison.md`; per-model `properties/full_coverage.md`):

- **M1 — push-only**: every node picks k_out targets and forwards everything to them.
- **M2 — pull relaying**: every node picks RF forwarders that relay everything they hold to it. The
  baseline all shipped experiments run on.
- **M3 — pull + standing initiation links**: M2's relay mesh plus s−1 seeding links per node,
  carrying only their owner's own publications (s counts the publisher itself). Eliminates the muted-
  publisher failure mode that dominates M2's bad tail.
- **M4 — bidirectional RF-out gossip**: every node picks RF peers, each pick a bidirectional link;
  flooding minus the arrival link. Minimum degree ≥ RF by construction — connected w.h.p. at RF ≥ 2,
  with no ignition failure mode at all.
- **M5 — directed k-in/k-out gossip**: each node picks k_in forwarders and k_out targets; the
  boundary cases recover M2 (k_out = 0) and M1 (k_in = 0) — built-in sanity checks for any M5 sweep.

The node implements all five: M4 is exactly-RF uniform picks (the models' selection family, the
node's pick count) established over the symmetric handshake.

**Erdős–Rényi closed forms** remain the asymptotic yardstick for the honest metrics: eclipse rate,
adversary tolerance `k_max(ε)`, coverage and its distribution, connectivity/partition thresholds,
and diameter. The protocol does not build ER graphs (Section 3), so these are limits to measure
deviation from, not ground truth.

**The golden-tier eclipse formula** (`formal_spec/hybrid_dissemination/partitioning/golden_tier/`)
covers the push-tier extension of Stage 6:

```
P(eclipse) ≈ exp(−G·F_g/N) · (k/N)^RF
```

with G golden nodes pushing to F_g targets each; golden parameters at zero reduce it to the pure-pull
eclipse `(k/N)^RF` the M2 stages target.

## 3. What the framework adds beyond the analytics

For the metrics with closed forms, the framework contributes:

- **Finite N.** The closed forms are asymptotic; real deployments are finite, and the finite-N
  behaviour — especially near thresholds — is what the framework measures directly. The executed M2
  comparison demonstrates the mode: depth means matched the published values exactly, and the bulk
  P(good) point landed within ordinary sampling noise of both the coverage law and the formal
  Monte Carlo.
- **The actual topology, not ideal ER.** Each node dials a fixed RF upstreams, so the realised graph
  is a k-out random digraph — regular out-degree, picks without replacement — not ER's independent
  Poisson edges; later stages add a golden hub tier and load-dependent admission. Quantifying the gap
  between the realised topology and the idealisation is itself a result.
- **Distributions and correlation, not just means.** A closed form gives an expected value; the
  operative question for a dissemination layer is the bad tail. Two configurations with the same 95 %
  mean coverage can behave oppositely — one delivers ~95 % every run, the other delivers 100 % most
  runs and occasionally collapses. Failures are also correlated: reception is a path property, so a
  single cut orphans a downstream cluster at once — independence-based union bounds predict tight
  concentration where the realised graph has fat tails. Sweeping seeds gives the full distribution
  and the outage-size clustering the mean cannot express.
- **Mechanism, not just rate.** For every node that misses, the instrument knows why — all upstreams
  adversarial or down, no upstream at all, or no up-honest path — so a coverage shortfall decomposes
  into causes. No closed form provides this.

Beyond the closed-form metrics, the framework is the **only** tool for: multi-round dynamics
(healing across epochs, compounding attackers, budget depletion under slashing); churning
populations; and whichever richer adversarial behaviours survive the relevance classification of
Stage 5 — for one-shot dissemination most are bounded by the silent adversary (E15).

## 4. Instruments and behaviour modeling

**The framework** (ADR 0035/0036): a *run* is a pure function of (parameters, seed) — registration,
dial drain, churn draw, SCC passes, publish drain, measurement; an *experiment* is R runs at one
parameter set; a *sweep* is the experiments serving one question. Artifacts are byte-reproducible
from the master seed and tool commit at any worker count. Two measurement instruments cross-check
each other:

- the **publish drain** — realised coverage (excluded-publisher denominator), per-node first-receipt
  depth, miss-cause decomposition, and send accounting with per-run identity checks; sends are split
  by recipient class **and** by carrying link kind (relay/publisher — under M3 the split reads
  relaying vs seeding, under M5 pull-serving vs push-forwarding, and the M5 grid's boundary
  reductions show as one column constant at zero; ADR 0041);
- **realised-graph analytics** — per-model extraction behind the `DisseminationModel` dispatch
  (M2/M3/M4: relay edges only, initiation links never relay; M5/M1: the union of relay and publisher
  edges), goodness from the condensation: one SCC for the publisher-alone-seed models, and for M3 the
  **seed-aware** criterion — every publisher's seed set (itself plus its initiation targets) must
  close over the whole graph, the formal M3 study's exact every-publisher check — with
  min-publisher-coverage as the worst per-publisher closure fraction. Goodness must come from this
  pass, not from sampled drains: a muted publisher is invisible to any other publisher's
  dissemination (the executed comparison's structural finding).

Probabilities are always reported as **raw counts plus a Wilson 95 % interval** — the ±1σ convention
degenerates to zero width at all-good samples, which well-sized configurations make the common case
(the methodology note in the M2 comparison, §4, records the convention mapping).

**Honest behaviour** is expressed through the node's injected strategy seams:

- **Connection** (dial): one selection implementation per seam over two knobs — the **bucket count**
  (keep the candidates passing the verifiable edge predicate at B; absent = ungated) and the **pick
  count** (exactly min(pick count, gate survivors) seeded uniform picks without replacement — the
  models' selection family, needed wherever a comparison must not conflate the selection-family gap
  with instrument error; absent = every survivor, 0 = dial none). The former strategy kinds are plane
  points: connect-to-all (both absent), hash-gated (bucket count only), uniform picks (pick count
  only), gated picks (both).
- **Acceptance** (admission): the same two dimensions on the serving side — gate verification
  follows the seam's bucket count (an `accept_unverified` opt-out preserves the trusting-acceptors
  comparison arm) plus an absolute per-seam **accept cap** (no retry/back-fill in v1 — a refused
  dial is simply not re-attempted).
- **Fan-out**: `forward-to-all` (the default — every held message over all links, M5's send side),
  `forward-to-relays` (M3's exclusivity: held messages to relay links only, own publications seeded
  over publisher links), and the experiments-only `silent-relay`.
- **Link kinds** realise the model family: the relay mesh (M2), the optional publisher pair for
  standing initiation links (M3/M5), and the symmetric handshake whose one accept decision
  establishes a bidirectional link on both ends — with a pick count and no bucket count, exactly
  M4. The sweep config declares the publisher pair per class (the `publisher` sub-table, with a
  `publisher_pick_count` axis for k_out sweeps) and validates the declared model name against the
  honest class's wiring before anything runs, so one config name yields consistent wiring and
  measurement (ADR 0041).

**Adversaries** come in two tiers. Level-1 — a hostile strategy bundle on an otherwise-honest node —
is implemented; the silent relay (the models' worst-case adversary) ships with the framework, and a
population mixing several adversarial bundles is just configuration, not new machinery. Level-2 —
coordinated adversaries with shared state and protocol-violating freedom (equivocation, flooding,
Sybil coordination) — has a design sketch and remains out of scope for this document; Stage 5's
relevance classification decides which adversarial behaviours, of either tier, are worth building an
instrument for at all.

## 5. Experiment program (order and status)

Stages are ordered so each needs the least new machinery beyond the previous; experiments are
numbered in presentation order. Status markers: **[done]** executed and documented, **[ready]**
runnable with today's machinery, **[needs: X]** blocked on named machinery — qualified where an
experiment is partially covered.

Where an attacker's optimal move and its effect are analytically obvious, we **document the attack
and its bound rather than simulate it**; simulation is reserved for configurations where a defence
produces non-trivial dynamics.

### Stage 1 — Honest pull dissemination (M2, k = 0)

- **E1 — Coverage and partition vs network size** [ready]. Sweep N at fixed RF; coverage and the
  onset of partition. Goodness and component structure come directly from the SCC instrument.
- **E2 — Propagation depth** [ready]. Depth distribution (not just the mean diameter) vs N and RF.

**Vs analytics:** ER connectivity/diameter closed forms — finite-N, realised-topology validation.

### Stage 2 — The M2 attack: silent adversaries

- **E3 — Per-target eclipse rate** [ready]. Compare to `(k/N)^RF`.
- **E4 — Adversary tolerance `k_max(ε)`** [ready]. Sweep k; bulk ε only.
- **E5 — End-to-end coverage under silent adversaries** [partially done]. Distribution, correlation,
  and cause decomposition. The executed **M2 comparison** covers this stage's fixed points: the
  N = 20 000 operating-point cost/latency means (exact agreement), the bulk-regime P(good) vs the
  coverage law (within sampling noise, Wilson-quantified), and a full-N grid-cell cross-check —
  see [`docs/experiments/m2-comparison.md`](experiments/m2-comparison.md). The k- and N-sweeps
  around those points remain to run.

### Stage 3 — Model-family cross-validation (M3, M4, M5)

The published per-model laws and grids (`models/comparison.md`) give each configuration its yardstick;
the boundary reductions (M5 → M2 at k_out = 0, M5 → M1 at k_in = 0) are built-in sanity checks.

- **E6 — M3, initiation links** [done]. Executed and documented in
  [`docs/experiments/m3-comparison.md`](experiments/m3-comparison.md): five coverage-law cells
  (bulk through the 30 000-run deep tail, both sizes) all law-consistent; the operating-point cost
  and latency means at published precision; the seeding cost measured at exactly s−1 publisher-kind
  sends per message via the kind split; the seed-aware goodness realising the study's exact
  every-publisher check.
- **E7 — M4, bidirectional links** [done]. Executed and documented in
  [`docs/experiments/m4-comparison.md`](experiments/m4-comparison.md): RF = 3/4/5/6 coverage cells
  law-consistent (the RF = 6 deep tail at 251/30 000 vs the law's 0.00836 — a 1.00× ratio), the
  RF = 8 operating point exact at published precision, degrees mirrored fleet-wide.
- **E8 — M5, the k-in/k-out grid** [done]. Executed and documented in
  [`docs/experiments/m5-comparison.md`](experiments/m5-comparison.md): seven M5 cells (the swap
  symmetry exercised and tightened) plus five M1 boundary cells, all law-consistent; both operating
  points at published precision; the kind split reproducing the k_in : k_out ratio and M1's empty
  relay mesh in the accounting.

### Stage 4 — Selection and admission knobs (separable layers)

Bucketing constrains *sampling*; the serving cap bounds *fan-in*. They defend against different
things and are studied separately. The bucket count B is **configuration** — a deliberately chosen
security trade-off, never derived from local state.

- **E9 — Bucketing, no cap** [ready]. Eclipse/coverage (the E3/E5 metrics) with bucketing on vs off,
  sweeping B; the bucketed-pull analysis predicts the eclipse fraction unchanged at the balanced B —
  confirm, then explore off-balanced.
- **E10 — Selection-family fidelity** [done]. Executed and documented in
  [`docs/experiments/e10-selection-fidelity.md`](experiments/e10-selection-fidelity.md): eleven
  cells over the calibrated M2 bulk point. Gated picks reproduce the coverage law exactly at
  survivor headroom r = (N−1)/(B·K) ≥ 2 (pooled 279/32 000 vs the law's 0.0088), degrade 5× at
  r = 1, and converge to gate-only behaviour below it; gate-only doubles P(bad) at equal mean
  degree by resurrecting the eclipsed-receiver defect class, and the +1-link compensation
  (B = (N−1)/(RF+1)) restores the law — measured, not just derived. The fidelity question
  ADR 0032/0034 deferred to this harness is answered.
- **E11 — Serving cap, honest** [ready]. With uniform dialing, serving load varies by chance; when
  the cap sits close to RF, upper-tail nodes refuse honest dials — and v1 has no retry, so a refused
  dial is lost. Measure effective in-degree and coverage against the uncapped baseline as a function
  of cap headroom. (A retry/back-fill variant re-runs this when that mechanism exists.)
- **E12 — Flooding mitigation under the cap** [done]. Executed and documented in
  [`docs/experiments/e12-flooding-mitigation.md`](experiments/e12-flooding-mitigation.md): the
  pilot plus a 48-cell B × cap × Sybil-count grid over the rational level-1 flooder (the
  (bucket count pinned, no pick count) adversarial bundle with silent-relay fan-out). Attacker
  concentration measured exactly at ≈ K/B wherever the cap leaves room (cap-truncated at the
  narrow-gate corner); the cap-controlled comparison shows starved honest links — not slot
  concentration — are the harm, with cap headroom absorbing attacks a tight cap converts into
  topology damage. Jointly with E10: the B trade-off table — at fixed pick count, the largest B
  with r ≥ 2 is both coverage-exact and flood-resilient. Measurement via the per-node
  connection-accounting detail columns; bounding cases documented below, unchanged.
- **E18 — Gated-symmetric selection (gated M4)** [done]. Executed and documented in
  [`docs/experiments/gated-symmetric.md`](experiments/gated-symmetric.md): the hash gate composed
  with the symmetric handshake — N-039's revisit trigger, no published law (the cells test
  closed-form predictions recorded before running). The pair draw makes picks and pickers share one
  survivor pool: B enters the realised degree everywhere (d = λm(2−m), measured to three digits
  across B = 10–500 — no r ≥ 2 plateau exists in this family), and isolation gains the
  **empty-pool channel** e^(−(1−μ)(N−1)/B) — K-independent, so RF cannot compensate — confirmed
  57 + 61 bad/8 000 at B = 250 against a matched-degree ungated twin at 0/8 000 (the naive
  law-at-realised-degree transfer rejected ~430×), and its (1−μ) exponent confirmed across
  μ = 0.2/0.3/0.4 at equal event counts (the μ-axis cells). Design rule: size the pool, not the headroom —
  (N−1)/B ≥ ln(H/δ)/(1−μ), with the gate coverage-free at r ≳ 3; the ordered-predicate
  alternative is priced in the report (≈ 2/B admissibility at equal B; the frontier
  λ_floor/(N−1) is predicate-independent). The benefit-side flooding grid (the E12 analogue under
  the symmetric handshake) is E19.
- **E19 — Symmetric flooding under the admissions budget (+ the ordered arm)** [done]. Executed and
  documented in [`docs/experiments/symmetric-flooding.md`](experiments/symmetric-flooding.md): the
  E12 question re-asked on the symmetric handshake, carrying the machinery its answer required —
  ADR 0042 resolved N-032 (the cap on a symmetric seam is an **admissions budget**: fresh peer
  arrivals spend it, crossings are exempt, own picks are never vetoed, degree ≤ K + C by
  construction) and N-040's drain-time route attribution made it measurable
  (own-only / mutual / admitted × peer class, refusals split fresh vs crossing). Results: the
  retired both-role scan's crossing veto measured once at its pinned commit (0.074/victim; the
  budget's is exactly zero over ~10⁷ refusals); the pilot-calibrated without-replacement race law
  at **zero flags across the 19-cell B × cap × Sybil grid**; the occupancy decomposition — the
  cap-blind own-pick floor K·μ (no acceptance policy sees it) vs the gate-divided admission route
  (S/B)(1−m); the cap-sizing rule re-anchored on the fresh-arrival load K(1−m)(1−μ); flooding
  structurally inert at pool saturation; and the replicated **cap × empty-pool composition term**
  (pooled 166/800 vs the uncapped law's 0.148, z = +4.75; the ledger's `capsweep` computes the
  full cap trade-off curve from the grid-validated race law, measured at both ends — the binding
  C = 3 corner and the pre-registered C = 12 quiet-end anchor, 59/400 bad vs registered 59.3 —
  so past the pool floor no cap both binds and stays harmless; inside the window the term is
  doubly suppressed *given* the sizing rule).
  The **ordered arm** (ADR 0043) turned E18 §4's derived pricing into measured rows: the
  pick-repairable tail is real below the saturation boundary B < (N−1)/K (0/8 000 at equal B vs
  the unordered 57) and vanishes above it (80/8 000 at equal density — the corrected
  registration), while at a binding budget the 2/B looseness converts into doubled honest
  starvation, not extra occupancy — the unordered pair's dominance is now measured, not argued.
  Instrument notes: two pre-registered race models corrected against measurement (documented),
  and N-042 — the wavefront budget race is class-fair but rank-concentrated dialer-side.
- **E20 — The M4 synthesis: the gated recommendation and the gated model comparison** [done].
  Executed and documented in [`docs/experiments/m4-synthesis.md`](experiments/m4-synthesis.md):
  the (N, K)-parameterised prediction ledger (`m4_synthesis_predictions.py` — the E18/E19 forms
  lifted to parameters, the directional/M3 forms validated against E10 and the published ungated
  op points, B = 1 recovering each ungated law exactly) plus eleven pre-registered cells at CIP
  scale and at the CIP's pick count (seeds 1139–1149, every one verified against its frozen
  registration — two coverage lines missed as recorded, the seed-rescue corrections documented).
  Headlines: **gated M4 at K = 10, B = 500, C = 23 is equal-or-better than the ungated CIP op
  point on every quoted axis except ~1 % of mean latency** (P(bad) 5.1e-6 vs 6.1e-6,
  copies/honest 13.00 vs 13.40 — the shared pool absorbs the extra pick, hops 5.00/3.95 vs
  5.00/3.90, churn budget 7.57 % vs 7.43 %) with flood
  divisor 500 and degree ≤ K + C exact; the cap-headroom floor is K-dependent (c ≈ 3.5 at
  K = 9–10, the three-point composition-curve rehearsal); the gated model comparison under the
  equal-attack-surface normalization (directional 2/B vs the pair draw's 1/B) shows **the pair
  draw runs twice the pool per unit of surface** — no M3 pick count meets the target at M4-equal
  total surface (the equal-deafen-cost alternative normalization prices feasible but ~9× behind
  at 19 picks vs 10 — both normalizations stated in report §7), measured by the equal-surface
  cliff pair (gated M3 17/400 bad, all deaf-class, vs M4 400/400 at surface 32); the first
  capped publisher-seam cells (N-041 completed) found the
  **seed-rescue coupling** (a binding seed-intake cap starves exactly the rescuing seeds,
  f = μ + (1−μ)ρ_p — size C_p to clear the intake load) and fired N-042's trigger (rank
  dissection recorded; the instrument fix chartered with frozen validation re-runs). Report §9
  catalogues the gated closed forms with epistemic grades, a symbol table, and the
  isolated-vertex reduction scoped (the enumerated pair-component term is ≤ 3.3e-4 of E_iso at
  every measured shape; the powered cells show zero pair excess); the PR's formal review
  independently re-derived the forms and reproduced every quoted number — the remaining
  hardening step is a derivation document in the formal folder's style.

**Documented, not simulated:** no cap ⇒ no flooding surface (nothing to exhaust); cap without
bucketing ⇒ the obvious attack (every Sybil dials the victim; concentration ≈ K, honest requests
crowded out as K approaches the cap).

### Stage 5 — Richer adversarial and dynamic behaviour

- **E13 — Churn** [ready]. Coverage, goodness, and miss causes as a function of the down fraction;
  seeded honest churn is a shipped first-class parameter (down ≠ unregistered; the adversary count
  is unchanged by the draw).
- **E14 — Multi-round healing and compounding** [needs: connection rotation]. Whether coverage
  recovers as the topology re-samples across epochs, and whether a persistent adversary compounds.
- **E15 — Adversarial-behaviour relevance classification** [analysis first; simulation for
  survivors]. Before richer adversary machinery is built — of either tier — classify each candidate
  behaviour by whether it can outperform the **silent** adversary at equal resources. Three
  structural arguments bound most of the space: for one-shot dissemination a Level-1 active relay is,
  per message, a mixture of silent and honest (a selective dropper is exactly a silent relay for the
  messages it targets and honest for the rest, so its per-message harm never exceeds full silence);
  enforcement pushes provable behaviours toward unprovable silence (the Stage-7 floor); and
  grinding-resistant selection removes placement levers (an attacker coordinates its own actions,
  never the victim's picks). Behaviours shown silent-bounded are documented with their argument, not
  simulated. The expected survivors are the ones that attack something other than a single message's
  dissemination — serving-capacity flooding (E12), detection evasion under enforcement (Stage 7,
  where selective dropping matters precisely because it is unprovable), and timing/compounding
  effects that only exist with rotation (E14). Their simulations join the stage that owns the
  machinery they need.

**No coordinated-eclipse experiment.** Eclipsing a receiver offers no coordination lever under
grinding-resistant selection: the victim picks its own upstreams from a set the attacker cannot bias,
so adding Sybils only raises the aggregate probability already measured by E3/E5. The one genuinely
coordinated receiving-side attack is serving-slot flooding (E12).

### Stage 6 — Golden nodes (push tier)

- **E16 — Full golden-tier model** [needs: golden push feature]. Re-run E3–E5 with G > 0, F_g > 0;
  confirm the push × pull factorisation of the golden-tier eclipse formula, and measure the hubs'
  effect on depth and on coverage under attack — which the per-target formula does not express. The
  adversarial configurations that survive E15's classification re-run with the tier present, to
  measure how much the push tier restores coverage under attack.

### Stage 7 — Deposit and slashing dynamics

- **E17 — Slashing dynamics** [needs: provable-misbehaviour detection, rotation, registry deposit
  field]. Effective-adversary decay across epochs, enforcement-driven healing, attacker budget
  depletion. Deposit sizing itself is arithmetic over the tolerance and concentration curves of
  Stages 2 and 4, not a separate simulation. A structural point frames the stage: slashing removes
  only *provable* misbehaviour, so a rational attacker is pushed toward unprovable silence — the
  residual threat surviving enforcement is exactly the silent adversary of Stage 2, which is
  therefore the floor.

## 6. Summary table

| # | Experiment | Stage | Analytical reference | Status |
|---|------------|:-----:|----------------------|--------|
| E1 | Coverage & partition vs N | 1 | ER closed form | ready |
| E2 | Propagation depth | 1 | ER diameter | ready |
| E3 | Per-target eclipse rate | 2 | M2 `(k/N)^RF` | ready |
| E4 | Adversary tolerance `k_max(ε)` | 2 | M2 | ready |
| E5 | End-to-end coverage, silent adversaries | 2 | M2 coverage law / ER percolation | **fixed points done** (M2 comparison); sweeps ready |
| E6 | M3 — initiation links | 3 | M3 law + grids | **done** (m3-comparison) |
| E7 | M4 — bidirectional links | 3 | M4 law (RF ≥ 2) | **done** (m4-comparison) |
| E8 | M5 — k-in/k-out grid | 3 | M5 law + boundary reductions | **done** (m5-comparison, M1 boundary included) |
| E9 | Bucketing, no cap | 4 | bucketed-pull (balanced B) | ready |
| E10 | Selection-family fidelity (B, K) | 4 | model selection family | **done** (e10-selection-fidelity) |
| E11 | Serving cap, honest | 4 | none (congestion) | ready |
| E12 | Flooding mitigation under cap | 4 | bucketed-pull concentration | **done** (e12-flooding-mitigation) |
| E13 | Churn | 5 | none | ready |
| E14 | Multi-round healing | 5 | none | needs rotation |
| E15 | Adversarial relevance classification | 5 | silent-adversary bound | analysis ready |
| E16 | Golden push tier | 6 | golden-tier formula | needs golden feature |
| E17 | Slashing dynamics | 7 | deposit arithmetic | needs detection + rotation |
| E18 | Gated-symmetric selection (gated M4) | 4 | none published (N-039 boundary) — own two-channel law | **done** (gated-symmetric) |
| E19 | Symmetric flooding under the admissions budget (+ ordered arm) | 4 | own race law + E18 §4 pricing | **done** (symmetric-flooding) |
| E20 | M4 synthesis: the gated recommendation + gated model comparison | 4 | the (N, K)-parameterised ledger (E10/E18/E19 forms; B = 1 = the ungated laws) | **done** (m4-synthesis) |

Reading the table: the confirmation experiments validate closed forms at finite N and for the
topology the protocol actually builds; E11 and E13–E15 are where the framework is the primary
instrument — the questions have no closed form, and E15 decides how much of the adversarial space
needs an instrument at all.

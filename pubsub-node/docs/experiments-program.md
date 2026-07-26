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

The node implements M2, M3, and M5; M4 lands with the uniform exactly-RF selection kind (the models'
selection family), a planned follow-up.

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
  depth, miss-cause decomposition, and send accounting with a per-run identity check;
- **realised-graph analytics** — good topology ⟺ one strongly connected component of the up-honest
  propagation digraph, min-publisher-coverage from the condensation's sinks, degree and sink
  statistics. Goodness must come from this pass, not from sampled drains: a muted publisher is
  invisible to any other publisher's dissemination (the executed comparison's structural finding).

Probabilities are always reported as **raw counts plus a Wilson 95 % interval** — the ±1σ convention
degenerates to zero width at all-good samples, which well-sized configurations make the common case
(the methodology note in the M2 comparison, §4, records the convention mapping).

**Honest behaviour** is expressed through the node's injected strategy seams:

- **Connection** (dial): `connect-to-all`, `hash-gated` (bounded, verifiable, hash-derived selection
  at `target_degree` = RF over a configured bucket count), `none`; plus the experiments-only
  `uniform-sampler` (exactly-RF uniform picks — the models' selection family, needed wherever a
  comparison must not conflate the selection-family gap with instrument error).
- **Acceptance** (admission): `accept-from-all`, `bounded` (serving cap, no retry/back-fill in v1 —
  a refused dial is simply not re-attempted), `hash-gated`, `hash-gated-bounded`, `none`.
- **Fan-out**: `forward-to-relays` (the default — held messages to relay links, own publications
  seeded over publisher links), `forward-to-all` (every held message over all links — M5's send
  side), and the experiments-only `silent-relay`.
- **Link kinds** realise the model family: the relay mesh (M2), the optional publisher pair for
  standing initiation links (M3/M5), and the symmetric handshake whose one accept decision
  establishes a bidirectional link on both ends (M4's mechanics, awaiting the uniform selection
  kind).

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

- **E6 — M3, initiation links** [needs: publisher-pair experiment configuration]. The s−1 mapping,
  the seeding/relaying cost split, and the elimination of M2's muted-publisher tail; coverage law and
  cost values vs the published M3 grids.
- **E7 — M4, bidirectional links** [needs: uniform exactly-RF selection kind]. The minimum-degree
  floor and connectivity at small RF vs the published M4 law.
- **E8 — M5, the k-in/k-out grid** [needs: publisher-pair experiment configuration]. Sweep both
  axes, verify the boundary reductions, compare the interior to the published values.

### Stage 4 — Selection and admission knobs (separable layers)

Bucketing constrains *sampling*; the serving cap bounds *fan-in*. They defend against different
things and are studied separately. The bucket count B is **configuration** — a deliberately chosen
security trade-off, never derived from local state.

- **E9 — Bucketing, no cap** [ready]. Eclipse/coverage (the E3/E5 metrics) with bucketing on vs off,
  sweeping B; the bucketed-pull analysis predicts the eclipse fraction unchanged at the balanced B —
  confirm, then explore off-balanced.
- **E10 — Selection-family fidelity** [needs: uniform exactly-RF selection kind]. Hash-gated
  selection realises a binomial degree around RF where the models prescribe exactly-RF uniform picks.
  Sweep the (bucket count, pick cap) plane and quantify the deviation from the models' laws — the
  fidelity question ADR 0032/0034 defer to this harness.
- **E11 — Serving cap, honest** [ready]. With uniform dialing, serving load varies by chance; when
  the cap sits close to RF, upper-tail nodes refuse honest dials — and v1 has no retry, so a refused
  dial is lost. Measure effective in-degree and coverage against the uncapped baseline as a function
  of cap headroom. (A retry/back-fill variant re-runs this when that mechanism exists.)
- **E12 — Flooding mitigation under the cap** [needs: Level-1 flooding dial kind]. Adversarial
  Sybils exhaust a victim's slots; measure concentration reduction toward ≈ K/B and honest
  starvation.

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
| E6 | M3 — initiation links | 3 | M3 law + grids | needs publisher-pair config |
| E7 | M4 — bidirectional links | 3 | M4 law (RF ≥ 2) | needs uniform selection kind |
| E8 | M5 — k-in/k-out grid | 3 | M5 law + boundary reductions | needs publisher-pair config |
| E9 | Bucketing, no cap | 4 | bucketed-pull (balanced B) | ready |
| E10 | Selection-family fidelity (B, K) | 4 | model selection family | needs uniform selection kind |
| E11 | Serving cap, honest | 4 | none (congestion) | ready |
| E12 | Flooding mitigation under cap | 4 | bucketed-pull concentration | needs flooding dial kind |
| E13 | Churn | 5 | none | ready |
| E14 | Multi-round healing | 5 | none | needs rotation |
| E15 | Adversarial relevance classification | 5 | silent-adversary bound | analysis ready |
| E16 | Golden push tier | 6 | golden-tier formula | needs golden feature |
| E17 | Slashing dynamics | 7 | deposit arithmetic | needs detection + rotation |

Reading the table: the confirmation experiments validate closed forms at finite N and for the
topology the protocol actually builds; E11 and E13–E15 are where the framework is the primary
instrument — the questions have no closed form, and E15 decides how much of the adversarial space
needs an instrument at all.

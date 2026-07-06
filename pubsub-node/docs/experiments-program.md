# Dissemination experiment program

**Status:** proposal for team review. This document lists the dissemination experiments we intend to
run on the prototype, the order in which we propose to run them, how each compares to the analytical
results we already have, and — for each — where the prototype produces data that the analysis cannot.
It deliberately does **not** specify how the experiments are implemented; once the list and order are
agreed, we translate it into an implementation plan.

## 1. Purpose and scope

The prototype is a discrete, deterministic model of the node protocol (connection establishment plus
message dissemination). We want to use it to characterise dissemination behaviour — chiefly the
**fraction of nodes that receive a published message** and the **longest hop path** a message takes —
as the network grows and as adversaries deviate from the protocol.

Two reference points already exist analytically (Section 2). The value of the prototype is twofold:
it **validates** those analytical results in regimes they only describe asymptotically, and it
**produces results in regimes where no closed form exists** — finite networks, the actual (non-ideal)
topology the protocol builds, adversaries richer than the analytical worst case, and multi-round
dynamics. Section 3 makes that division explicit; Sections 5–6 apply it experiment by experiment.

In scope: single-topic dissemination over the pull topology, the golden push tier, adversarial nodes,
and the deposit/slashing dynamics that enforcement introduces. Out of scope: a real network transport,
on-chain integration, peer discovery/IP layers, and storage — the registry provides a global view, and
the in-process model is sufficient for the statistics we care about.

## 2. Reference models (the analytical yardsticks)

**M2 — per-target eclipse.** The M2 model (`formal_spec/hybrid_dissemination/partitioning/golden_tier/`)
gives a closed form for the probability that one honest node receives no useful in-edge in a single
round:

```
P(eclipse) ≈ exp(−G·F_g/N) · (k/N)^RF
```

with N nodes, G golden nodes pushing to F_g targets each, regular nodes pulling RF forwarders, and k
silent adversaries. It is a **per-target, single-round, one-hop** quantity under a **silent,
uniformly-placed** adversary and an honest sampling cache. Setting the golden parameters to zero
(`G = 0` or `F_g = 0`) reduces it to the pure-pull eclipse `(k/N)^RF` — the regime our first
experiments target.

**Erdős–Rényi closed forms.** For random graphs of the Erdős–Rényi family, closed forms are available
(in the `N → ∞` limit) for the metrics that matter to us: per-target eclipse rate, adversary tolerance
`k_max(ε)`, end-to-end coverage, the coverage distribution, connectivity/partition as a function of
network size, and the longest hop path (graph diameter). These describe the **honest** dissemination
metrics well in the asymptotic limit.

Together, M2 and the ER closed forms cover the honest and silent-uniform-adversary cases analytically.
The prototype's job around those is validation; its unique contribution lies beyond them.

## 3. What the prototype adds beyond the analytics

For the metrics with closed forms, the prototype contributes:

- **Finite N.** The closed forms are asymptotic (`N → ∞`); real deployments are finite, and the
  finite-N behaviour — especially near thresholds — is what the prototype measures directly.
- **The actual topology, not ideal ER.** The protocol does not build `G(n,p)` graphs. Each node dials
  a fixed RF upstreams, so the graph is a **k-out random digraph** — regular out-degree, not ER's
  independent Poisson edges (and a node's RF picks are without replacement). On top of that there is a
  golden hub tier (a heavy in-degree tail), non-uniform membership, and — once serving caps are on —
  edge formation that depends on other nodes' load through rejection back-fill. The prototype measures
  this realised topology; the ER results idealise it, and quantifying the gap is itself a result.
- **Distributions and correlation, not just means.** A closed form usually gives an *expected value*
  — expected coverage, or expected missed-node count — but the mean hides the spread. Two
  configurations with the *same* 95% mean coverage can behave oppositely: one delivers ~95% on every
  run, another delivers 100% most runs but occasionally collapses to 40%. For a dissemination layer the
  operative question is the bad tail — the chance a message reaches fewer than, say, 80% of nodes —
  which the mean cannot express. Running the model across many seeds gives the full distribution, not
  just the average.

  Correlation compounds this. To turn a per-node eclipse probability into a whole-network statement the
  analytics assume independence (the union bound, ≈ N·ε missed). But failures are not independent:
  reception is a path property — a node receives only via its upstream, and *its* upstream, back to the
  publisher — so a single cut orphans an entire downstream cluster at once. Independence predicts the
  missed-count is tightly concentrated around its mean; the reality has fat tails. Same 5% mean, two
  different stories: independence says "≈5% miss, scattered, roughly 5% every run," while the realised
  graph says "usually 0% miss, but occasionally a branch is cut and 30% miss together." The prototype
  measures that clustering and the resulting outage-size distribution — the operational risk that the
  mean and the union bound cannot show.
- **Mechanism, not just rate.** For every node that fails to receive, the prototype knows *why* — its
  in-edges were all adversarial, it sat in an unreachable region, or the golden tier missed it — so a
  coverage shortfall decomposes into causes. No closed form provides this.

Beyond the closed-form metrics, the prototype is the **only** tool for:

- adversaries that are not silent and not uniformly placed (selective dropping, withholding,
  equivocation, coordinated targeting);
- multi-round dynamics (healing across epochs, an attacker compounding its effect, budget depletion
  under slashing);
- heterogeneous and churning populations.

## 4. How behaviour is modeled (brief)

Node behaviour is factored into three injected strategy seams — **connection** (which upstreams to
dial), **acceptance** (which inbound requests to admit), and **fan-out** (which downstream to forward
to). Honest behaviour is expressed as strategy instances; the bounded-topology strategies land in
feature 005-peer-view:

- **Connection:** `ConnectToAllCandidates` (the original full-mesh default, being superseded) and
  `HashGatedConnection` — a bounded, verifiable, hash-derived selection whose `target_degree` is the
  pull fan-out RF. Selection is reproducible from a per-network `genesis` seed.
- **Acceptance:** `AcceptFromAllCandidates` and `VerifiableBoundedAcceptance`, which caps admitted
  connections (`target_degree` plus a `cap_buffer`) and signals over-capacity with a rejection the
  dialer back-fills.
- **Fan-out:** `ForwardToAll`.

Adversaries come in two tiers. Many are expressible as a **hostile strategy bundle** on an
otherwise-honest node — a silent relay (forward to none), a selective dropper, or a node that refuses
to serve — which covers the M2-style attacks and the bounded-acceptance flooding case. Others require
**coordinated adversaries with shared state and greater behavioural freedom** — protocol-violating
behaviour such as equivocation, indiscriminate flooding, or Sybils coordinating on a target. A design
for that coordination model exists and is **out of scope for this document**; here it is enough to note
which experiments need it.

## 5. Experiment program (proposed order)

The stages are ordered so each builds on the previous one, and so the earliest results need the least
new machinery. Stages 1–3 run on the bounded topology and strategy seams of 005-peer-view with no new
protocol feature; later stages add richer adversary machinery, connection rotation, the golden push
feature, and enforcement in turn.

Where an attacker's optimal move and its effect are analytically obvious, we **document the attack and
its bound rather than simulate it**; simulation is reserved for configurations where a defence produces
non-trivial dynamics. Several Stage-3 cells fall into that documented category.

### Stage 1 — Honest pull dissemination (M2 with golden parameters zero)

**Goal.** Characterise the honest pull topology with no adversaries and no golden tier — M2 at `G = 0`,
`k = 0`.

**Experiments.**
- **E1 — Coverage and partition vs network size.** Sweep N at fixed RF; measure the fraction of nodes
  reached and the onset of partition (where RF must grow to keep the graph connected).
- **E2 — Propagation depth.** Measure the longest hop path (rounds to full coverage / delivery-tree
  depth) vs N and RF.

**Vs analytics.** Both metrics have ER closed forms. These experiments are **finite-N and
real-topology validation**: do the asymptotic connectivity threshold, coverage, and diameter hold at
realistic N, and for the hash-gated, degree-balancing topology rather than an i.i.d.-edge ER graph?

**Prototype adds.** The finite-N answer, the deviation of the realised topology from ideal ER, and the
depth distribution (not just the mean diameter).

### Stage 2 — The M2 attack: silent adversaries

**Goal.** Reproduce M2's eclipse result on the pull layer and measure its end-to-end consequence.

**Experiments.**
- **E3 — Per-target eclipse rate.** Fraction of nodes whose RF pull-upstreams are all adversarial;
  compare to `(k/N)^RF`.
- **E4 — Adversary tolerance `k_max(ε)`.** Sweep k; find where the eclipse rate crosses ε; compare to
  the M2 tolerance formula (bulk ε only — Monte-Carlo cannot reach security-grade tails).
- **E5 — End-to-end coverage under silent adversaries.** The fraction actually reached after full
  multi-hop propagation, its distribution, and the per-node cause decomposition.

**Vs analytics.** E3/E4 confirm M2 (`G = 0`). E5's mean has an ER percolation closed form.

**Prototype adds.** Confirmation at finite N; and for E5 specifically, the **coverage distribution and
spatial correlation** (which the union bound misses) and the **mechanism decomposition** — separating
pull-layer eclipse from unreachable-region failures. M2 gives the one-hop eclipse probability; the
prototype gives the realised end-to-end coverage the eclipse rate only bounds.

### Stage 3 — Bucketed selection and bounded acceptance (separable layers)

**Goal.** Study the two topology-hardening knobs **separately**: a per-round bucketed pull-permission
predicate on the connection side, and a serving cap on the acceptance side. They are not one layer —
they defend against different things. Bucketing constrains *sampling* (and an attacker's ability to
target); the serving cap bounds *fan-in*, which is what creates the surface a flooding attacker tries
to exhaust. Both are strategy-level (no new protocol feature); the flooding attacker is a hostile
connection strategy.

**Experiments.**
- **E6 — Bucketing, no cap.** A node may dial only same-bucket candidates; acceptance stays unbounded.
  Re-run the eclipse/coverage metrics (E3/E5) with bucketing on vs off, sweeping the bucket count B.
  The bucketed-pull analysis predicts the eclipse *fraction* is unchanged at the balanced B and shifts
  off-balanced — confirm, then explore off-balanced.
- **E7 — Serving cap, no bucketing (honest).** Bounded acceptance with uniform selection and no
  attacker. With uniform dialing every node is dialed ~RF times on average, but the serving load
  varies; when the cap sits close to RF, nodes in the upper tail of that (chance) variance reject some
  honest dialers, who then back-fill. Measure whether that reduces effective in-degree and coverage
  against the uncapped baseline, and how it depends on the cap headroom above RF (the variance buffer).
- **E8 — Flooding mitigation under the cap.** With a serving cap and bucketing, adversarial Sybils dial
  a victim to exhaust its slots and starve honest requests. Measure the concentration reduction (toward
  ≈ K/B) and honest starvation. (The no-bucketing case is the obvious attack, documented below.)

**Vs analytics.** The bucketed-pull analysis gives the concentration reduction from bucketing and the
cap relationship; E6/E8 confirm and extend it at finite N. E7's congestion effect has no closed form.

**Prototype adds.** The separated effect of each knob, finite-N behaviour, and the interaction with the
rejection/back-fill dynamics.

**Obvious cases — documented, not simulated.**
- **No cap ⇒ no flooding surface.** Without a serving cap there are no slots to exhaust, so a flooding
  attacker gains nothing; recorded as a non-attack.
- **Cap without bucketing ⇒ the obvious attack.** With no per-round gate, the optimal attacker requests
  the victim from every Sybil, so concentration rises to ≈ K (bounded by the cap) and honest requests
  are crowded out as K approaches the cap. The move and its bound are obvious; we document them as the
  baseline vulnerability and reserve simulation for the bucketing mitigation (E8).

**Open decisions:** D1 (bucket smaller than target degree, E6) and D2 (dialer response to rejection,
E7) — see §7.

### Stage 4 — Richer adversarial and dynamic behaviour

**Goal.** Move past the silent, uniform, single-round adversary to behaviours the analytics cannot
express: active (non-silent) adversaries, multi-round dynamics, and heterogeneous populations. Active
adversaries are strategy-level; the multi-round experiments additionally need connection rotation (the
current setup only adds connections).

**Experiments.**
- **E9 — Active adversaries.** Coverage under selective dropping and message withholding rather than
  pure silence.
- **E10 — Multi-round healing and compounding.** Whether coverage recovers across epochs as the
  topology re-samples, and whether a persistent adversary compounds its effect. *Requires connection
  rotation.*
- **E11 — Heterogeneous populations and churn.** Per-node RF, mixed behaviours, and nodes
  failing/joining over time.

**No coordinated-eclipse experiment.** Eclipsing a *receiver* offers no coordination lever under
grinding-resistant selection: the victim picks its own upstreams uniformly from a set the attacker
cannot bias, so adding Sybils only raises the aggregate probability `(k/N)^RF` already measured by
E3/E5 — a quantity, not a coordinated attack. Attackers coordinate their own actions, not the victim's
picks; the one would-be targeting lever, grinding identities into the victim's selected set, is
precisely what the hash-gated selection defeats. The only genuinely coordinated receiving-side attack
is serving-slot flooding (E8).

**Vs analytics.** No closed form. **Prototype adds:** these results are only obtainable by simulation.

### Stage 5 — Golden nodes (push tier)

**Goal.** Introduce the golden push tier and study the full M2 model (push and pull together), then
re-run the adversarial experiments with goldens present. *Requires the push-based connection feature
(golden-connection-flow); a publisher delivers a message to a golden by publishing on it directly.*

**Experiments.**
- **E12 — Full M2.** Re-run E3–E5 with `G > 0`, `F_g > 0`; confirm the push × pull factorisation and
  measure end-to-end coverage, depth, and mechanism with the golden hubs present.
- Re-run the Stage-4 adversarial experiments with a golden tier, to measure how much the push tier
  restores coverage under attack.

**Vs analytics.** E12 confirms the full M2 closed form; the adversarial re-runs have no closed form.

**Prototype adds.** Finite-N confirmation of the full model, and — since goldens are hubs — their effect
on propagation depth and on coverage under attack, which the per-target formula does not express.

### Stage 6 — Deposit and slashing dynamics

**Goal.** Study enforcement as a feedback loop: provable misbehaviour is slashed, the offender is
removed from the registry, and after synchronisation other nodes stop accepting from it — so the
effective adversary count decays over time. *Requires provable-misbehaviour detection (e.g.
equivocation), connection rotation, and a deposit field on registry entries; modelled at the level of
consequence (removal) and cost (deposit burn), without on-chain proof cryptography.*

**Experiments.**
- **E13 — Slashing dynamics.** Effective-adversary-count decay across epochs, enforcement-driven
  healing of coverage, and attacker budget depletion (a fixed budget funds a bounded number of provable
  misbehaviours). Deposit sizing itself is a derived calculation from the tolerance and concentration
  curves measured in Stages 2–3, not a separate simulation.

**Vs analytics.** Deposit sizing is arithmetic over earlier measured curves; the dynamics have no closed
form.

**Prototype adds.** The entire dynamic story. A structural point frames it: slashing removes only
*provable* misbehaviour, so a rational attacker is pushed toward unprovable silence — meaning the
residual threat that survives enforcement is exactly M2's silent adversary. M2 is therefore the floor;
enforcement handles everything provable above it.

## 6. Summary table

| # | Experiment | Stage | Analytical reference | What the prototype adds |
|---|------------|:-----:|----------------------|-------------------------|
| E1 | Coverage & partition vs network size | 1 | ER closed form | Finite-N + real (non-ER) topology validation |
| E2 | Propagation depth (longest hop path) | 1 | ER diameter | Finite-N; depth distribution, not just the mean |
| E3 | Per-target eclipse rate | 2 | M2 (`G=0`) | Finite-N confirmation |
| E4 | Adversary tolerance `k_max(ε)` | 2 | M2 | Finite-N confirmation (bulk ε) |
| E5 | End-to-end coverage under silent adversaries | 2 | ER percolation (mean) | Distribution + correlation + per-node mechanism |
| E6 | Bucketing, no cap: eclipse/coverage vs B | 3 | Bucketed-pull (balanced B) | Off-balanced + finite-N; surfaces D1 |
| E7 | Serving cap, honest: coverage under bounded fan-in | 3 | none (congestion) | Effect of cap + back-fill on coverage; surfaces D2 |
| E8 | Flooding mitigation under cap (bucketing) | 3 | Bucketed-pull concentration | Finite-N K/B; interaction with back-fill |
| E9 | Active adversaries (drop / withhold) | 4 | none | Simulation-only |
| E10 | Multi-round healing & compounding | 4 | none | Simulation-only (needs rotation) |
| E11 | Heterogeneous populations & churn | 4 | none | Simulation-only |
| E12 | Full M2 with golden tier | 5 | M2 (full) | Finite-N confirmation; hub effect on depth & coverage |
| E13 | Slashing dynamics | 6 | Deposit sizing (arithmetic) | The entire dynamic story |

Two documented (not simulated) cells sit alongside Stage 3: flooding with **no serving cap** (no
surface to exhaust) and flooding with a **cap but no bucketing** (the attacker trivially requests the
victim from every Sybil, concentration ≈ K).

Reading the table: the confirmation experiments (E1–E6, E8, E12) validate closed forms — ER, M2, or
the bucketed-pull bound — at finite N and for the topology the protocol actually builds. The remainder
(E7, E9–E11, E13) is where the prototype is the **primary instrument**: the questions have no closed
form.

## 7. Open decisions

- **D1 — Bucket smaller than target degree** (Stage 3): accept fewer upstreams, widen the bucket, or
  carry the deficit forward.
- **D2 — Dialer response to rejection** (Stage 3): iterate back-fill until U upstreams, or cap the
  rounds and accept fewer than U. Determines what "a completed connection round" means under bounded
  acceptance.
- **Prerequisite — connection rotation** (Stages 4 and 6): the current connection setup only adds
  connections. Multi-round healing, compounding, and slashing removal all need the topology to re-sample
  (drop and re-dial) across epochs.
- **Prerequisite — golden push feature** (Stage 5) and **provable-misbehaviour detection** (Stage 6).

## 8. Next steps

Review and adjust the experiment list and its order. Once agreed, we translate the accepted stages into
an implementation plan — the experiment/testing framework (topology builder, metrics, sweep harness),
the adversarial strategies and coordination model, connection rotation, and the golden push feature,
sequenced to match the stage order above.

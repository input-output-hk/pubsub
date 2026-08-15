# Gated-symmetric selection — the hash gate under the bidirectional handshake

The first empirical pass over the (bucket count, pick count, symmetric)
plane point: the verifiable hash gate composed with the ADR 0034
symmetric handshake — "gated M4". No published model covers this point
(the N-039 boundary): the bucketed-pull analysis treats directional
pull, and the formal M4 model is ungated. The cells therefore test
closed-form predictions derived here, recorded per cell in the config
comments before any cell ran. Like the model comparisons, this document
**informs, it does not gate**; the statistical conventions (raw counts +
Wilson 95 %, and why not ±1σ) are documented in the
[M2 comparison](m2-comparison.md) §4 and apply unchanged.

## Provenance

| | |
|---|---|
| Tool commit | `7da261e` (nine cells) / `95dfac5` (the μ-axis pair) / `1ec648b` (the anchor re-run) — config- and docs-only successors of `3887ea7`; the instrument binary is unchanged across them |
| Configurations | [`configs/experiments/gated-symmetric/`](../../configs/experiments/gated-symmetric/) — eleven cells, master seeds 1101–1111, N = 4 000, K = 16 throughout; μ = 0.2 except the two μ-axis cells (§3) |
| Anchors | the validated ungated M4 structure point [`m4-uniform-symmetric.toml`](../../configs/experiments/m4-uniform-symmetric.toml) (seed 44, re-run at `1ec648b` for the standing-degree readout — the recorded `d7e7132`/`23d0223` baselines predate that column; the re-run is value-identical to them on every shared aggregate field) as the B → small degree endpoint; a fresh ungated pick-8 twin (seed 1109) as the matched-degree tail control |
| Predictions | [`gated_symmetric_predictions.py`](gated_symmetric_predictions.py) — exact binomial arithmetic, no Poisson or large-r approximations |
| Timings | 500-run gated cells ≈ 6 min at 12 workers; the two 8 000-run tail cells 93 min each; the ungated 8 000-run twin ≈ 8 min (the gate is the cost: ~16 M SHA-256 edge evaluations per run, as in [E10](e10-selection-fidelity.md) §5) |
| References | N-039 (the pair-draw design and its recorded assumptions — this pass is its revisit trigger), ADR 0034 (symmetric handshake), ADR 0039/0040 (unified selection plane, seed derivation), [E10](e10-selection-fidelity.md) §6–§7 (why nothing there transfers to M4) |

Artifacts are reproduced byte-for-byte from the tool commit and master
seeds (worker count is a wall-clock choice, not part of the contract).

## 1. The pair-draw geometry: B enters the degree

The symmetric gate hashes the **unordered pair** under `edge-sym/v1`
(N-039), so both ends of every candidate edge compute the same verdict:
a node's survivor pool — mean λ = (N−1)/B — is simultaneously *who it
may pick* and *who may pick it*. Picks and pickers share one random
object. With survivor headroom r = λ/K, the realised mean degree is

> d = λ·m·(2−m), m = E[min(K, p)/p] over pool sizes p

(m ≈ 1/r at r ≫ 1, m → 1 at r ≤ 1): own picks λm, inbound picks λm,
minus the mutual-pick overlap λm² — the reciprocation term ≈ K/r that
the directional families put at ≈ K²/N instead. Two structural
consequences, both absent from E10's directional plane:

- **There is no plateau.** In the directional family B is invisible at
  r ≥ 2 (the law-exact regime); here B shapes the degree at every r —
  d runs from 2K at B → 1 down through 1.5K at r = 2 to K at r = 1, and
  below r = 1 the pick count stops mattering entirely (nodes link their
  whole pool mutually: d = λ).
- **Gate-only symmetric measures the pair density directly**: every
  survivor dialed, both ends agree, so realised degree = the pool
  exactly.

Measured, 500 runs per cell (the ungated anchor is the shipped seed-44
configuration, re-run at this pass's tool line — the recorded baselines
predate the standing-degree column):

| cell | r | d predicted | d measured |
|---|---|---|---|
| gated picks B = 10 | 25.0 | 31.36 | **31.361** |
| gated picks B = 50 | 5.0 | 28.80 | **28.798** |
| gated picks B = 125 | 2.0 | 24.00 | **23.996** |
| gated picks B = 167 | 1.5 | 21.26 | **21.258** |
| gated picks B = 250 | 1.0 | 15.84 | **15.839** |
| gated picks B = 500 | 0.5 | 8.00 | **7.996** |
| gate-only B = 125 | — | 31.99 | **31.985** |
| gate-only B = 250 | — | 16.00 | **15.997** |
| ungated anchor (K = 16) | — | 31.94 | **31.937** |

The shared-pool degree law holds to three digits across the ladder,
including the pick-truncation region at the r = 1 boundary (own picks
14.41 predicted at B = 250 — pools smaller than K truncate). Own picks
are themselves on the ledger — the dial ledger (`dial_sends` over
N × dial waves) reads 15.999 at B = 125 and 14.414 at B = 250 against
the predicted 16.00 and 14.41 — so the mutual-pick overlap is measured
arithmetic on two observables, 2·out − d: 8.002 at B = 125 (predicted
8.00) and 12.988 at B = 250 (predicted 12.99).

## 2. The two-channel isolation law

An honest node is stranded when it holds no honest link. Under the pair
draw that event has two channels with different structure:

- **Channel A — the empty pool**: the node's survivor pool contains no
  honest member at all. Probability e^(−(1−μ)λ) — and note what the
  exponent does *not* contain: K. If the gate admits no honest peer, no
  pick budget in either direction can rescue the node. This channel is
  the pair draw's genuinely new failure mode; it does not exist in any
  directional family, where the inbound side draws independent coins
  over the whole population.
- **Channel B — honest members present but unlinked**: all K own picks
  land on adversarial pool members *and* every honest pool member's
  picks miss the node. This keeps ungated M4's two-independent-factor
  shape and converges to exactly the ungated law μ^K·e^(−K(1−μ)) as
  r grows.

The channels cross at λ = K·(ln(1/μ) + (1−μ))/(1−μ) — at this shape
λ ≈ 48, r ≈ 3 (B ≈ 83). Above that headroom the gate is
coverage-free at the same K; below it channel A dominates and grows
explosively as the pool shrinks. Setting channel A below a per-epoch
failure target δ gives the **pool floor**

> λ = (N−1)/B ≥ ln(H/δ)/(1−μ)

— at N = 4 000, μ = 0.2: pool ≥ 21.6, i.e. B ≤ 185 for δ = 10⁻⁴
(B ≤ 214 at 10⁻³, B ≤ 163 at 10⁻⁵). Unlike E10's soft r ≥ 2 headroom
rule, this floor cannot be traded against RF: raising K at fixed B
lowers r without touching channel A.

## 3. The tail cells: correlated law vs the naive transfer

At B = 250 (r = 1, d ≈ 16) the two candidate readings separate by
~520×: the two-channel law predicts P(bad) = 0.0086 (channel A is
essentially the whole tail there), while the ungated M4 law read at the
realised degree — the natural naive transfer, two independent factors
at RF = d/2 — predicts 1.65×10⁻⁵. 8 000 runs decide:

| cell | d measured | bad/runs | P(bad) | Wilson 95 % | predicted |
|---|---|---|---|---|---|
| gated picks B = 250 | 15.839 | **57/8 000** | 0.00713 | [0.0055, 0.0092] | 0.0086 (z = −1.4) |
| gate-only B = 250 | 15.997 | **61/8 000** | 0.00763 | [0.0059, 0.0098] | 0.0086 (z = −1.0) |
| ungated twin K = 8 | 15.984 | **0/8 000** | 0 | [0, 4.8×10⁻⁴] | 1.32×10⁻⁵ |
| gated picks B = 500 | 7.996 | 500/500 | 1.0 | [0.992, 1.0] | 0.995 |

Three readings:

- **The naive transfer is rejected, not merely disfavoured**: 57
  observed against ~0.13 expected is a ~430× excess. The side-by-side
  with the twin makes it a raw-count comparison at equal measured
  degree — ungated 0/8 000, gated 57/8 000 — with no closed form on
  either side of the headline.
- **The pick rule is irrelevant to the tail**: gate-only and gated
  picks at B = 250 land statistically identical (61 vs 57), pinning the
  channel on the pair-draw geometry itself.
- **Isolated-node dominance is exact at this depth**: every bad run in
  the picked cell strands exactly one node (7 943 : 57 : 0 across
  missed counts 0/1/2); the gate-only cell shows a single 2-node event
  in 8 000 — the expected second-order trace. At B = 500
  (P(bad) ≈ 0.995) the per-run stranded count distributes as the
  predicted Poisson at mean E_iso = 5.3, mode 5.

Both 8 000-run points sit ~1σ below prediction — consistent, with the
same mild optimism the formal folder's isolated-vertex laws show in
reverse; nothing at this depth distinguishes sampling noise from a
higher-order correction.

### The μ axis

The empty-pool exponent carries the (1−μ) the pool floor quotes; two
further cells at the same pool (B = 250, K = 16) raise μ in steps of
0.1 — each step multiplies per-node isolation by e^(0.1·λ) ≈ 4.95
(4.97 in the exact arithmetic) —
with run counts sized for ~74 expected bad runs per point (equal
statistical weight across the axis):

| μ | bad/runs | P(bad) | Wilson 95 % | predicted |
|---|---|---|---|---|
| 0.2 | 57/8 000 | 0.00713 | [0.0055, 0.0092] | 0.0086 |
| 0.3 | 75/2 000 | 0.03750 | [0.0300, 0.0468] | 0.0369 |
| 0.4 | 76/500 | 0.15200 | [0.1232, 0.1861] | 0.1479 |

Per-node isolation follows the exponent across the axis (stranded
honest nodes over honest-node-runs: 2.23×10⁻⁶ → 1.34×10⁻⁵ → 7.00×10⁻⁵
vs predicted 2.70×10⁻⁶ → 1.34×10⁻⁵ → 6.67×10⁻⁵). Two structural checks ride along: realised degree is
μ-invariant as the class-blind gate requires (15.839 / 15.837 / 15.840
across the axis), and at μ = 0.4 — where E_iso = 0.16 makes two
independent isolation events per run likely enough to see — the
stranded-count histogram shows 8 two-node runs among 76 bad against ~6
expected, the second-order term appearing exactly on schedule.

## 4. The B design rule under the symmetric handshake

For the protocol's choice of B on a gated bidirectional configuration
(μ = 0.2 shape; scale via the closed forms):

- **Size the pool, not the headroom**: (N−1)/B ≥ ln(H/δ)/(1−μ) is the
  binding constraint (B ≤ 185 at this scale, δ = 10⁻⁴). RF cannot
  substitute — channel A is K-independent.
- **At r ≳ 3 verifiability is coverage-free** at the same K, with the
  same degree ≈ 2K as ungated M4 — no RF premium exists anywhere in
  the safe region, unlike the directional gate-only mode's +1-link
  compensation (E10 §4).
- **The degree itself is B-dependent everywhere**, so the E12-side
  benefit accounting (attacker slot concentration ≈ K/B grows with
  large B) trades against a *hard* floor rather than E10's soft
  headroom rule. At this shape the window is roughly B ∈ [83, 185]:
  coverage-free below 83, floor-compliant to 185, and a cliff beyond it
  that no other knob can pay back. The E12 flooding grid was
  directional; its benefit side is not re-measured here (caps are
  deliberately out of scope — N-032, N-040).

### The ordered-predicate alternative, priced

N-039 records the rejected construction — the directional (ordered)
gate on the dialer with reciprocity constructed on accept — and lists
its costs qualitatively. The measurements complete the comparison,
because both of this pass's headline effects are faces of one fact:
the unordered pair flips **one coin per pair** where the ordered
construction flips **two independent coins** (one per direction, the
edge forming if either passes). One coin concentrates the randomness:
that concentration is what creates the empty-pool channel (a single
bad draw kills both directions — the 57/8 000) *and* what keeps the
adversarial accounting tight (one admissibility chance per identity
per victim). Two coins relax both at once: isolation returns to the
two-coincidence product law (the twin's 0/8 000 shape, RF-repairable
at any B since K is in both exponents), and a Sybil reaches a victim
if either direction's draw holds — admissibility ≈ 2/B, halving the
identity cost per victim. On the two axes (this shape; the ordered
column is closed-form arithmetic from the measured laws, not a
measured cell):

| construction | bad-graph tail | Sybil admissibility per identity per victim |
|---|---|---|
| ungated M4 | 5×10⁻¹⁴ at K = 16 | 1 — anyone may dial anyone |
| ordered gate | ≈ ungated at every B; RF-repairable | ≈ 2/B |
| unordered pair | ≈ ungated for B ≲ 83; floor at B ≈ 185; K-independent cliff beyond | 1/B |

Inside the operating window (pool ≥ ~3K) the unordered pair therefore
**dominates**: identical coverage, a 2× tighter adversarial bound, and
the audit properties N-039 records (validity a pure function of
(nonce, topic, pair); both ends agreeing on the edge set). The ordered
construction becomes relevant only where a deployment needs gate
widths past the pool floor — there its RF-repairability is the only
way to keep coverage, at the price of the looser bound, the initiation
bit in symmetric link state, and history-dependent edge sets. Neither
rescaling repairs the other: doubling B under the ordered gate
restores total density 1/B but splits every neighbourhood into two
directed half-pools, which is a different geometry, not the unordered
design.

**The frontier is conserved across predicates.** The 2× is an
equal-B statement; at equal *coverage* it evaporates. Push either
construction to its saturation frontier at a failure target δ (every
admissible edge realised) and coverage pins the honest admissible
degree at λ_floor = ln(H/δ)/(1−μ), while the per-identity Sybil
admissibility toward a victim is the total pair admissibility
d/(N−1) — whatever coin structure produced it. The tightest bound
either predicate can reach is therefore the same number,
λ_floor/(N−1) (≈ 21.6/3999 ≈ 0.0054 at this shape): the unordered
pair reaches it at B ≈ 185, the ordered gate at B ≈ 370, twice the
width at half the per-coin density. Coverage fixes the admissibility
budget and the adversary holds the μ-fraction of it; the predicate
choice decides which knob moves along the frontier (B — a consensus
parameter inside the predicate — for the unordered pair; RF — a local
dial, until picks saturate the half-pool — for the ordered gate), not
where the frontier lies.

## 5. N-039 assumption verdicts

The pass is N-039's recorded revisit trigger; its three assumed
properties of the pair draw, checked:

- **Per-pair 1/B density**: measured directly by gate-only B = 125
  (degree 31.985 vs (N−1)/B = 31.99) and B = 250 (15.997 vs 16.00).
- **Both-ends agreement**: the degree law and the gate-only
  degree-equals-pool identity both fail without it; held to three
  digits everywhere.
- **Adversarial occupancy**: pool composition enters every isolation
  constant through (1−μ); the 0.0086 prediction matched twice at
  8 000 runs.

What the assumptions did *not* cover — the empty-pool channel and its
RF-independence — is this document's §2–§3.

## 6. Instrument notes

- Worker counts varied across cells (16 → 8 → 12) for thermal and
  power reasons; artifact bytes are worker-count-independent by the
  framework's determinism contract.
- The `standing_degree_mean` run column (whole-population standing
  relay links) is the degree observable; under the symmetric handshake
  it counts each mutual edge once per endpoint, exactly the d of §1.
  The per-node detail slot columns are deliberately not read on these
  cells (direction-blind under the symmetric handshake — N-040).
- The prediction script computes every number in this document from
  exact binomial sums; the hypergeometric avoid-term and the
  min(K, pool) truncation matter at the r = 1 boundary (d = 15.84, not
  16.00 — visible at 500-run precision).

## 7. Scope

Single topic, N = 4 000, μ = 0.2, oblivious adversaries (N-037), open
acceptance throughout — the symmetric × capped combination stays with
N-032/N-040. The directional families' E10/E12 results do not transfer
here and vice versa: this family has no plateau, no eclipsed-receiver
class resurrection (its analogue is the empty-pool channel), and its
B trade-off has a hard floor where theirs has a soft rule. The
adaptive-adversary and epoch-rotation questions (pool re-draws under a
fresh nonce) are untouched.

The benefit-side grid — the E12 analogue under the symmetric
handshake, measuring what the 1/B admissibility bound delivers against
a flooding adversary — is the natural paired follow-up and is *not*
runnable as a config-only pass: its measurand needs the cap semantics
of N-032 resolved (what a cap bounds on a symmetric node) and the
detail slot columns of N-040 made direction-aware or re-scoped, which
is exactly the machinery decision those notes' triggers anticipate.

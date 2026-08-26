# Symmetric flooding — the admissions budget under the pair-draw flooder

The benefit side of the gated-symmetric design of record: what the hash
gate's 1/B admissibility bound and an acceptance cap deliver against a
Sybil flooder on the bidirectional handshake — the E12 question
re-asked where E18 measured the coverage cost. The pass carries the
machinery its own question required: ADR 0042 resolved N-032 (the cap
on a symmetric seam is an **admissions budget**), and N-040's
drain-time route attribution made the answer measurable.

Population N = 4 000, honest pick count K = 16 over the symmetric pair
gate at width B; the adversarial class is S Sybil flooders — silent
relays on the symmetric handshake at the same pinned B, no pick count
(every admissible pair dialed), uncapped acceptance — so S doubles as
the ambient adversarial count (μ = S/N), the E12 convention. 400 runs
per cell, per-node detail on; probabilities as counts + Wilson 95 %;
means as run-clustered mean ± SE (per-victim rows within a run are
correlated, so the run is the replicate).

## Provenance

| cells | seeds | tool commit (instrument) |
|---|---|---|
| scheme-A contrast (1 cell) | 1112 | `2eaded3` (instrument `9197868` — the pre-ADR-0042 both-role scan; not re-runnable at HEAD) |
| budget twin | 1113 | `9865b96` |
| pilot | 1114 | `ea2fbe0` (instrument `9865b96`) |
| core grid (16), degeneracy pair (2), μ cell (1) | 1115–1133 | `01d79a3` (instrument `9865b96`) |
| μ replicate | 1134 | `8871ed1` (instrument `9865b96`) |
| ordered arm: two tails + flooder mirror (ADR 0043) | 1135–1137 | `1dd8722` (instrument `6d0385b`) |
| capsweep quiet-end anchor (1 cell) | 1138 | `c0f66a4` (instrument `6d0385b` — the intervening commits are docs/tests-only) |

Configs in `configs/experiments/symmetric-flooding/`; per-cell
predictions recorded in the config comments before each cell ran
(`symmetric_flooding_predictions.py`); fold + identity gates in
`summarise_symmetric_flooding_cell.py`. Byte-identity chain for every
instrument commit: `23d0223`/`21acd36` → (E13 standing-links fields
only) → `b294391` → (exact) → `9197868` → (exact) → `9865b96` — all
seven baseline sweeps, recorded in `notes/experiments-baselines/`.
Artifacts reproduce byte-for-byte from tool commit and master seed (the
scheme-A cell from its pinned instrument commit only). The instrument
chain closes at `6d0385b` (the ADR 0043 predicate), also byte-identical
across all seven baseline sweeps; the gated-symmetric path — absent
from those sweeps — was attested separately: E18's committed
`gated-picks-b250-mu40` cell re-run at HEAD is byte-identical to the
E18 session's artifacts, re-anchoring the uncapped μ = 0.4 measurement
(76/500) under this branch's binary — §6's comparison is
same-instrument on both sides.

## 1. The two-scheme contrast: what a refusal is allowed to destroy

ADR 0042 replaced the recorded 015 behaviour (the cap scanning the
both-role mirrored link set) with the admissions budget: fresh peer
arrivals spend it, crossings — requests answering the node's own
pending dial — are exempt, the node's own picks are never vetoed, and
realised degree is bounded by K + C **by construction**. The retired
scheme was measured once at its pinned commit before the change; the
budget twin ran at equal nominal pairing (C_A = C + K):

| B = 50, S = 800 | scheme A, scan C_A = 32 | budget, C = 16 |
|---|---|---|
| Sybil edges / victim | 15.580 | 12.016 |
| honest edges / victim | 22.356 | 16.661 |
| Sybil proportion | 41.1 % | 41.9 % |
| crossings refused / victim | **0.0743 ± 0.0003** | **0 (exact)** |
| max degree observed | 32+ (order-dependent overshoot) | 32 = K + C (never exceeded) |

Two structural results:

- **The veto channel is real and asymmetric.** Under the scan, 95 053
  refusals across the cell (0.074/victim) hit crossings — edges the
  victim's own selection wanted. Sybils accept everything, so only
  picks on honest mutual partners die this way. Under the budget the
  count is identically zero, over ~10⁷ refusals across the whole grid.
- **The admitted composition is scheme-independent.** Both schemes
  admit by the same fair race, so the Sybil proportion is the same
  41–42 % — the cap semantics decides how much gets in and what
  refusals destroy, never the class mix. The security gain of the
  budget is the sharp invariant (≤ C non-chosen edges, degree ≤ K + C)
  and the closed veto channel, not a composition change.

**Pairing caveat.** C_A = C + K equalises effective budgets under a
uniform-interleaving model of the scan race that the measurement
falsified: in the wavefront driver all requests land in wave 0 and all
mirrors (Accepted replies) in wave 1, so the scan races **arrivals
only** against the full C_A — refusals are the tail
E[(arrivals − C_A)⁺] (measured 0.925/victim vs 0.926 from the corrected
form, printed by the prediction ledger as `wave_refused_all`; the
pre-registered interleaving figure of 8.23 was wrong and stands in the
config comment as the recorded miss). The equal-effective
pairing under the corrected model is C_A ≈ C + crossings (≈ 22 at this
shape); the cell pair as run compares a loose scan against a tight
budget, which strengthens rather than weakens the veto finding (even a
barely-binding scan vetoes measurably).

## 2. The race law: pilot-calibrated, then nineteen cells at zero
   flags

The budget race is the E12 fair-arrival contention model enumerated
exactly over the joint pool, with the pick split **without
replacement**: given a pool of h honest + a Sybil members the node
draws exactly min(K, h+a) distinct members, so its Sybil picks are
hypergeometric and the fresh Sybil load is deterministic given the
split (every unpicked admissible Sybil dials). The pilot calibrated
this form — a per-member binomial approximation missed at second order
(admitted_s z = −16 at B = 125, where pick-fraction variance is
large) — and the calibrated model then reproduced both already-measured
cells within ~1 SE with no refit.

The 19-cell grid (B ∈ {50, 125} × cap ladders {16, 20, 26} / {10, 12,
16} × S ∈ {40, 400, 800}, plus the B = 250 cells) against the frozen
config-comment predictions: **zero flags** — every z across the six
model-priced columns per cell (the two admitted routes, the two mutual
routes, own-only, and the refusal rate; the fold tracks eleven columns,
the other five being sums and identities of these) within |z| ≤ 2.6,
every accounting identity exact, `refused_crossing ≡ 0` everywhere, max degree ≤ K + C in every
cell, and all sixteen B ∈ {50, 125} cells 400/400 good as predicted.
The per-victim identity own-only = admitted (every honest fresh refusal
kills one admitted edge at the victim and one pick at the dialer) held
to the fourth decimal in every cell.

## 3. The occupancy decomposition: the gate divides only half the
   surface

Per honest victim, Sybil occupancy = **floor + admitted**:

- **The floor — own picks on Sybils — is B-independent in composition
  and cap-blind**: realised picks × (Sybil pool share), ≈ K·μ wherever
  picks don't saturate the pool. Measured 0.16 → 3.20 → 5.76 across
  S = 40 → 800 → 1600 — the ambient share exactly; the prefactor is K
  at B ∈ {50, 125} and E[min(K, pool)] = 14.4 at the saturated B = 250
  (hence 5.76 rather than K·μ = 6.4 — the ledger's exact `mutual_s`).
  No acceptance policy sees it; only pool composition (μ) moves it. This is the structural difference from directional E12, where
  the entire attacker surface was admission-gated: under the symmetric
  handshake the victim's own selection hands the adversary its ambient
  share for free.
- **The admitted route is what the gate divides and the cap bounds**:
  fresh Sybil pressure (S/B)(1−m) raced against the budget. At B = 50,
  S = 800 it is 12.8 dials/victim; at B = 125, 3.2; at B = 250, 0.32 —
  the gate's division by B, discounted by the crossing exemption
  (1−m).

Design reading: the flooding-resistance knob is B (shrinks the
admission route) and C (bounds it absolutely); μ alone sets the floor.
An operator sizing against occupancy should read
`sybil ≈ K·μ + min(fair-race share, C·share)` (picks saturating the
pool shrink the first term's prefactor below K, never above) — both
terms measured at table precision across the grid.

## 4. Starvation and the cap-sizing rule

As in directional E12, the harm channel is starved honest links, not
slot concentration — but under the symmetric handshake every refusal
costs a **whole edge at both ends**: the victim loses an admitted edge
and the dialer loses a pick. The cap axis isolates it (per-victim
refused honest fresh dials, S = 800):

| | C = tight | C = medium | C = generous |
|---|---|---|---|
| B = 50 (caps 16/20/26) | 3.19 | 1.70 | 0.39 |
| B = 125 (caps 10/12/16) | 0.91 | 0.45 | 0.078 |
| control (S = 40), same caps, B = 50 | 0.52 | 0.075 | 0.0015 |

The rule transfers from E12 with the anchor changed: the cap must
clear the **fresh-arrival load** K(1−m)(1−μ) plus its variance — not
the both-role degree ≈ 2K (own picks and crossings never spend
budget). At the generous caps the controls lose essentially nothing
(the E12 prerequisite); every cell in the grid stayed 400/400 good
regardless of starvation, because at these shapes the honest degree
floor (own picks ≥ K(1−refusal echo) plus mutuals) keeps every node
connected — coverage damage needs the saturation regime below.

## 5. Degeneracy at saturation

At B = 250 the pool saturates (m ≈ 0.90): every load carries (1−m),
so almost nothing is fresh and flooding is structurally inert — the
20 % flooder gains 0.21 admitted edges/victim over its 2.89 floor, and
coverage matches the *uncapped* E18 empty-pool expectation (395/400
good vs ≈ 396.6 expected). The grid does not tile this regime; the
degeneracy pair confirms the (1−m) arithmetic that says there is
nothing there to measure.

## 6. The composition finding: starvation reaches the empty-pool
   channel at saturation

The one pre-registered prediction the grid refuted. The μ = 0.4
spot-check (B = 250, C = 3, S = 1600) predicted the E18 empty-pool law
untouched by the cap (P(bad) = 0.148, ≈ 59/400 bad): measured 80/400,
and the fresh-seed replicate 86/400 — pooled **166/800 = 0.208**,
z = +4.75 against the pure law and z = +2.50 against E18's own
uncapped measurement at the same geometry (76/500 = 0.152). At
saturation, with the budget tight (C = 3) and μ high, refusals strand
marginal nodes the empty-pool law alone would have spared: the
**cap × empty-pool composition term** is real, ≈ +6 pp of P(bad) at
this shape. The term factors structurally: to first order
ΔP(iso) ≈ Σ_h P(h honest pool members) · (1−σ)^h with
σ = m + (1−m)(1−ρ) — mutual edges are cap-immune, so a node dies to
the cap only where the small-pool boundary mass and the refusal rate ρ
are simultaneously large. The prediction ledger computes this form
over the joint pool (`cap_composition`; the per-(h, a) pick fraction
reduces σ to the boundary form exactly where the boundary mass lives)
and walks the whole cap ladder from the grid-validated race law
(`capsweep`), so the one measured corner anchors a computed trade-off
curve rather than an extrapolation:

- **At the corner (B = 250, S = 1600)** the computed increment at
  C = 3 is ΔE_iso = 0.059 against the measured excess 0.073 — on
  P(bad), 0.196 predicted vs 0.208 measured, z ≈ +0.8 against the
  pooled 800 runs — and across the ladder no C both binds and stays
  harmless. The cap's entire addressable surface is the 0.63-edge
  fresh route over the 5.77-edge cap-blind floor; every C that blocks
  even 8 % of it (C ≤ 6, ρ ≥ 0.08) adds ≥ 1 pp of P(bad), while by
  C = 12 both columns are dead (≤ 0.001 edges blocked, ≤ 0.03 pp
  added). This is structural, not a coincidence of the cells: one
  quantity — the pool-to-picks ratio (N−1)/B against K — drives both
  columns together, collapsing fresh loads to small all-tail integers
  (mean ≈ 1.5; C = 3 is twice the mean and still refuses at ρ ≈ 0.33)
  at exactly the widths where the boundary mass is large. Binding and
  harmful coincide past the floor by construction.
- **Inside the window the columns separate**: at B = 50 the grid's
  sizing-rule caps (16/20/26) block 4.0/2.1/0.5 fresh Sybil edges per
  victim at computed ΔE_iso ≤ 4×10⁻⁵ — the 400/400 grid rows sit on
  this curve — because the boundary mass carries channel A's own
  exponent and loads ~10+ give the √-headroom real room (ρ measured
  10⁻⁴ at the generous controls). The guarantee stays conditional on
  the sizing rule, and the same sweep prices the window's own cliff:
  a pathological C = 8 at B = 50 computes to P(bad) ≈ 0.11, and C = 0
  (mutual edges only) fails at any B.

The composition therefore sharpens the E18 rule rather than moving
it: **past the pool floor no cap both binds and stays harmless — a
binding cap makes the bad regime worse, and a harmless one protects
nothing**. The curve is measured at both ends: the binding corner
(C = 3, two seeds, 80 + 86 bad/400) and the quiet-end anchor (C = 12,
seed 1138, pre-registered off the sweep) — registered ~59.3 bad/400,
measured **59/400** (z = −0.04; z = −0.02 against the uncapped law,
z = −2.5 against the C = 3 elevation), every race column on its
registration and refusals down to 1 550 over 1.28 M victim rows. A
never-binding cap returns the law exactly: the form's ρ mediation
held out-of-sample.

## 7. The ordered comparison arm, measured

ADR 0043 made the construction N-039 rejected — the directional draw on
the symmetric handshake, under its own `edge-sym-ordered/v1` domain —
expressible in experiments configuration, so E18 §4's derived pricing
table gets measured rows (seeds 1135–1137; predictions frozen in the
config comments, the two tail cells at open acceptance mirroring E18's
cell shapes):

- **The saturation boundary** (`ordered-tail-b500`, equal total density
  to the unordered B = 250): measured **80 bad / 8 000** vs the
  corrected prediction 70.1 (z ≈ 1.2) — statistically the unordered
  cliff, not the twin-level 0 the follow-up program originally
  expected. The pre-registration arithmetic itself supplied the
  correction before the cell ran: at B′ = 500 the out-pool (mean 8)
  sits below K = 16, picks saturate it, and both of the ordered
  construction's independent exponents become pool-driven. E18 §4's
  "≈ ungated at every B, RF-repairable" row holds only **below**
  B′ = (N−1)/K; its frontier paragraph ("until picks saturate the
  half-pool") already contained the boundary the table row elided.
- **The equal-B discriminator** (`ordered-tail-b250`): measured
  **8 000/8 000 good** vs predicted E_iso = 8.3×10⁻⁸ — zero bad graphs
  at the exact gate width where the unordered pair measured 57 and 61.
  Below saturation the two independent coins do let the picks repair
  the tail, at the price of doubled admissibility (measured mean degree
  28.77 against the unordered 16; the ledger's first-order 29.1 sits
  ~1 % above — an unmodelled second-order residual, noted, not chased:
  the cell's registered measurands were the tail and the density).
- **The flooder mirror** (`ordered-flood-b50-cap16-s800`, the budget
  twin's coordinates): every race and route column within |z| ≤ 1.3 of
  the frozen predictions — including the two structural discriminators:
  the crossing shield collapses (mutual ≈ 0.05/0.06 vs the unordered
  2.56/3.20) and **own-only Sybil edges exist** (3.138 ± 0.001,
  identically zero under the pair draw). At the equal budget the
  attacker's occupancy clamps to the same value (12.09 vs the twin's
  12.02) while honest starvation nearly doubles (refused 5.64 vs 3.19
  per victim): the ordered gate's ≈ 2/B admissibility converts into
  honest damage, not extra occupancy, when the budget binds.

**The flooder's one miss, dissected.** Coverage came in at 386/400
against a pre-registered 400/400 (the prediction quoted geometric
isolation only). All fourteen bad graphs are single high-rank
strandings: nodes holding ~20 Sybil edges and zero honest edges. The
dissection (regenerated detail, rank-resolved) attributes the rate to
the instrument's order coupling, recorded as **N-042**: the wavefront's
canonical sender-rank sort makes budget admission first-come by one
global order, so per-dialer losses concentrate (bottom-400-rank honest
nodes lost 0 dials; top-400 lost ≈ all 12.65) while class-level shares
stay exactly fair — which is why every class-level column in this
report matched its prediction. Under a real network's decorrelated
per-victim orders, total out-side death is a product of independent
losses (≈ 0.44^12.75 ≈ 3×10⁻⁵) and the cell's expected coverage is
≈ 400/400. Two conclusions survive the attribution intact: the
refusal-rate doubling is class-level and real, and **the cap-sizing
anchor is predicate-dependent** — C = 16 satisfies the fresh-arrival
rule under the unordered pair (load 23.0) and violates it under the
ordered draw (load 28.7, and no crossing immunity), so an
ordered-gated deployment must raise C (admitting more attacker
pressure) to buy the same safety margin the unordered pair gets free.
*(Attribution confirmed at the ADR 0044 instrument — the per-victim
seeded arrival order, N-042 resolved: this cell re-run measured
**400/400 good** with the class-level race columns identical to the
decimal, 40 617.4 rejected/run.)*

E18 §4's comparison, completed with measured rows: inside the operating
window the unordered pair keeps its dominance (equal coverage, half the
per-identity admissibility, the crossing shield, and the budget anchor
intact); the ordered construction's one genuine advantage — a
pick-repairable tail — exists only below its saturation boundary and
is paid for twice at a binding budget.

## 8. Instrument notes

- The detail rows carry the drain-time route attribution (ADR 0042 /
  N-040): own-only / mutual / admitted × linked-peer class, partitioned
  by the dial drain's symmetric-dial record; refusals attributed
  fresh vs crossing. Run rows untouched; the instrument commits are
  byte-identical to the recorded baselines (§Provenance).
- Two pre-registered race models were corrected against measurement,
  both documented above and in the prediction script: the scan race's
  uniform-interleaving assumption (falsified by the driver's wave
  order — §1) and the per-member binomial pick split (falsified by the
  pilot — §2). Config comments keep the original registrations; every
  grid cell was priced by the corrected model **before** it ran.
- Mean comparisons use run-clustered SEs (the fold records per-run
  means); pooled per-victim SEs understate under within-run
  correlation.
- The budget race is class-fair but rank-concentrated on the dialer
  side (N-042, found by §7's dissection): class-level columns — all of
  this report's predictions — are exact under the canonical order;
  per-node tails under saturated budgets are amplified relative to a
  real network's decorrelated arrival orders. Resolved after this
  report by ADR 0044 (the per-victim seeded arrival order): the §7
  flooder cell re-run at the fixed instrument measured 400/400 good
  with class-level columns unchanged, confirming the attribution.

## 9. Scope

- The flooder is level-1 rational (dials only admissible pairs — an
  out-of-pair dial is self-incriminating evidence, N-036) and
  fair-race timed; a race-winning attacker bounds the worst case
  analytically (min(fresh pressure, C) admitted before any honest
  dial) exactly as in directional E12.
- Scheme A exists only at its pinned instrument commit; its cell is
  the recorded funeral, not a maintained configuration.
- The composition term (§6) is measured at one (B, μ) shape
  (B = 250, μ = 0.4): the binding corner (C = 3, two seeds) and the
  quiet-end anchor (C = 12, one seed) bracket the computed cap ladder
  (the ledger's `capsweep`); intermediate caps and other shapes are
  computed, not measured. Either way the regime sits outside the
  recommended window.
- The ordered × tight-budget × saturation × high-μ corner (the ordered
  analogue of §6) is not measured: the corrected forms predict the same
  composition mechanism there, and the regime sits outside any
  recommended window for either predicate.
- Retry/rotation, adaptive victim selection (N-037), and the
  incentive/chain layer remain out of scope, as in E12/E18.

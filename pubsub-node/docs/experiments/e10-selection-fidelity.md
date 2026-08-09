# E10 — selection-family fidelity: the verifiable hash gate vs the model's exact-K selection

| | |
|---|---|
| Tool commit | `ff09143` (branch `experiments-gate-tradeoff`; two commits ahead of `main` `b28cf58`) |
| Cell configs | [`configs/experiments/selection-fidelity/`](../../configs/experiments/selection-fidelity/) — ten cells, master seeds 901–903 (Family A) and 911–917 (Family B), suite-validated |
| Baseline cell | [`m2-bulk-regime.toml`](../../configs/experiments/m2-bulk-regime.toml): N = 4 000, μ = 0.2, RF = 16, coverage-law P(bad) = 0.0088, validated 71/8000 ([`m2-comparison.md`](m2-comparison.md) §2) |
| Reference values | `formal_spec/hybrid_dissemination/models/m2/properties/full_coverage.md` §3 (the coverage law read at each cell's realised mean degree); program of record `docs/experiments-program.md` E10 |
| Timings | gated cells run ~10× slower than the ungated baseline (the gate is 4 000 × 3 999 SHA-256 edge evaluations per run): 8 000-run cells ~85 min at 16 workers / ~2 h 10 at 8; the ten-cell grid ~15.5 h wall on an M4 Max, most of it at 8 workers |
| Artifacts | deliberately not committed; each cell reproduces byte-identically from its config, master seed, and the tool commit, at any `--workers` value |

The comparison program (PR #138) validated all five dissemination models
against the formal laws using the models' own idealized selection:
exactly-K seeded uniform picks. The protocol's real mechanism is the
verifiable hash gate: a dial edge is legal iff
`SHA-256(nonce, topic, requester, candidate) mod B == 0`, so the legal
out-neighbourhood per node is not a chosen set of size K but a
Binomial(N−1, 1/B) survivor set. E10 measures what that substitution
costs in coverage — a number the formal folder cannot produce, because
the models have no gate. It is the cost side of the B knob; E12
(flooding mitigation under the cap) is the benefit side.

All ten cells hold the baseline cell's population fixed (N = 4 000,
μ = 0.2 silent-relay adversaries, relay-only wiring, `forward-to-relays`,
open acceptance, model `m2` goodness = one SCC of the up-honest
propagation digraph) and vary only the selection coordinates.

## 1. The two families

- **Family A — gate only** (`pick_count` absent, the 005 shape): every
  gate survivor is dialed; realised out-degree *is* the binomial. Three
  cells: B = 167 / 250 / 500 ⇒ mean degree 23.95 / 16.0 / 8.0.
- **Family B — gate + pick count** (the 017 gated-picks plane): K = 16
  picked seeded-uniform from the survivor set. Seven cells sweep the
  **survivor headroom r = (N−1)/(B·K)** — expected survivors per pick
  wanted — from 25 down to 0.5.

Realised mean degree is verified in every cell through the send-cost
identity (honest→honest sends scale linearly with mean degree;
exact-16 reference 40 947).

## 2. Family B — the headroom ladder

| B | r | runs | bad | P(bad) | Wilson 95 % | coverage law 0.0088 |
|---|---|---|---|---|---|---|
| 10 | 25 | 8 000 | 77 | 0.0096 | [0.0077, 0.0120] | inside (z = +0.79) |
| 50 | 5 | 8 000 | 57 | 0.0071 | [0.0055, 0.0092] | inside (z = −1.60) |
| 125 | 2 | 8 000 | 64 | 0.0080 | [0.0063, 0.0102] | inside (z = −0.77) |
| 167 | 1.5 | 8 000 | 81 | 0.0101 | [0.0082, 0.0126] | inside (z = +1.27) |
| 250 | 1 | 4 000 | 177 | 0.0443 | [0.0383, 0.0511] | **excluded — 5.0× the law** |
| 333 | 0.75 | 4 000 | 1 473 | 0.368 | [0.353, 0.383] | Family-A regime (mean degree 11.76) |
| 500 | 0.5 | 4 000 | 4 000 | 1.0 | [0.999, 1.0] | Family-A regime (mean degree 7.97) |

**The plateau (r ≥ 1.5) is law-exact.** Pooled over the four
law-consistent cells: **279 bad / 32 000 = 0.00872** vs the law's
0.0088 (z ≈ −0.16); per-cell z-values carry both signs with mean ≈ −0.1.
Picking K uniformly from a uniformly-thinned survivor set is
distributionally exact-K while survivors ≥ K, and the measurement
confirms the composition is lossless — verifiability is free where B
leaves headroom. At r = 1.5 the composition already shows its first
seam: ~4 % of nodes fall 1–2 picks short, the realised mean degree dips
0.47 %, and the law read at that mean predicts ~74 expected bad — the
observed 81 is noise on top of a real but negligible drift.

**The cliff is at r = 1.** With mean survivors equal to K, half the
nodes fall short, and — the mechanism worth stating precisely — the
pick cap **truncates only the protective upper tail** of the binomial
while passing the entire dangerous lower tail: realised mean degree
drops 9.9 % (sends 36 886) with the low-degree tail of B = 250 intact.
P(bad) lands at 5× the law. At the same B, *removing* the pick count
entirely (Family A, row below: 0.0193) halves the damage: at fixed B a
pick count never helps coverage — it only removes edges. Its value is
cost-bounding and the E12 flooding side, never robustness.

**Below r = 1 the pick knob is inert** and the cells converge to
Family A at the survivor mean. The designed twin check — B = 500 with
and without the pick knob, independent master seeds — agrees across
every instrument: good 0/4 000 in both, honest sends 20 405 vs 20 409
(−0.02 %), condensation sinks per run 5.33 vs 5.34, and both produced
runs where the sampled publisher was muted outright (3 199 of 3 200
honest nodes missed).

## 3. Family A — the gate-only degree distribution

| B | mean degree | runs | bad | P(bad) | law at same mean | ratio |
|---|---|---|---|---|---|---|
| 167 | 23.95 | 8 000 | 0 | 0.0 (Wilson ≤ 4.8×10⁻⁴) | ≈ 3×10⁻⁵ | consistent (≈ 0.24 bad expected) |
| 250 | 16.0 | 4 000 | 77 | 0.0193 [0.0154, 0.0240] | 0.0088 | **2.2× (z = +7.1)** |
| 500 | 8.0 | 4 000 | 4 000 | 1.0 | ≈ 0.995 | collapse both ways |

The balanced point (B = 250, mean degree exactly the calibrated RF) is
the informative row, and its defect decomposition explains the whole
effect. The 77 bad graphs split into two classes:

- **43 muted publishers** (condensation sink, min-publisher-coverage 0)
  — the class the exact-K law already prices. A publisher's audience
  (its in-degree) is Poisson-dispersed *even under exact-K selection*
  (it is the count of independent dialers who picked it), so the gate
  changes nothing here: e^{−RF(1−μ)}·H ≈ 35 expected, 43 observed
  (z ≈ +1.3).
- **34 eclipsed receivers** (a node whose entire upstream draw is
  adversarial: the publish drain misses exactly that node) — the class
  exact-K makes negligible and the binomial resurrects. Under exact-K
  the per-node eclipse term is μ^K = 0.2¹⁶ ≈ 6.6×10⁻¹²; under the
  binomial it is E[μ^D] = (1 − (1−μ)/B)^{N−1} ≈ e^{−(1−μ)·mean} — equal
  in magnitude to the mute term, ≈ 6 orders of magnitude above μ^K.
  Predicted ≈ 35 runs, observed 34.

So at equal mean degree the gate-only shape **doubles P(bad)**: it
leaves the mute defect untouched and adds an equal-sized eclipse
defect. Both classes follow the same closed form, so the doubling is
multiplicative at any operating point — which yields the compensation
rule below. The B = 167 row confirms the multiplicative reading at a
clean operating point (2 × ~1.5×10⁻⁵ is still ~0), and the B = 500 row
confirms that no distribution subtlety matters once the mean itself is
insufficient.

The two-instrument design carried the decomposition: muted publishers
are invisible to the sampled publish drain (full coverage in all
8 000 + 4 000 runs of the clean and balanced Family B cells, bad graphs
included) while eclipsed receivers are exactly what the drain sees
(every deaf run misses precisely its deaf nodes) — the
`full_coverage ≥ good` invariant doing its job in both directions.

## 4. The B design rule

For the protocol's choice of B, at the calibrated operating shape
(μ = 0.2, δ-target parameters):

- **Gated picks (the 017 plane): size B for r ≥ 2**, i.e.
  **B ≤ (N−1)/(2K)**. The law then applies unchanged — verifiability
  costs nothing in coverage. r = 1.5 is marginal (sub-percent mean-degree
  erosion, law still inside the interval); r = 1 costs 5×; r < 1 is the
  gate-only regime at a mean the pick count no longer controls.
- **Gate only (no pick count): size B for one extra link of mean
  degree.** In this mode B *is* the degree dial — each node dials all
  its survivors, mean (N−1)/B — and sizing it for the model's RF pays
  the measured 2× P(bad) penalty (§3). Because both defect classes
  decay as e^{−(1−μ)·d} in the mean degree d, a factor of 2 is bought
  back by Δd = ln 2 / (1−μ) ≈ 0.87 ≈ 1 link at μ = 0.2. The correction
  is therefore: **choose B = (N−1)/(RF + 1) instead of (N−1)/RF.** At
  this grid's shape that is B ≈ 235 rather than the balanced 250:
  gate-only at mean degree 17 gives ≈ 2 × 0.0088 × e^{−0.8} ≈ 0.0079,
  back below the exact-K law's 0.0088 at RF = 16. The premium is
  independent of the δ target (the 2× is multiplicative on the law), so
  one extra link — ~6 % more traffic at RF = 16 — is the gate's entire
  coverage price wherever it is priced at all.
- The two knobs interact destructively at fixed B: adding a pick count
  to a gate-only configuration can only remove edges, and at r ≤ 1 it
  measurably worsens coverage (5.0× vs 2.2× at B = 250). The pick
  count's value is cost/attack-surface bounding (E12), not robustness.

## 5. Instrument notes

- **The gate's compute cost is real and first-class**: gated cells run
  ~10× slower than ungated ones in this instrument (16 M SHA-256 edge
  evaluations per run), and the dial-side analogue for a real node is
  N−1 evaluations per epoch per topic. A short-circuit dial (scan
  candidates in seeded-random order, stop at the K-th survivor) would
  cut this to ≈ K·B expected evaluations while preserving the uniform
  K-subset distribution exactly — but it changes realised draws for the
  same seed (a selection-plane behavior change: ADR + re-baseline), and
  a SHA-256 midstate cache over the constant per-requester preimage
  prefix would speed the instrument byte-identically. Both are noted
  here as candidates, deliberately not done mid-experiment.
- Worker counts varied across cells (16 → 10 → 8) for thermal reasons;
  artifact bytes are worker-count-independent by the framework's
  determinism contract, and one machine-sleep interruption mid-grid
  paused and resumed a cell without artifact effect.
- The B = 167 Family A cell is the convention's showcase for all-good
  samples: 0/8 000 with Wilson ≤ 4.8×10⁻⁴ is a *bound consistent with
  the ≈ 3×10⁻⁵ prediction*, where a ±1σ convention would degenerate.

## 6. Scope

The gated-symmetric variant (gate under the ADR 0034 symmetric
handshake — gated M4) is exactly N-039's recorded trigger and stays out
of this first E10 pass. E12 — flooding mitigation under the cap, the
benefit side of the same B knob — is the paired follow-up; together
they form the B trade-off table (coverage cost vs flooding resistance)
that neither side of the formal folder carries.

# E12 — flooding mitigation under the cap: the hash gate's defensive value

| | |
|---|---|
| Tool commit | instrument at `72bf76c` (the per-node connection-accounting detail columns; spot-checked byte-identical against the `23d0223`/`21acd36` baseline generations on all seven reference sweeps); the pilot ran at `286c944`, the 48 grid cells at `f5baea6` — both code-identical to `72bf76c` (config-only commits between) |
| Cell configs | [`configs/experiments/flooding/`](../../configs/experiments/flooding/) — the pilot (master seed 1000) plus the 4 × 3 × 4 grid (seeds 1001–1048, cap-major), suite-validated |
| Grid | B ∈ {50, 125, 250, 500} × accept cap ∈ {20, 24, 32} × K ∈ {40, 200, 400, 800} Sybils (1/5/10/20 % of N = 4 000); 150 runs per cell, 400 for the saturating K = 800 cells; 10 350 runs total including the pilot |
| Measurement | `--per-node-detail` (the `72bf76c` columns); each cell's detail folded into a per-cell summary by [`summarise_flooding_cell.py`](summarise_flooding_cell.py) and the raw detail deleted (regenerable from the seed); per-run identities — detail sums ≡ the row's `rejected_over_capacity`, acceptor-issued ≡ dialer-refused per class — verified in **all 49 cells** |
| Timings | ~2 min per 150-run cell / ~5.5 min per 400-run cell at `--workers 10`; the grid ~2.5 h wall |
| Artifacts | three main artifacts per cell reproduce byte-identically from config + seed + tool commit at any worker count; the derived summaries under `results/e12/summaries/` are pure folds of the detail |

E10 priced the hash gate's coverage cost; E12 measures what that same
gate buys against a slot-flooding Sybil attacker — the benefit side of
the B knob, and the empirical calibration of bucketed-pull's
K_max = OC·B flood bound. Together they form the B trade-off table
(§5) that neither side of the formal folder carries.

## 1. The scenario

An attacker with K purchased Sybil identities floods honest acceptance
slots. Honest acceptance is capped (`accept_cap`), and v1 has no dial
retry — an honest dial refused over capacity is a permanently lost
link. The **rational level-1 flooder** stays inside its valid edge set
(an out-of-bucket dial is self-incriminating, checkable evidence —
N-036), so each Sybil can legitimately dial ~1/B of the population and
of K identities only ~K/B can reach a given victim: the gate divides
per-victim attacker pressure by B.

In the framework: the adversarial class at (bucket count pinned, **no
pick count** — every valid edge dialed) with `silent-relay` fan-out and
uncapped acceptance, so every routed refusal is issued by an honest
victim; the honest class runs gated picks (K = 16) behind the cap. The
Sybil count is the class count, so each cell's ambient adversarial
fraction equals its attack scale (the Sybils are silent relays too —
at K = 800 the cell doubles as "the calibrated μ = 0.2 population turns
flooder").

**Contention semantics.** All dials land in one drain wave, canonically
sorted by (sender rank, recipient rank, content key); ranks come from
per-run keys independent of the class draw, so slot contention is a
**fair, seeded-random race** — refusals hit each class in proportion to
its share of arriving dials (verified throughout: the honest-to-Sybil
refusal ratio tracks the load ratio in every congested cell). A real
attacker who additionally wins the race (dialing the instant an epoch
opens) bounds the worst case analytically: min(≈ K/B, cap) slots per
victim before any honest dial lands. The measured curves are the
fair-arrival baseline inside that envelope.

## 2. The concentration law: attacker slots ≈ K/B, exactly

Across all 48 cells, mean attacker-held slots per honest victim match
the Binomial-thinning prediction K/B wherever the cap leaves room —
36 of 48 cells within 2 % of K/B, most within rounding — and the
per-victim distributions are the predicted Poisson shapes (the pilot's
P(0) matched e^{−K/B} to three decimals). The gate's division of
attacker pressure by B is not approximate; it is the mechanism's
arithmetic, measured.

The exception is itself the defense working: where K/B approaches the
cap's headroom above honest load, the cap truncates the attacker —
at B = 50 (the narrowest gate) and K = 800, the unconstrained 16
slots/victim are squeezed to 11.06 (cap 20), 13.06 (cap 24), 15.49
(cap 32). Attacker self-interference is real too: at that corner the
Sybils' own refusals exceed honest ones (15 788 vs 12 605 per run at
cap 20) — the identities compete with each other for the same ~K/B
valid slots.

## 3. Starvation and the cap-controlled attribution

Within a (B, K) cell, the ambient adversarial fraction and the
selection shape are fixed — only the cap varies. The cap axis therefore
isolates pure starvation damage:

| B = 50 | cap 20 | cap 24 | cap 32 |
|---|---|---|---|
| K = 400 (10 %): good | **45/150** | **150/150** | 150/150 |
| — starved honest dials/run | 7 590 | 2 794 | 123 |
| K = 800 (20 %): good | **0/400** | **0/400** | **392/400** |
| — starved honest dials/run | 12 605 | 7 533 | 1 320 |

Two design facts fall out:

- **Slot concentration is not the harm; starved honest links are.** At
  (B = 50, K = 400), raising the cap from 20 to 24 hands the attacker
  *more* slots (6.83 → 7.57) yet converts a collapsed network
  (P(good) = 0.30) into a fully good one — the headroom absorbs the
  same attack the tight cap turns into topology damage.
- **Headroom rescues even the saturating attack.** The 20 % flooder
  kills B = 50 at caps 20 and 24 outright, but at cap 32 the network is
  back to 392/400 good — within ~2σ of the pure ambient-μ expectation
  (the coverage law at μ = 0.2, law-exact selection at r = 5, predicts
  ≈ 396.5/400), i.e. the flooding-specific residue is nearly gone.

At B ≥ 125 the flooder never gains traction at any tested cap: the
B = 125 K = 800 row reads 388 → 394 → 396/400 across the caps (the law
at μ = 0.2 expects ≈ 396.5), and the B = 250 K = 800 cells match E10's
selection-penalty baseline exactly (382–386/400 vs the r = 1 baseline
P(bad) = 0.0443 → expected 382.3) — **statistically zero flooding
damage on top of what the selection shape already costs**.

The tight cap also self-congests without any attacker: at cap 20 the
K = 40 control cells lose ~600–1 900 honest dials/run to honest-on-
honest refusals (rising with the honest degree, i.e. with small B),
the congestion phenomenon E11 owns; at cap 32 the same controls lose
essentially none. Cap sizing against *honest* load variance is a
prerequisite for the flooding numbers to mean anything.

## 4. What the bounding cases add (documented, not simulated)

Per the program of record: with **no cap** there is no flooding surface
(nothing bounds a victim's accepted set — the attack degenerates to
resource exhaustion outside this model); with a **cap but no gate**,
all K identities can dial every victim, so concentration ≈ min(K, cap)
and honest crowd-out is total as K approaches the cap — the measured
≈ K/B curves against these two poles are the gate's entire defensive
contribution.

## 5. The B trade-off table (E10 × E12)

At the calibrated operating shape (N = 4 000, μ = 0.2, K_relay = 16,
gated picks), reading both sides of the knob together:

| B | headroom r | coverage cost (E10, measured) | attacker concentration (E12, measured) | flood outcome at cap ≥ 24 |
|---|---|---|---|---|
| 50 | 5 | law-exact | K/50 — 4.0 slots/victim per 5 % of N Sybils | 10 % attacker survivable; **20 % attacker kills below cap 32** |
| **125** | **2** | **law-exact** | **K/125 — 1.6 per 5 %** | **resilient through the 20 % attacker at every tested cap** |
| 250 | 1 | 5.0× the law | K/250 — 0.8 per 5 % | flooding adds nothing beyond the selection cost — but that cost is 5× |
| 500 | 0.5 | collapse | K/500 | moot (no coverage to defend) |

The design answer the two experiments jointly give: **at fixed pick
count K, choose the largest B that keeps r = (N−1)/(B·K) ≥ 2** —
here B = 125. Below it (B = 50) coverage is equally exact but the
attacker's per-victim concentration is 2.5× higher and the saturating
flooder becomes a real threat at practical caps; above it, E10's
selection penalty sets in long before the extra flooding resistance
is worth anything. The cap then sizes against honest load variance
with attack headroom on top (cap 24 ≈ mean + 2σ sufficed for the 10 %
attacker at every B; the 20 % attacker at the r-optimal B needed
nothing more). In bucketed-pull's terms, the measured concentration
curves are the empirical K_max = OC·B calibration with OC = the cap's
headroom above honest load.

## 6. Scope

Rotation/retry (which would convert lost dials into delayed ones),
attacker timing advantage beyond the analytic envelope in §1, and the
golden-tier variants stay out of this pass; E11 (honest congestion,
no adversary) reuses the same detail columns when it runs. The
per-victim adaptive-eclipse variant remains the formal folder's
`candidate_properties.md` backlog entry.

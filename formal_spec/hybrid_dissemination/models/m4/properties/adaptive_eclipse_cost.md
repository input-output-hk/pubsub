# M4 — adaptive eclipse cost (corruptions to strand a victim)

**Verdict: HYBRID** — the per-node degree law is exact, but the network
minimum treats the H degrees as independent (the same Poissonisation the
coverage law already makes at j = 0), so finite-N values are confirmed by
simulation. Script (in `../scripts/`): `sim_m4_eclipse.py` — closed
forms by default; `--mc` re-runs the 400-graph measurement behind the MC
columns (minutes; never run by CI).

## 1. Property

As in [M1](../../m1/properties/adaptive_eclipse_cost.md), but M4's links are
**undirected**, so deafening and muting are the *same* cut: severing a node's
honest links strands it in both directions at once, and the cost is its
honest degree.

Two consequences specific to M4:

- the cost is the honest degree **2·RF(1−μ) = 14.4**, not twice that — a
  bidirectional link is one connection and one corruption kills it;
- the guarantee-breaking minimum is over H degrees, **not** over 2H
  directional draws. Treating the two directions as independent would
  double-count and understate the cost (4.2 instead of 4.5).

## 2. Guiding formula

$$\text{degree} \;=\; \underbrace{\mathrm{Hypergeom}(RF)}_{\text{own picks, honest part}}
\;+\;\underbrace{\mathrm{Bin}\!\left(H-1,\tfrac{RF}{N-1}\right)}_{\text{others' picks}}
\quad\text{— mean } 2\,RF(1-\mu),$$

$$\mathbb{E}[\min] \;=\; \sum_{j\ge 1}\Pr(D\ge j)^{H}.$$

At j = 0 this is exactly `m4_model.p_isolated()` — the script asserts it
(3.791×10⁻¹⁰) — so the same law that sets P(bad) sets the eclipse cost.

## 3. Results — N = 20 000, μ = 0.2, RF = 9 (400 graphs)

Threat A — a named victim's own draw:

| attack | side | mean | sd | p1 % | p0.1 % | MC mean |
|---|---|---|---|---|---|---|
| deafen = mute | chosen + accepted | 14.40 | 2.94 | 8 | 6 | 14.39 |

Threat B — the network minimum:

| attack | E[min] | MC min | observed |
|---|---|---|---|
| deafen = mute | 4.5 | 4.5 | 2, 3, 4, 5, 6 |

Closed form and MC agree to within 0.1. No graph contained an isolated
node — a δ-event, where the eclipse cost is 0 because the victim is already
stranded, is a ~0.2 % occurrence in 400 draws at E ≈ 6.1×10⁻⁶ (the
δ-cheapest RF = 8, at E ≈ 6.8×10⁻⁵, puts one zero in a 400-draw run at
~2.7 % odds) — the coverage law at j = 0 and the eclipse cost at j ≥ 1 are
the same distribution read at different depths.

## 4. Answer

**Threat A: 14.4 corruptions** to strand a named victim. For a standing
target, 1 epoch in 100 that drops to ≤ 8 and 1 in 1 000 to ≤ 6. The
δ-cheapest RF = 8 read is 12.8.

**Threat B: 4.5 corruptions** out of μN = 4 000 — 3.2× below the mean
(δ-cheapest RF = 8: 3.6; at the δ-cheapest points the any-victim ordering
is M3 3.3, M4 3.6, M5 3.7, M1/M2 4.6).

**The backlog figure 25.6 is not defensible.** The backlog quotes M4's
eclipse cost as 25.6 — exactly twice the δ-cheapest RF = 8 honest degree,
4·RF(1−μ) rather than 2·RF(1−μ) — which is consistent with the doubling in
the closed form being applied a second time, plausibly on the intuition
that a bidirectional link counts in both directions. The figure appears in
no script or table anywhere in the repository, while
[`node_degrees.md`](node_degrees.md) measures the honest degree directly.
One corruption removes a bidirectional link once, so the honest degree is
what the attack costs: 14.4 at RF = 9, 12.8 at the δ-cheapest RF = 8. With
the threat-B order statistic applied, M4 sits near the cheap end of the
family's any-victim ordering, not the safe end. Its single link type and
low per-node state are real advantages; eclipse resistance is not among
them.

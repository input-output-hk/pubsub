# M4 — adaptive eclipse cost (corruptions to strand a victim)

**Verdict: HYBRID** — the per-node degree law is exact, but the network
minimum treats the H degrees as independent (the same Poissonisation the
coverage law already makes at j = 0), so finite-N values are confirmed by
simulation. Script (in `../scripts/`): `sim_m4_eclipse.py`.

## 1. Property

As in [M1](../../m1/properties/adaptive_eclipse_cost.md), but M4's links are
**undirected**, so deafening and muting are the *same* cut: severing a node's
honest links strands it in both directions at once, and the cost is its
honest degree.

Two consequences specific to M4:

- the cost is the honest degree **2·RF(1−μ) = 12.8**, not twice that — a
  bidirectional link is one connection and one corruption kills it;
- the guarantee-breaking minimum is over H degrees, **not** over 2H
  directional draws. Treating the two directions as independent would
  double-count and understate the cost (3.2 instead of 3.6).

## 2. Guiding formula

$$\text{degree} \;=\; \underbrace{\mathrm{Hypergeom}(RF)}_{\text{own picks, honest part}}
\;+\;\underbrace{\mathrm{Bin}\!\left(H-1,\tfrac{RF}{N-1}\right)}_{\text{others' picks}}
\quad\text{— mean } 2\,RF(1-\mu),$$

$$\mathbb{E}[\min] \;=\; \sum_{j\ge 1}\Pr(D\ge j)^{H}.$$

At j = 0 this is exactly `m4_model.p_isolated()` — the script asserts it
(4.226×10⁻⁹) — so the same law that sets P(bad) sets the eclipse cost.

## 3. Results — N = 20 000, μ = 0.2, RF = 8 (400 graphs)

Threat A — a named victim's own draw:

| attack | side | mean | sd | p1 % | p0.1 % | MC mean |
|---|---|---|---|---|---|---|
| deafen = mute | chosen + accepted | 12.80 | 2.77 | 7 | 5 | 12.80 |

Threat B — the network minimum:

| attack | E[min] | MC min | observed |
|---|---|---|---|
| deafen = mute | 3.6 | 3.6 | 0, 2, 3, 4, 5 |

Closed form and MC agree to within 0.1. One of the 400 graphs contained an
isolated node — a δ-event, where the eclipse cost is 0 because the victim is
already stranded. At E ≈ 6.8×10⁻⁵ that is a ~2.7 % occurrence in 400 draws
(other seeds give minima 3.5–3.6 with no zero), and it illustrates the
continuity between the coverage law at j = 0 and the eclipse cost at j ≥ 1.

## 4. Answer

**Threat A: 12.8 corruptions** to strand a named victim. For a standing
target, 1 epoch in 100 that drops to ≤ 7 and 1 in 1 000 to ≤ 5.

**Threat B: 3.6 corruptions** out of μN = 4 000 — 3.6× below the mean, and
second cheapest in the family behind M3.

**Correction to the earlier estimate.** The backlog previously quoted M4 at
25.6, which placed it as the *most* eclipse-resistant model. That figure is
exactly twice the measured degree — 4·RF(1−μ) rather than 2·RF(1−μ) — which
is consistent with the doubling in the closed form being applied a second
time, plausibly on the intuition that a bidirectional link counts in both
directions. It appears in no script or table anywhere in the repository,
while [`node_degrees.md`](node_degrees.md) has measured 12.80 since the
models were first published. Since one corruption removes a bidirectional
link once, the honest degree is what the attack costs, so 12.80 is the
defensible figure. With that corrected and the threat-B order statistic
applied, M4 moves from the safe end of the ordering to second cheapest. Its
single link type and low per-node state remain real advantages; eclipse
resistance is not among them.

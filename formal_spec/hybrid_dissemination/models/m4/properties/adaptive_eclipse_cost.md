# M4 — adaptive eclipse cost (corruptions to strand a victim)

**Verdict: CLOSED FORM** — the degree law is exact; only the network minimum
needs simulation. Script (in `../scripts/`): `sim_m4_eclipse.py`.

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

## 3. Results — N = 20 000, μ = 0.2, RF = 8 (15 graphs)

Threat A — a named victim's own draw:

| attack | side | mean | sd | p1 % | p0.1 % | MC mean |
|---|---|---|---|---|---|---|
| deafen = mute | chosen + accepted | 12.80 | 2.77 | 7 | 5 | 12.80 |

Threat B — the network minimum:

| attack | E[min] | MC min | observed |
|---|---|---|---|
| deafen = mute | 3.6 | 3.7 | 3, 4 |

## 4. Answer

**Threat A: 12.8 corruptions** to strand a named victim. For a standing
target, 1 epoch in 100 that drops to ≤ 7 and 1 in 1 000 to ≤ 5.

**Threat B: 3.6 corruptions** out of μN = 4 000 — 3.6× below the mean, and
second cheapest in the family behind M3.

**Correction to the earlier estimate.** The backlog previously quoted M4 at
25.6, which placed it as the *most* eclipse-resistant model. That figure was
4·RF(1−μ) — the 2× in the closed form applied twice — and appears nowhere in
any script or measurement; [`node_degrees.md`](node_degrees.md) has measured
12.80 since the models were first published. With the correct degree, and
then with the threat-B order statistic applied, M4 moves from the safe end of
the ordering to second cheapest. M4's single link type and low per-node state
remain its advantages; eclipse resistance is not among them.

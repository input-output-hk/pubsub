# M2 — adaptive eclipse cost (corruptions to strand a victim)

**Verdict: HYBRID** — the per-node degree laws are exact, but the network
minimum treats the H degrees as independent (the same Poissonisation the
coverage law already makes at j = 0), so finite-N values are confirmed by
simulation. Script (in `../scripts/`): `sim_m2_eclipse.py` — closed
forms by default; `--mc` re-runs the 400-graph measurement behind the MC
columns (minutes; never run by CI).

## 1. Property

As in [M1](../../m1/properties/adaptive_eclipse_cost.md): after the epoch's
draws are public, stranding a victim costs its honest degree on the attacked
side — **deafen** (cut honest in-edges) or **mute** (cut honest out-edges).
Threat **A** names the victim and pays its own draw; threat **B** takes the
cheapest node in either direction, a minimum over H nodes.

## 2. Guiding formula

M2 is M1 with the sides swapped. A node *chooses* its RF forwarders, so its
in-degree is concentrated; it does not choose its requesters, so its
out-degree carries the Poisson tail:

$$\text{in (chosen)}\sim\mathrm{Hypergeom}\ \text{— mean } RF(1-\mu),
\qquad
\text{out (accepted)}\sim\mathrm{Bin}\!\left(H-1,\tfrac{RF}{N-1}\right)\approx\mathrm{Poisson}(RF(1-\mu)).$$

$$\mathbb{E}[\min] \;=\; \sum_{j\ge 1}\Pr(D\ge j)^{H}.$$

At j = 0 the in-side expression is exactly `m2_model.p_eclipse()`
(1.589×10⁻¹⁷ both ways; the script asserts it) and the out-side is the
muted-publisher term e^{−RF(1−μ)}.

## 3. Results — N = 20 000, μ = 0.2, RF = 24 (400 graphs)

Threat A — a named victim's own draw:

| direction | side | mean | sd | p1 % | p0.1 % | MC mean |
|---|---|---|---|---|---|---|
| deafen | chosen | 19.20 | 1.96 | 14 | 13 | 19.20 |
| mute | accepted | 19.20 | 4.38 | 10 | 7 | 19.20 |

Threat B — the network minimum:

| direction | E[min] | MC min | observed |
|---|---|---|---|
| deafen | 10.4 | 10.5 | 7 … 12 |
| mute | 4.6 | 4.6 | 2, 3, 4, 5, 6 |

Closed form and MC agree on every mean to two decimals and on every minimum
to within 0.1.

## 4. Answer

**Threat A: 19.2 corruptions** for a named victim in either direction on
average; 1 epoch in 100 the *mute* cost of a standing publisher drops to
≤ 10, and 1 in 1 000 to ≤ 7.

**Threat B: 4.6 corruptions**, via **muting**, out of μN = 4 000 — 4.2×
below the mean.

**M2 is not the eclipse-resistant outlier its in-degree suggests.** Deafening
an M2 node is the most expensive attack in the family (10.4, more than
double any other model) precisely because the in-side is chosen and
concentrated. But coverage also fails when a publisher is muted, and M2's
out-side is accepted with a Poisson tail, so the cheapest break costs 4.6 —
identical to M1. This matches the coverage analysis, where M2's P(bad) is
carried entirely by the muted-publisher term (4.5×10⁻⁹ per node) and the
eclipse term (1.6×10⁻¹⁷) is eight orders of magnitude below it: the two
analyses agree on which side is weak, and this one prices it.

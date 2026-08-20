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
(3.164×10⁻¹⁸ both ways; the script asserts it) and the out-side is the
muted-publisher term e^{−RF(1−μ)}.

## 3. Results — N = 20 000, μ = 0.2, RF = 25 (400 graphs)

Threat A — a named victim's own draw:

| direction | side | mean | sd | p1 % | p0.1 % | MC mean |
|---|---|---|---|---|---|---|
| deafen | chosen | 20.00 | 2.00 | 15 | 13 | 20.00 |
| mute | accepted | 20.00 | 4.47 | 10 | 8 | 20.00 |

Threat B — the network minimum:

| direction | E[min] | MC min | observed |
|---|---|---|---|
| deafen | 11.1 | 11.1 | 7 … 13 |
| mute | 5.0 | 5.0 | 1 … 7 |

Closed form and MC agree on every mean to two decimals and on every minimum
to within 0.1.

## 4. Answer

**Threat A: 20.0 corruptions** for a named victim in either direction on
average; 1 epoch in 100 the *mute* cost of a standing publisher drops to
≤ 10, and 1 in 1 000 to ≤ 8.

**Threat B: 5.0 corruptions**, via **muting**, out of μN = 4 000 — 4.0×
below the mean.

**M2 is not the eclipse-resistant outlier its in-degree suggests.** Deafening
an M2 node is the most expensive attack in the family (11.1, more than
double any other model) precisely because the in-side is chosen and
concentrated. But coverage also fails when a publisher is muted, and M2's
out-side is accepted with a Poisson tail, so the cheapest break costs 5.0 —
identical to M1 at equal fanout. This matches the coverage analysis, where
M2's P(bad) is carried entirely by the muted-publisher term (2.0×10⁻⁹ per
node) and the eclipse term (3.2×10⁻¹⁸) is nine orders of magnitude below
it: the two analyses agree on which side is weak, and this one prices it.

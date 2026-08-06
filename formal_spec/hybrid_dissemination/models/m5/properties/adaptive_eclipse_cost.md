# M5 — adaptive eclipse cost (corruptions to strand a victim)

**Verdict: HYBRID** — the per-node degree laws are exact, but the network
minimum treats the H degrees as independent (the same Poissonisation the
coverage law already makes at j = 0), so finite-N values are confirmed by
simulation. Script (in `../scripts/`): `sim_m5_eclipse.py`.

## 1. Property

As in [M1](../../m1/properties/adaptive_eclipse_cost.md): after the epoch's
draws are public, stranding a victim costs its honest degree on the attacked
side — **deafen** (cut honest in-edges) or **mute** (cut honest out-edges).
Threat **A** names the victim and pays its own draw; threat **B** takes the
cheapest node in either direction.

## 2. Guiding formula

M5 is the only model that mixes chosen and accepted on *both* sides — a node
opens k_in inbound and k_out outbound links, and receives others' picks in
both directions:

$$\text{in} \;=\; \mathrm{Hypergeom}(k_{in}) + \mathrm{Bin}\!\left(H-1,\tfrac{k_{out}}{N-1}\right),
\qquad
\text{out} \;=\; \mathrm{Hypergeom}(k_{out}) + \mathrm{Bin}\!\left(H-1,\tfrac{k_{in}}{N-1}\right),$$

both with mean (k_in + k_out)(1−μ) = 13.6. Threat B:

$$\mathbb{E}[\min] \;=\; \sum_{j\ge 1}\Pr(D_{in}\ge j)^{H}\Pr(D_{out}\ge j)^{H}.$$

At j = 0 the two expressions are exactly `m5_model.p_in_isolated()` and
`p_out_isolated()` — the script asserts both (8.44×10⁻¹⁰ and 1.898×10⁻⁹).

## 3. Results — N = 20 000, μ = 0.2, (k_in, k_out) = (9, 8) (400 graphs)

Threat A — a named victim's own draw:

| direction | side | mean | sd | p1 % | p0.1 % | MC mean |
|---|---|---|---|---|---|---|
| deafen | chosen + accepted | 13.60 | 2.80 | 8 | 6 | 13.60 |
| mute | chosen + accepted | 13.60 | 2.91 | 7 | 6 | 13.60 |

Threat B — the network minimum:

| direction | E[min] | MC min | observed |
|---|---|---|---|
| deafen | 4.1 | 4.1 | 1 … 5 |
| mute | 3.9 | 3.9 | 1 … 5 |

Closed form and MC agree on every mean to two decimals and on every minimum
to within 0.1.

## 4. Answer

**Threat A: 13.6 corruptions** in either direction for a named victim. For a
standing target, 1 epoch in 100 the cost drops to ≤ 7–8 and 1 in 1 000 to
≤ 6.

**Threat B: 3.7 corruptions** out of μN = 4 000 — 3.7× below the mean.

Note this sits *below* either marginal minimum (4.1 deafening, 3.9 muting).
M5's two directions are close enough in law that the cheapest node overall
beats the cheapest node in either direction taken alone; the joint order
statistic is the correct one. In the other directed models the two sides are
far enough apart that the joint value coincides with the weaker marginal.

**M5 gets no concentration benefit on either side.** Mixing chosen and
accepted links in both directions means neither side is as tightly
concentrated as M2's all-chosen in-degree (sd 1.96) — both of M5's sit near
sd 2.8–2.9. Its balanced design buys symmetry, not eclipse resistance, which
is consistent with the cross-model finding that M5 is best on no measured
axis ([`../../comparison.md`](../../comparison.md)).

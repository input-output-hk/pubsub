# M3 — adaptive eclipse cost (corruptions to strand a victim)

**Verdict: HYBRID** — the per-node degree laws are exact, but the network
minimum treats the H degrees as independent (the same Poissonisation the
coverage law already makes at j = 0), so finite-N values are confirmed by
simulation. Script (in `../scripts/`): `sim_m3_eclipse.py` — closed
forms by default; `--mc` re-runs the 400-graph measurement behind the MC
columns (minutes; never run by CI).

## 1. Property

As in [M1](../../m1/properties/adaptive_eclipse_cost.md), but M3 has two link
kinds carrying different traffic, so "deafen" needs a definition. Initiation
links deliver only their owner's own publications, so they cannot supply a
node with *every* publisher's traffic:

- **deafen (coverage)** — cut the RF chosen forwarders. The victim can no
  longer receive arbitrary publishers, so full coverage fails, even though it
  still hears its initiation partners' own messages. **This is the reading δ
  is stated against.**
- **deafen (silence)** — additionally cut the accepted initiation in-links,
  after which the victim hears nothing at all. The number an operator would
  recognise as "eclipsed".
- **mute** — cut the honest requesters that pull from it (accepted) *and* its
  own honest initiation targets (chosen).

## 2. Guiding formula

$$\text{in, coverage (chosen)}\sim\mathrm{Hypergeom}(RF)\ \text{— mean } RF(1-\mu),$$
$$\text{in, silence} \;=\; \text{coverage} \;+\; \mathrm{Bin}\!\left(H-1,\tfrac{s-1}{N-1}\right),
\qquad
\text{out} \;=\; \mathrm{Bin}\!\left(H-1,\tfrac{RF}{N-1}\right) + \mathrm{Hypergeom}(s-1).$$

$$\mathbb{E}[\min] \;=\; \sum_{j\ge 1}\Pr(D\ge j)^{H}.$$

At j = 0 the coverage and mute expressions are exactly
`m3_model.p_in_isolated()` and `p_out_isolated()` — the script asserts both
(8.07×10⁻¹⁰ and 1.94×10⁻⁹).

## 3. Results — N = 20 000, μ = 0.2, RF = 13, s = 7 (400 graphs)

Threat A — a named victim's own draw:

| attack | side | mean | sd | p1 % | p0.1 % | MC mean |
|---|---|---|---|---|---|---|
| deafen (coverage) | chosen | 10.40 | 1.44 | 7 | 5 | 10.40 |
| deafen (silence) | chosen + accepted | 15.20 | 2.62 | 9 | 8 | 15.20 |
| mute | accepted + chosen | 15.20 | 3.37 | 8 | 6 | 15.20 |

Threat B — the network minimum:

| attack | E[min] | MC min | observed |
|---|---|---|---|
| deafen (coverage) | 3.8 | 3.9 | 2 … 5 |
| deafen (silence) | 5.8 | 5.8 | 4 … 7 |
| mute | 4.1 | 4.2 | 2 … 6 |

Closed form and MC agree on every mean to two decimals and on every minimum
to within 0.1.

## 4. Answer

**Threat A: 10.4 corruptions** to break coverage at a named victim — the
cheapest in the family — or 15.2 to silence it completely or to mute it as a
publisher (the 15.2 sides are budget reads, identical for every split of
RF + (s−1) = 19). For a standing target, 1 epoch in 100 the coverage cost
drops to ≤ 7 and 1 in 1 000 to ≤ 5.

**Threat B: 3.8 corruptions**, via deafening, out of μN = 4 000 —
2.7× below the mean (the joint deafen-or-mute read is 3.6).

**M3's weakness is level, not spread.** Its coverage in-degree is *chosen*
and tightly concentrated (sd 1.44, the smallest in the family), so it gets no
worse than its distribution implies — it is simply low, because RF = 13 is
the smallest pull fanout whose split clears the disturbance margin. Raising
RF is the direct fix; converting
accepted links to chosen ones, which is what M1 would need, buys M3 nothing.
This is the same brittleness the μ-shift analysis found
([`mu_shift_robustness.md`](mu_shift_robustness.md)): the bandwidth winner
pays for it on the robustness axes.

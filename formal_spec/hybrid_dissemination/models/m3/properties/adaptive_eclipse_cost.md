# M3 — adaptive eclipse cost (corruptions to strand a victim)

**Verdict: HYBRID** — the per-node degree laws are exact, but the network
minimum treats the H degrees as independent (the same Poissonisation the
coverage law already makes at j = 0), so finite-N values are confirmed by
simulation. Script (in `../scripts/`): `sim_m3_eclipse.py`.

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
(4.045×10⁻⁹ and 8.612×10⁻¹⁰).

## 3. Results — N = 20 000, μ = 0.2, RF = 12, s = 8 (400 graphs)

Threat A — a named victim's own draw:

| attack | side | mean | sd | p1 % | p0.1 % | MC mean |
|---|---|---|---|---|---|---|
| deafen (coverage) | chosen | 9.60 | 1.39 | 6 | 5 | 9.60 |
| deafen (silence) | chosen + accepted | 15.20 | 2.74 | 9 | 8 | 15.20 |
| mute | accepted + chosen | 15.20 | 3.27 | 8 | 6 | 15.19 |

Threat B — the network minimum:

| attack | E[min] | MC min | observed |
|---|---|---|---|
| deafen (coverage) | 3.3 | 3.3 | 1, 2, 3, 4 |
| deafen (silence) | 5.6 | 5.5 | 3 … 7 |
| mute | 4.3 | 4.3 | 2 … 6 |

Closed form and MC agree on every mean to two decimals and on every minimum
to within 0.1.

## 4. Answer

**Threat A: 9.6 corruptions** to break coverage at a named victim — the
cheapest in the family — or 15.2 to silence it completely or to mute it as a
publisher. For a standing target, 1 epoch in 100 the coverage cost drops to
≤ 6 and 1 in 1 000 to ≤ 5.

**Threat B: 3.3 corruptions**, via deafening, out of μN = 4 000 — the
cheapest guarantee break in the family, 2.9× below the mean.

**M3's weakness is level, not spread.** Its coverage in-degree is *chosen*
and tightly concentrated (sd 1.39, the smallest in the family), so it gets no
worse than its distribution implies — it is simply low, because RF = 12 is
the bandwidth-minimal choice. Raising RF is the direct fix; converting
accepted links to chosen ones, which is what M1 would need, buys M3 nothing.
This is the same brittleness the μ-shift analysis found
([`mu_shift_robustness.md`](mu_shift_robustness.md)): the bandwidth winner
pays for it on the robustness axes.

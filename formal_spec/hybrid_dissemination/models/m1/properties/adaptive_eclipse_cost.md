# M1 — adaptive eclipse cost (corruptions to strand a victim)

**Verdict: HYBRID** — the per-node degree laws are exact, but the network
minimum treats the H degrees as independent (the same Poissonisation the
coverage law already makes at j = 0), so finite-N values are confirmed by
simulation. Script (in `../scripts/`): `sim_m1_eclipse.py` — closed
forms by default; `--mc` re-runs the 400-graph measurement behind the MC
columns (minutes; never run by CI).

## 1. Property

Bribery is adaptive: the adversary acts *after* the epoch's draws are
public. Stranding a victim v means cutting every honest edge on one side of
it — its already-adversarial links are dead anyway — so the cost is v's
**honest degree** in the attacked direction:

- **deafen** v — cut its honest in-edges; v can no longer receive some
  publisher, and full coverage fails;
- **mute** v — cut its honest out-edges; v's own publications reach nobody.

Coverage fails either way, so both directions count. Two threat models:

- **A — chosen victim**: the adversary names v and pays v's own draw. The
  mean is what a random node pays; the lower tail is what a *standing*
  high-value target risks, since epoch rotation re-draws the degree.
- **B — any victim**: the adversary only needs to break the δ guarantee, so
  it takes the cheapest node in either direction — a minimum over H nodes.

**Cost = degree, not less.** The min vertex cut separating v could in
principle sit deeper than v's in-neighbourhood, but at M1's branching factor
of 20.0 the depth-2 shell is an order of magnitude larger than the depth-1
shell and overlap is negligible at H = 16 000, so Menger's disjoint-path
count saturates at the degree. A max-flow cross-check (node-split, unit
vertex capacities, weakest node in each direction) lives in
[`../../validate.py`](../../validate.py) §7, but as its PR review observed,
it sources every honest node — the victim's neighbours included — which
forces flow = degree on any graph: it pins the implementation, not the
claim, which for now rests on the shell argument above. Restricting the
sources to distant publishers, so that a shared upstream hub could surface
as a cheaper cut, is the noted follow-up.

## 2. Guiding formula

M1's two sides have different laws, and that is the whole story:

$$\text{in (accepted)}\sim\mathrm{Bin}\!\left(H-1,\tfrac{F}{N-1}\right)\approx\mathrm{Poisson}(F(1-\mu)),
\qquad
\text{out (chosen)}\sim\mathrm{Hypergeom}\ \text{— mean } F(1-\mu).$$

Threat B reads the same laws as an order statistic:

$$\mathbb{E}[\min] \;=\; \sum_{j\ge 1}\Pr(D\ge j)^{H},
\qquad\text{equivalently the smallest } j \text{ with } H\cdot\Pr(D\le j)\gtrsim 1 .$$

**This is the coverage law read at j ≥ 1.** At j = 0 the same expressions are
exactly `m1_model.p_in_isolated()` and `p_out_isolated()`, whose sum times H
is `E_defects()` — the script asserts the identity (3.258×10⁻⁵ both ways).

## 3. Results — N = 20 000, μ = 0.2, F = 25 (400 graphs)

Threat A — a named victim's own draw:

| direction | side | mean | sd | p1 % | p0.1 % | MC mean |
|---|---|---|---|---|---|---|
| deafen | accepted | 20.00 | 4.47 | 10 | 8 | 20.00 |
| mute | chosen | 20.00 | 2.00 | 15 | 13 | 20.00 |

Threat B — the network minimum:

| direction | E[min] | MC min | observed |
|---|---|---|---|
| deafen | 5.0 | 5.0 | 1 … 7 |
| mute | 11.1 | 11.1 | 7 … 13 |

Closed form and MC agree on every mean to two decimals and on every minimum
to within 0.1 — the Poissonisation behind E[min] costs nothing measurable at
this H.

## 4. Answer

**Threat A: 20.0 corruptions** to deafen or mute a named victim on average —
but 1 epoch in 100 the deafen cost of a standing target drops to ≤ 10, and
1 in 1 000 to ≤ 8.

**Threat B: 5.0 corruptions**, via deafening, out of an adversarial budget of
μN = 4 000. The guarantee-breaking cost is **4.0× below the mean**, because
the adversary shops the lower tail across 16 000 nodes.

**Why deafening is the cheap side**: M1's in-degree is *accepted* (others'
picks — a balls-in-bins draw with a Poisson lower tail, sd 4.47), while its
out-degree is *chosen* (its own F picks, only thinned by μ — binomially
concentrated, sd 2.00). At an identical mean of 20.0 the accepted side is
2.2× cheaper to cut. M2 is the exact mirror
([`../../m2/properties/adaptive_eclipse_cost.md`](../../m2/properties/adaptive_eclipse_cost.md)).

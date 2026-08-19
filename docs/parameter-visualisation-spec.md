# Parameter-relationship visualisations for the CIP and the companion site

Status: draft for review. Written against the 2026-08-17 Ezequiel/Will
session; View 3's sizing rules revised against E20.

## Why this exists

The session aligned on presenting **relationships between parameters** in the CIP
and on the companion site, using charts and sliders, **rather than proposing a
single static model**. That decision changes what the documents have to carry.
Today the CIP argues towards one configuration and the site's compare-designs
tool renders a fixed operating point; neither lets a reader ask the question they
actually have, which is *what happens to my deployment at my size, against an
attacker with my budget*.

Two findings make that question urgent rather than cosmetic.

**Realistic network size is far below the analysed one.** The analysis runs at
*N* = 4 000 and *N* = 20 000. Reaching 1 000 participants is the ambitious case.
Every headline figure is therefore quoted at a scale the deployment will not see
for a long time, and the axis that most separates the candidate designs —
connection count — is also the axis that stops separating them as topics shrink
and multiplexing collapses the difference.

**A fixed adversarial fraction is the wrong security frame at small *N*.** The
session agreed to analyse the impact of a **fixed number of Sybil identities**
rather than a fixed fraction of the network. An attacker does not choose a
percentage; it chooses a spend. With a deposit *D* and capital *X* it can afford
*A* = ⌊*X*/*D*⌋ identities, and the fraction it holds is

> *μ* = *A* / *N*

which **rises as the network shrinks**. A design safe at *μ* = 0.2 and
*N* = 20 000 faces *μ* = 0.2 at *N* = 1 000 only if the attacker can afford
200 identities rather than 4 000. Same deposit, twentieth of the network,
twentieth of the attack cost. This is the relationship the visualisations exist
to make visible, and it is invisible in every chart we currently publish.

## The three views

### View 1: where a fixed attacker budget breaks a design

Fix *A*, the attacker's identity count. Sweep *N*. Plot p<sub>bad</sub> against
*N* for each design at its proposed parameters, with the failure target *δ* drawn
as a horizontal line.

The reader's takeaway is the **crossing point**: the network size below which the
design no longer meets its target against that attacker. This is the direct
answer to "at what point does the system become vulnerable given a fixed attacker
budget", and it inverts the usual reading, since here *smaller is worse*.

This is a **reparameterisation, not new mathematics**. Every coverage law
already takes *μ*; substituting *μ* = *A*/*N* and sweeping *N* reuses the closed
forms as they stand. The one quantity worth naming is the **critical size**
*N*\*, the root of p<sub>bad</sub>(*N*, *A*/*N*) = *δ*, below which a design stops
meeting its target against that attacker.

Controls: *A* (identity count), *δ*, design selection, and the per-design
parameters already exposed by the compare-designs tool.

Annotations the view must carry, because they bound where the curve means
anything:

- the **gate floor**, *N*<sub>T</sub> = 4*k* + 1, below which the gate switches
  off entirely: about 37 for a pick count of 9, about 53 for a pick count of 13;
- the **pool floor** from the symmetric gating work,
  (*N*−1)/*B* ≥ ln(*H*/*δ*)/(1−*μ*), which a larger fanout cannot buy back;
- the range where **no measurement exists** — the CIP is explicit that a topic of
  fifty is outside the analysis rather than a small instance of it, and that the
  few-hundred middle is unmeasured. The curve must be visibly distinguished
  there rather than drawn as though it were evidence.

### View 2: deposit against security

The bridge from economics to the coverage laws. Given an assumed attacker capital
*X*, a deposit *D* yields *A* = ⌊*X*/*D*⌋ and hence *μ* = *A*/*N*.

Plot the **deposit required** to hold p<sub>bad</sub> at or below *δ*, against
*N*, for a given *X*. Equivalently, plot the attacker capital needed to break a
deployment at a given *D* and *N*, which is the number an operator can reason
about.

This view is where the honest caveat belongs: the session's position is that a
reasonable deposit is **not purely an engineering question** and needs the use
cases defined first. The view should therefore present *X* as an explicit,
user-supplied assumption rather than shipping a default that reads as a
recommendation.

The companion statement is the one an operator can act on: holding a fraction
*μ* at size *N* costs the attacker *μ*·*N*·*D*, so **attack cost scales linearly
in network size**. Growth buys security at a fixed deposit; shrinkage sells it.

A single global registration deposit is assumed. Per-topic deposit sizing set by
topic maintainers was floated in the session and not pursued, so it is out of
scope here.

### View 3: the parameter surface

The relationships between *N*, the pick budget *K*, the bucket count *B*, the
admissions budget *C*, and the epoch length. This is the view Ezequiel's tool or
table is expected to source, and the site should render rather than recompute.

**The sizing rules below are E20's, and they replace the ones this section
carried in its first draft.** E20 is the first pass to measure the gate and the
admissions budget at the CIP's own operating shape — *N* = 20 000, *k* = 9 or
10, *μ* = 0.2. Both earlier rules were calibrated at *k* = 16, where the
all-picks-adversarial term *μ*<sup>*k*</sup> ≈ 10⁻¹¹ is invisible; at *k* = 9 it
is 5 × 10⁻⁷, large enough to re-enter the coverage budget, and neither rule
transfers. A view drawn against the superseded rules would render a safe region
that is not safe.

Minimum content:

- ***B* is derived, and at the CIP's pick count the coverage target is what
  binds it — not the headroom floor.** *B* is maximal subject to three
  constraints at once: the gated coverage law meeting the failure target *δ*,
  the pool floor (*N*<sub>T</sub>−1)/*B* ≥ ln(*H*/*δ*)/(1−*μ*), and the
  selection headroom *r* = (*N*<sub>T</sub>−1)/(*B*·*k*) ≥ 2. Which of the three
  binds is *k*-dependent, so the view should draw all three and let the reader
  see the smallest rather than hard-code one as *the* constraint. At *N* =
  20 000, *μ* = 0.2, *δ* = 10⁻⁴ and *k* = 9 the target binds at *B* ≈ 730
  (*r* ≈ 3), the pool floor at *B* = 847, and the headroom floor not until
  *B* = 1111 — where p<sub>bad</sub> is 9.2 × 10⁻³, ninety-two times the target.
  **The one-line rule *B* = ⌊(*N*<sub>T</sub>−1)/2*k*⌋ returns exactly that last
  value**, which is why it is the rule the view must not draw as safe.
- ***C* is sized on the total fresh arrival load, with a *k*-dependent headroom
  term**: *C* ≥ *L* + *c*·√*L*, where
  *L* = (1−*m*)·[*k*(1−*μ*) + *A*/*B*] and
  *m* = min(1, *k*·*B*/(*N*<sub>T</sub>−1)) is the share of a node's own picks
  answered as crossings rather than arriving as admissions. Two corrections to
  the earlier form. *L* carries the attacker's fresh pressure *A*/*B* alongside
  the honest term *k*(1−*m*)(1−*μ*), because those arrivals consume budget
  whether the node wants them or not, so a cap clearing only the honest half is
  short by about half. And *c* is *k*-dependent — *c* ≈ 2 at *k* = 16,
  **c ≈ 3.5 at *k* = 9–10** — so the sizing cannot be quoted without its pick
  count. At the CIP shape (*k* = 10, *B* = 500, *A* = 4 000), *L* = 12.0: the
  honest-only rule gives *C* = 11 and p<sub>bad</sub> 1.5 × 10⁻², *c* = 2 gives
  *C* = 19 and 1.0 × 10⁻⁴ — on target with no margin — and *c* ≈ 3.5 gives
  *C* ≈ 25, against the measured recommendation of 23.
- **The *C* axis is one-sided, and the view should show it that way.**
  Tighter than the rule is not safer: past the pool floor a cap that binds makes
  isolation measurably worse through the composition channel, and no value both
  binds and stays harmless. Looser is inert. The *C* axis is therefore a
  one-sided cliff, not a trade-off, and drawing it as a trade-off inverts the
  advice.
- the **cap-blind floor** *k*·*μ*, so a reader can see the share of a node's
  connections that no acceptance policy reaches and that only gate width moves;
- epoch length against the churn ceiling, since the beacon floors the epoch and
  the epoch decides whether the churn budget binds at all. The churn budgets
  drawn here must be the **gated** ones, which differ sharply by pick count:
  7.57 % at *k* = 10 against 2.65 % at *k* = 9, where the ungated figure the
  site carries today is 7.43 %.

Because *L* contains *A*, the admissions budget now consumes the attacker's
identity count — the same input View 1 is built on. The two views share a slider
rather than merely a subject.

## What already exists

`web/experiments/cost-model/index.html` is a self-contained page with no build
step, holding a hand-ported JS implementation of the closed forms and a 42-case
self-test that runs on load. It already has an *N* slider (1 000 to 50 000), a
*μ* input, a *δ* selector, per-design parameters, eight radar axes and a
comparison table, and its numbers reproduce the CIP at the recommended operating
points.

So the rendering surface, the maths and the interaction model are all in place.
What is missing is exactly the reframing above:

| needed | present today |
| --- | --- |
| fixed identity count *A* as an input | no — *μ* is entered as a fraction |
| deposit, or attacker capital | no |
| bucket count *B* | no |
| admissions budget *C* | no |
| selection headroom *r* | no |
| gate floor / pool floor / unmeasured range | no |
| epoch length | no |

The first two rows are the new page. The rest are the extension to this one,
which does not model *B*, *C* or *r* at all today.

## Where these live

**Views 1 and 2 belong on a new page**, not in the compare-designs tool. They
answer a different question — *is my deployment safe at my size and deposit* —
and they invert the existing framing, since a smaller network is worse here and
neutral there. None of their inputs (*A*, *X*, *D*) exist on the current page,
whose radar already carries eight axes with four shown, and whose single
self-contained file and self-test are worth not destabilising.

**View 3 belongs in the compare-designs tool**, as an extension. Same laws, same
operating points, same reader.

## Division of labour

The session assigned Ezequiel a tool or table for the parameter relationships,
and this specification should not duplicate it. The split that avoids collision:

- **Ezequiel's tool is the source of truth** for formulas and for any figure that
  has to be validated against experiment.
- **The site renders**, and must not introduce a second, independently maintained
  copy of the maths.

**That tool now exists.** E20 landed
`pubsub-node/docs/experiments/m4_synthesis_predictions.py`, the E18/E19 forms
with *N*, *k*, *B*, *C* and the attacker size lifted to parameters, with modes
for the *B* ladder, a single cell, the cap sweep, and the cross-model
comparison. It is the natural back end for this view, and it covers the site's
current page as well as the new surface: the gate is vacuous at *B* = 1, and the
ledger there returns 6.066 × 10⁻⁶ against the CIP's published ungated M4 law of
6.1 × 10⁻⁶. One implementation can therefore serve both the ungated figures the
compare-designs tool renders today and the gated surface View 3 adds, with *B*
as the slider between them.

E20 also proposes porting its comparison into `web/experiments/` as a follow-on.
That lands in the same place as View 3 and on the same laws, so the two should be
settled as one piece of work rather than arriving as two ports of the same
surface.

The current page violates that split already: its self-test fixtures are
hand-copied constants stamped 2026-08-11, with nothing re-reading the Python, so
drift in `formal_spec/**` will not be detected until someone re-runs it by hand.
Whatever data contract comes out of Ezequiel's tool should replace those
constants, and a check should run in CI.

## Non-goals

Scope is deliberately frozen ahead of the delivery date agreed in the session.
Not in scope here: selecting between M3 and M4, which the session deferred; a
recommended deposit figure, which needs the use cases first; and any change to
the compare-designs page's existing axes or operating points.

## Open questions

1. What attacker capital *X* to offer as an illustrative default, or whether to
   ship none and require the reader to supply one.
2. Whether the views ship in the CIP as static generated figures, on the site as
   interactive pages, or both. The session's wording covers both surfaces; static
   figures need a generator in the CIP's existing figure pipeline.
3. Whether the unmeasured range is drawn with a visual treatment or omitted. It
   is the range most readers will actually be in, so omitting it may be worse
   than drawing it with an explicit caveat. E20 gives the boundary a number in
   one direction at least: its CIP-scale cells are law-consistency anchors
   rather than tail measurements, since 400 runs resolve nothing below about
   10⁻²·⁵. Everything below that line on a p<sub>bad</sub> axis is the ledger
   speaking, not a measurement, whatever *N* the reader has chosen.
4. Whether View 1 should carry the **hardened backbone / golden channel** variant
   discussed for small-scale use cases such as stake pool notifications, since
   that is precisely the regime where the curves say the open design struggles.

# Parameter-relationship visualisations for the CIP and the companion site

Status: draft for review. Written against the 2026-08-17 Ezequiel/Will session.

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

Open input, needs a decision before build: whether to model a **per-topic**
deposit, which is what the topic-maintainer self-regulation idea implies, or a
single global registration deposit. The two give different curves, and the
on-chain topic registry can express either.

### View 3: the parameter surface

The relationships between *N*, the pick budget *K*, the bucket count *B*, the
admissions budget *C*, and the epoch length. This is the view Ezequiel's tool or
table is expected to source, and the site should render rather than recompute.

Minimum content:

- *B* is derived, not configured: *B* = ⌊(*N*<sub>T</sub>−1)/2*k*⌋, with the
  headroom rule *r* = (*N*<sub>T</sub>−1)/(*B*·*k*) ≥ 2 shown as the binding
  constraint and the saturation boundary *B* = (*N*<sub>T</sub>−1)/*k* shown as
  the upper one;
- *C* sized on fresh honest arrival, *k*(1−*m*)(1−*μ*) with
  *m* = min(1, *k*·*B*/(*N*<sub>T</sub>−1)), **not** on a multiple of the pick
  count, which is a directional result superseded for symmetric links;
- the **cap-blind floor** *k*·*μ*, so a reader can see the share of a node's
  connections that no acceptance policy reaches and that only gate width moves;
- epoch length against the churn ceiling, since the beacon floors the epoch and
  the epoch decides whether the churn budget binds at all.

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

The two shortest paths are to add an *A*-and-*N* mode alongside the existing *μ*
input, and to surface *B*, *C* and *r*, which the page does not model at all
today.

## Division of labour

The session assigned Ezequiel a tool or table for the parameter relationships,
and this specification should not duplicate it. The split that avoids collision:

- **Ezequiel's tool is the source of truth** for formulas and for any figure that
  has to be validated against experiment.
- **The site renders**, and must not introduce a second, independently maintained
  copy of the maths.

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

1. Per-topic or global deposit, per View 2.
2. What attacker capital *X* to offer as an illustrative default, or whether to
   ship none and require the reader to supply one.
3. Whether the views live in the CIP as static generated figures, on the site as
   interactive pages, or both. The session's wording covers both surfaces; static
   figures need a generator in the CIP's existing figure pipeline.
4. Whether the unmeasured range is drawn with a visual treatment or omitted. It
   is the range most readers will actually be in, so omitting it may be worse
   than drawing it with an explicit caveat.
5. Whether View 1 should carry the **hardened backbone / golden channel** variant
   discussed for small-scale use cases such as stake pool notifications, since
   that is precisely the regime where the curves say the open design struggles.

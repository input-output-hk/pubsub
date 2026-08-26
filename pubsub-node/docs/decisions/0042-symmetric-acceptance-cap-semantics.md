# ADR 0042: The acceptance cap on a symmetric seam is an admissions budget

**Status**: Accepted
**Date**: 2026-08-15
**Feature**: the symmetric flooding pass (the E12 analogue under the
symmetric handshake — E18's benefit-side follow-up; deliberately on the
small-feature path — no spec dir, this ADR is the decision record). Resolves
N-032; fires N-040's trigger.

## Context

N-032 recorded the question 015 deferred: on a symmetric node every
accepted edge is mirrored into both link maps, and the node's own accepted
dials bypass the acceptance gate while still occupying the capacity its
scan counts. What the code does today was recorded so the behaviour would
not be mistaken for a decision: the cap's link scan counts the whole
mirrored link set — own-dial mirrors included — while the gate fires only
on peer-initiated requests, so realised degree can exceed the cap and the
outcome is arrival-order-dependent. N-032's trigger was the first
experiment requiring the symmetric × capped combination; the symmetric
flooding grid is that consumer.

Four structural facts frame the decision:

- **Direction is unrecoverable from end-state, by design.** The link model
  stores kind, never who dialed (N-039 records the cost of storing
  initiation). But initiation needs no post-hoc recovery: at event time it
  is manifest — an own dial is a pending `AwaitingAccept` entry completed
  by an Acceptance (not an admission decision); a peer-initiated request
  arrives with no such entry; a crossing is a request matching one.
- **The node's own picks are already bounded** — by the pick count K, by
  construction, chosen by the same operator who chooses the cap. The only
  free variable an acceptance policy can govern is what other peers send.
- **The victim has an admission-free entry route.** Its own picks landing
  on gate-admissible adversarial identities construct mutual links without
  any acceptance decision — expected occupancy ≈ K × (adversarial pool
  share), at any cap value, under any cap semantics.
- **A refusal costs a whole edge, both ends** — under the symmetric
  handshake a refused dial is not one directed link out of K but the
  entire edge, including the refused dialer's own selection.

## Decision

**The cap bounds admissions — one semantics on the existing knob.** An
acceptance cap of C means: refuse a peer-initiated request for an edge the
node did not itself select once C such admissions have been granted this
epoch. On the directional and publisher seams the existing link scan
already implements this (the scanned kind-set *is* the admitted set), and
nothing changes there. On symmetric seams the implementation is corrected
from the accidental both-role scan to the intended count:

- **An admitted-count per (topic, kind)** on `NodeState`, folded at the
  existing symmetric admission fold site and surfaced through a `NodeView`
  accessor; a symmetric `UnifiedAcceptance` instance compares it — not the
  link scan — against `accept_cap`. Aggregate bookkeeping only: no
  per-link initiation state, nothing enters the protocol object, edge
  validity stays a pure function of (nonce, topic, pair).
- **Crossings are exempt.** An inbound request matching the node's own
  pending `AwaitingAccept` toward the requester short-circuits ahead of
  gate and cap — the same shape as the prelude's idempotent already-held
  re-Accept, one lifecycle step earlier — and spends no budget: answering
  one's own selection is not an admission decision. The short-circuit
  runs ahead of the whole admission policy, membership included: the
  pending dial is the witness (it went only to a gate-surviving,
  membership-checked candidate — under either symmetric predicate), so a
  peer leaving between dial and crossing is accepted on the dial-time
  view, the staleness any in-flight accept already tolerates.
- **The budget is per-epoch.** No decrement on severance: the
  direction-erased link set cannot attribute a severed link to a past
  admission without the per-link state this design refuses, so a spent
  slot is unrefundable until rotation. (On directional seams the scan
  keeps freeing slots on severance — the nuance between "currently held
  admitted links" and "admissions granted this epoch" exists only where
  direction is erased, and this ADR owns it.)

Two properties follow by construction, order-independently, both scoped
per epoch: total symmetric degree ≤ K + C within one epoch's
accumulation, and the defensive invariant **no node holds more than C
edges it did not choose, per epoch** — checkable per run. (A refunding
`Epoch` rotation leaves previously admitted edges in place while the
budget resets, so across n epochs the standing bound is K + nC until
link rotation/teardown — out of scope since 005 — retires carried-over
admissions; nothing shipped fires `Epoch` today.)

One conservative edge case: in a real network a crossing can race — the
peer's request arriving before the node's own dial for that pair leaves —
and is then counted against the budget. The error direction is budget
spent early, never exceeded. Under the experiments driver the race cannot
occur: dial emission folds every pending entry before the wave delivers
any request.

## Why the recorded scan behaviour is retired, not preserved

Every candidate cap semantics admits fresh arrivals by the same fair race
(refusals hit each class in proportion to its arriving load — the E12
contention model), so the admitted adversarial:honest **ratio** is
semantics-independent. The candidates differ in exactly two things:

- **How much budget reaches fresh arrivals.** The both-role scan spends
  ≈ K units on the node's own picks; counting all inbound admissions
  spends units on crossings. Both are pure waste — the spent-on edges
  exist regardless — and both refuse honest and adversarial arrivals
  proportionally, so they lower realised degree (the E12 harm channel:
  starved honest links) without improving composition. The admissions
  budget wastes nothing and its arrival budget is a constant, uncoupled
  from pick-realisation noise.
- **What a refusal can kill.** Under the scan, an arrival past cap is
  refused even when it is the crossing of the node's **own pick** — and
  the veto is asymmetric in the attacker's favour: adversarial acceptors
  accept everything, so only the node's picks on honest mutual partners
  die. Worse, it is attacker-triggerable: flooding early fills the scan,
  and every honest mutual crossing arriving after that is vetoed — a
  second damage channel on top of budget capture. Under the admissions
  budget a refusal only ever kills an edge the node did not choose.

The admissions budget therefore weakly dominates on the final adversarial
link proportion in every regime and strictly dominates under congestion
and the race-winning attacker — precisely the regimes a cap exists for —
while being the only candidate whose defensive claim is a sharp,
order-independent invariant. The scan behaviour was never a sanctioned
semantics; it is measured once (the contrast cell pair, run at the
pre-change tool commit) and then ceases to be expressible.

## Consequences

- N-032 is resolved. N-040 resolves with it: the cap now enforces a
  direction-attributed quantity, so the detail columns gain drain-time
  route attribution (driver-side, the refusal-maps precedent) — an
  instrument change recorded in the living contract home
  (`configs/experiments/README.md`), detail rows only.
- The semantics change is expected byte-invisible to every recorded
  baseline generation: no baseline or suite config combines the symmetric
  switch with an accept cap (verified against `notes/experiments-baselines/`
  before any cell runs).
- Scheme A (the both-role scan on symmetric) is not reproducible at HEAD —
  it was an emergent interaction of the kind-scan with mirrored state,
  never a selectable policy. The contrast cells' configs pin their
  tool commit; a re-run at HEAD violates their recorded predictions
  (refused-crossing ≡ 0 under the budget), so the mistake fails loud.
- Cap-sizing guidance anchors on the **fresh-arrival load** K(1−m)
  (m = pick fraction, min(1, K·B/(N−1))), not on the both-role degree
  ≈ 2K; the 017 quickstart's "caps anchor on ≈ 2× the pick count" reading
  is superseded for symmetric seams (frozen record stays frozen; the
  flooding report and the living homes carry the rule).

## Alternatives rejected

- **Ratify the both-role scan** (the cap bounds total symmetric degree).
  The own-pick half of the total is already bounded by K by construction;
  re-bounding it through the scan adds no protection and creates the veto
  pathology, a pick-noise-coupled arrival budget, arrival-order-dependent
  overshoot, and no sharp invariant.
- **Count all inbound admissions, crossings included** (the directional
  "granted serving slots" semantics transplanted). On a symmetric seam
  both roles ride every edge, so there is no inbound-serving-load quantity
  to bound; crossings spend budget on edges that exist anyway, and
  refusing one kills the node's own pick.
- **A dedicated second knob keeping the scan expressible at HEAD.**
  Permanent CLI surface for a construction whose only remaining purpose is
  one contrast measurement; two names for what the ADR establishes as one
  semantics with a corrected implementation.
- **Per-link initiation state** (exact decrement-on-severance, a
  concurrently-held-admissions bound). The cost N-039 records against
  direction-dependent designs: an initiation bit in symmetric link state
  on both ends, history-dependent edge sets, and the loss of both-ends
  agreement — for a refinement the per-epoch budget does not need.

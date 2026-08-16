# ADR 0043: The ordered symmetric predicate — a measured comparison arm

**Status**: Accepted
**Date**: 2026-08-15
**Feature**: the symmetric flooding pass (with ADR 0042; no spec dir).
Companion to N-039, whose rejected construction this arm measures.

## Context

N-039 records why the symmetric gate draws the **unordered pair**: the
symmetric link model erases initiation, so edge validity must be a pure
function of (nonce, topic, pair) — the alternative (the directional
draw on the dialer, reciprocity constructed on accept) makes validity
initiation-dependent, lets the two ends' survivor sets disagree, and
doubles per-identity Sybil admissibility to ≈ 2/B. E18 §4 priced that
alternative from closed forms only: tail ≈ ungated at every B
(RF-repairable — no empty-pool cliff, two independent coins per pair)
against the unordered pair's K-independent cliff past the pool floor,
and admissibility 2/B against 1/B. The symmetric flooding pass measures
the unordered column of that table; the ordered column has never been
measured, and the pass's machinery (the route-split detail columns, the
pair-draw flooder, the admissions budget) makes measuring it a
config-plus-predicate exercise.

## Decision

**A second symmetric gate predicate under its own domain, expressible
in experiments configuration only.**

- `is_valid_edge_sym_ordered` in `strategies::edge`: the directional
  draw (`requester → candidate`, initiation-dependent) under the
  dedicated domain `pubsub/bucketed-pull/edge-sym-ordered/v1` — an
  independent draw from both the directional relay domain and the
  unordered symmetric domain. An edge forms if **either** direction's
  draw holds (each end dials its own survivors; the acceptor verifies
  the dialer's direction); total pair density ≈ 2/B.
- `Selection` and `UnifiedAcceptance` gain a `symmetric_ordered`
  switch, legal only with `symmetric`: the handshake vocabulary, the
  link model, and the acceptance semantics (the ADR 0042 budget,
  crossing exemption included) are untouched — only the draw and its
  verification change.
- The knob is an **experiments-config coordinate**
  (`symmetric_ordered = true`, requiring `symmetric = true`), not an
  operator CLI flag: its one consumer is the comparison program, and
  N-039's protocol choice stands. Model coherence treats it as a free
  plane coordinate under `m4` (like caps and bucket counts): existing
  configurations keep their meaning unchanged.

## Consequences

- The E18 §4 pricing table's ordered rows become measurable: the tail
  identification (ordered B′ = 2B against the unordered cliff at equal
  total density), the ≈ 2/B admissibility (read directly off the
  flooder's fresh-pressure column), and the composition contrast (two
  independent coins have no empty-pool channel for starvation to
  compose with).
- No row-schema change; the instrument commit is expected
  byte-identical to the recorded generations (no shipped configuration
  sets the knob).
- Under the ordered draw the two ends' survivor sets differ, so the
  crossing exemption covers only the mutually-drawn overlap
  (≈ 1/B of each end's survivors × pick behaviour) — the flooder's
  effective fresh pressure rises accordingly; the prediction script
  carries the ordered forms.

## Alternatives rejected

- **A three-valued `symmetric = "pair" | "ordered"`**: changes the
  type and meaning of an existing knob every committed config uses;
  the additive boolean leaves them untouched.
- **An operator CLI flag**: forward-compatible surface with no named
  operator consumer — the comparison program is config-driven.
- **Measuring via the directional relay domain instead of a dedicated
  one**: would correlate the ordered-symmetric draw with directional
  relay selection wherever both appear, and (N-039) the domain is the
  identity of the draw; independence is the point of a comparison arm.

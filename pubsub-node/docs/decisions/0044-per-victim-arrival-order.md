# ADR 0044: The instrument's arrival-order model — per-victim seeded order

**Status**: Accepted
**Date**: 2026-08-20
**Feature**: the instrument pass (with the failure-severity row change;
no spec dir). Resolves N-042, whose trigger fired in the M4 synthesis
pass.

## Context

The wavefront driver canonicalises every wave by a single global key —
(sender rank, addressee rank, message identity) — so all recipients
process their intra-wave arrivals in the same sender-rank order. For
class-level measurands this is exactly fair: ranks are independent of
the class draw, so refusals split by class-load share, and every
class-level mean the passes predicted matched (E12, E18, the E19
grid). Per node it concentrates: budget admission is first-come, so
once arrivals exceed budgets population-wide, a low-ranked dialer wins
every race at every acceptor and a high-ranked dialer loses every one.
Per-node tail events composed of many dial outcomes are then amplified
by orders of magnitude relative to the independent per-victim arrival
orders a real network approximates (N-042). Two passes measured the
amplification directly: the E19 ordered flooder's coverage row (386/400
good where independent orders predict ≈ 400/400, every stranding
high-rank) and the M4 synthesis pubseam cell at μ = 0.4 (188/400 bad
against the corrected independent-order form's 84.8, seed-dial losses a
step function of rank).

A real network serialises each victim's arrivals independently: the
order in which dials reach victim A says nothing about their order at
victim B. The canonical global order couples every victim's race to
one rank order — a measurement artifact, not a model property.

## Decision

**Each recipient processes its intra-wave arrivals in its own seeded
order — a pure function of (run seed, recipient, sender) — and the
canonical global sender-rank order is retired.**

- The wave sort key becomes (addressee rank, arrival key, sender rank,
  message identity), where the arrival key is
  `SHA-256(lp("experiments/arrival-order/v1") ‖ run_seed ‖
  lp(recipient) ‖ lp(sender))`. Deliveries group by recipient; within a
  recipient, senders follow the recipient's own keyed order,
  decorrelated between recipients by construction; remaining ties are
  same-pair deliveries, ordered by message identity as before.
- The order is deterministic and worker-count-independent exactly as
  the retired sort was: the key is a pure function of the run seed and
  the wave's content, never of collection order or scheduling. A run
  remains a pure function of (configuration, seeds).
- `Driver::new` takes the run seed; the recorded seed-derivation rule
  in every manifest names the arrival-key derivation. This is
  **instrument randomness**, not protocol randomness: nothing a node
  computes reads it, and the core is untouched.
- Class-level fairness is preserved under both orders (ranks and
  arrival keys are equally class-blind); what changes is only the
  per-node coupling.

## Consequences

- Per-node tail measurands under saturated budgets (starvation
  isolation, mute-stranding under a binding seed-intake cap,
  attacker-timing and retry studies) now measure the decorrelated-order
  quantity the closed forms model, instead of an instrument-amplified
  upper bound. Two frozen registrations gate the change: the M4
  synthesis pubseam μ = 0.4 cell re-run must land on the
  independent-order form (~84.8 bad/400, ~83 % mute / 17 % deaf) and
  the E19 ordered flooder re-run on geometric isolation only
  (≈ 400/400 good), both with class-level race columns unchanged.
- The change is byte-affecting wherever intra-wave order matters
  (races, first-delivery attribution — not wave membership), so it
  forces a re-baseline generation per ADR 0036; the parked
  failure-severity row change batches with it. Uncapped sweeps may or
  may not be value-identical; the generation decides and is stored
  regardless.
- Existing reports' class-level results are unaffected by
  construction; their per-node-tail rows carry the N-042 attribution
  already recorded in them.
- One SHA-256 per delivery per wave in the sort's cached key — noise
  against the apply cost at the measured population sizes.

## Alternatives rejected

- **Keeping the global order and correcting analytically** (the
  synthesis pass's stopgap): every saturated-budget pass would carry a
  model of the instrument instead of measuring the modelled quantity;
  the correction is itself shape-dependent (the rank step function).
- **A per-victim order seeded from (victim, run seed) but ranking by
  sender rank within ties**: preserves a global sender bias inside
  each victim's order; the per-pair key removes rank from the race
  entirely.
- **Randomising per wave (keying the wave index in)**: models
  per-message re-serialisation, which decorrelates *retries* too —
  stronger than a real network's per-link ordering and not needed by
  either consumer; per-run pair keys keep replays and dissections
  simple (one order per pair per run).
- **An RNG-shuffled wave** (ChaCha over the collected wave): order
  would depend on wave membership and collection history, breaking
  the content-derived canonicalisation that makes interrupted sweeps
  and replays byte-exact.

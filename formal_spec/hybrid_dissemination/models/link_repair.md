# Link repair (mid-epoch redraws) — M1–M5

**Status: DEFINED — analysis and simulation pending.** Expected verdict:
HYBRID (closed-form equivalence + repair traffic; residual exposure under
realistic repair latency by a time-stepped simulation).

## 1. Property

Behaviour when dead links **are** repaired within the epoch. Only the
chooser of a link can redraw it: chosen-side losses are detectable (the
connection drops) and repairable from the next entries of the node's
verifiable draw sequence (replacement budget b + j, preserving uniformity
and bounded admission); accepted-side losses need no repair — their chooser
is the node that departed.

## 2. Planned analysis

- **fresh-sample equivalence**: with prompt chooser-side repair the
  surviving subgraph is distributionally a fresh sample on the surviving
  population, so each coverage law holds continuously at N(t), μ(t);
- **residual exposure**: the repair-latency window (a node runs one link
  short between a death and its redraw), integrated over the epoch;
- **repair traffic**: churn rate × chosen degree × (handshake + proof
  verification); the repair surface per model:

| model | repair surface (chosen links) |
|---|---|
| M1 | F = 24 (out) |
| M2 | RF = 24 (in) |
| M3 | RF + (s−1) = 19 (pull + initiation) |
| M4 | RF = 8 (bidirectional) — the smallest in the family |
| M5 | k_in + k_out = 17 (both kinds) |

- machinery: a time-stepped simulator with Poisson departures (new kind).

Open design inputs to pin down first: (a) departures visible (clean
disconnect) vs silent (timeout) — sets the repair-latency distribution;
(b) mid-epoch redraw permission and the replacement-budget semantics of the
verifiable scheme. Related properties:
[`churn_tolerance.md`](churn_tolerance.md),
[`join_service.md`](join_service.md).

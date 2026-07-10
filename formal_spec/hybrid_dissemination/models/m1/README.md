# M1 — pure random push gossip

(Previously called *RandCast*; the historical reports use that name.)

## Model

N nodes; each node pushes every message it holds to F targets drawn uniformly
without replacement from the other N−1. No ring, no deterministic links, no
seeding. A node receives a message iff some reached node's push link points at
it — reception is governed entirely by its in-edges, which are other nodes'
choices.

## Assumptions

- k = μN silent adversaries: receive, never relay.
- Uniform, grinding-resistant target sampling; honest nodes always serve;
  one-shot dissemination per message.

## Analyses

- [`properties/`](properties/README.md) — per-property analyses and the
  script index; executable model and simulators in `scripts/`.
- Historical:
  [`randcast_partition_report.md`](../../partitioning/randcast_partition_report.md)
  (connectivity threshold).

# M4 — undirected (bidirectional) RF-out gossip

**M4 = every node picks RF peers uniformly; each pick is a *bidirectional* link.
A message floods along all incident links except the one it arrived on.**
No seeding mechanism, one knob per node (RF). Links are
symmetric, so the same edge carries a node's outgoing and incoming traffic.

## Model

N nodes: H honest, k = μN silent adversarial (receive but never relay).
Each node picks RF distinct others uniformly; every pick becomes an undirected
edge. Dissemination is flooding: a node that receives a message forwards it on
every incident link except the arrival link.

Because edges are symmetric, an honest node reaches (and is reached by) exactly
the honest nodes in **its connected component of the honest-induced subgraph**
(adversaries are silent, so paths run through honest nodes only). Full coverage
of a message ⇔ that honest subgraph is connected — independent of which honest
node is the source.

## Why bidirectionality is interesting

A single set of edges serves both directions:

- **Every node's own RF picks are also its in-edges.** At μ = 0 this gives
  minimum degree ≥ RF with no help from anyone, so there are no isolated
  vertices and the random RF-out graph is connected w.h.p. for **RF ≥ 2**
  (Fenner–Frieze) — no ln N fanout, no seeds. There is no
  ignition/muted-publisher failure mode at all: a publisher's own links carry
  its message out.
- **With adversaries**, an honest node is cut off only if *both* directions
  fail: all RF of its own picks are adversarial **and** no honest node picked
  it. Those are independent, so isolation is doubly rare.

## Assumptions

Uniform, grinding-resistant peer sampling (cache poisoning voids the picks);
silent worst-case adversaries; one-shot flooding per message (recovery
out-of-band). Not addressed: active adversaries (drop/delay/equivocate),
serving/bandwidth exhaustion under flooding, correlated churn.

## Analyses

- [`properties/`](properties/README.md) — per-property analyses and the
  script index; executable model and simulators in `scripts/`.

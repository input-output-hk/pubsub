# M5 — directed k_in/k_out gossip

## Model

N nodes: H honest, μN silent adversarial. Every node opens two
kinds of directed links, both its own uniform picks:

- **k_in inbound links** — it chooses k_in forwarders; each chosen forwarder
  relays every message it holds to the choosing node (edge f → j, picked
  by j).
- **k_out outbound links** — it chooses k_out targets; the node relays every
  message it holds to them (edge j → t, picked by j).

A node that receives a message relays it once on every outgoing propagation
edge — its own k_out targets plus the nodes that picked it as forwarder —
except back on the arrival link. A pick landing on an adversary is a dead
edge. There is no separate seeding mechanism: a publisher's own k_out links
(plus whoever in-picked it) carry its messages out.

The sampled graph is the classical **k-in/k-out random digraph**
(Fenner–Frieze) restricted to honest picks; the boundary cases k_out = 0 and
k_in = 0 recover pull-only ([M2](../m2/README.md)) and push-only
([M1](../m1/README.md)) relaying.

Epoch semantics: one sampled graph serves all messages and all publishers of
an epoch.

## Assumptions

Uniform without-replacement sampling for both pick sets, independent across
nodes and link kinds; grinding-resistant peer sampling (cache poisoning voids
the picks); silent worst-case adversaries; honest nodes always serve; one-shot
dissemination per message (recovery out-of-band).

## Analyses

- [`properties/`](properties/README.md) — per-property analyses and the
  script index; executable model and simulators in `scripts/`.

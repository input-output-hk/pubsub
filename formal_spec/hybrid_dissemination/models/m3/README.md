# M3 — pull relaying with standing initiation links

## Model

N nodes: H honest, k = μN silent adversarial. Two link kinds, both
sampled per epoch:

- **Relaying — pull, RF links.** Each honest node requests RF
  forwarders, chosen uniformly via private, epoch-seeded selection from a
  grinding-resistant peer-sampling layer. A forwarder relays every message it
  holds to its requesters for the whole epoch; a pick landing on an adversary
  is a dead edge. All multi-hop propagation runs on these edges.
- **Seeding — initiation, s−1 links.** Each node opens s−1 standing
  initiation links to uniform targets (fixed for the epoch); s counts the
  intended initial holders — the publisher plus its s−1 targets. At publication,
  the publisher sends its message directly over its initiation links; an
  honest target becomes an initial holder. Initiation links carry only their
  owner's own publications — they are never part of the relay graph.

Epoch semantics: one sampled graph (pull picks + initiation targets) serves
all messages and all publishers of an epoch.

## Assumptions

Inherited from M2: uniform, grinding-resistant peer sampling for both the
pull picks and the initiation targets (cache poisoning voids the guarantees);
silent worst-case adversaries; honest nodes always serve; one-shot
dissemination per message (recovery of missed messages is out-of-band).

## Analyses

- [`properties/`](properties/README.md) — per-property analyses and the
  script index; executable model and simulators in `scripts/`.

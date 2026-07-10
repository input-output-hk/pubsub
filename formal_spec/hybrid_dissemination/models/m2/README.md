# M2 — pull relaying

## Model

N = H + k nodes: H honest, k = μN silent adversarial. One edge
family:

- **Relaying — pull, per epoch.** Each honest node requests RF
  forwarders, chosen uniformly via private, epoch-seeded selection from a
  grinding-resistant peer-sampling layer
  ([`mitigation_epoch_report.md`](../../partitioning/mitigation_epoch_report.md)).
  A forwarder relays every message it holds to its requesters for the whole
  epoch; a pick landing on an adversary is a dead edge. All multi-hop
  propagation runs on these edges.

There is no seeding mechanism: a publisher injects a message only through its
*serving set* — the nodes that happened to pick it as forwarder.

Epoch semantics: one sampled pull graph serves all messages and all publishers
of an epoch.

## Assumptions

Uniform without-replacement sampling; independence of all pull samples; silent
worst-case adversaries; honest nodes always serve; unbiasable sampling (cache
poisoning voids everything); one-shot dissemination per message.

## Analyses

- [`properties/`](properties/README.md) — per-property analyses and the
  script index; executable model and simulators in `scripts/`.

# Partitioning Analysis

Formal analysis of graph partitioning in the Cardano Pub/Sub dissemination layer.
Two topologies are studied: **RingCast** (H(N,2) ring + random r-links) and
**RandCast** (pure random gossip, no ring backbone).

See [`manual.md`](manual.md) for instructions on how to run all models and scripts.

---

## Reports

| File | Description |
|---|---|
| [`adversarial_partition_report.md`](adversarial_partition_report.md) | RingCast: adversarial partitioning attack — 2 nodes can isolate any subscriber with P = e^{−RF}, independent of N |
| [`randcast_partition_report.md`](randcast_partition_report.md) | RandCast: natural connectivity fragility — P(partition) → 1 for fixed RF as N grows; connectivity threshold at RF ≈ ln N |

---

## RingCast — adversarial model (PRISM)

Adversary controls k nodes, chosen before r-links are sampled. One PRISM DTMC
run per adversary choice; sweep script takes the maximum over all C(5,k) choices.

| File | Description |
|---|---|
| [`ringcast_n6_adversarial.prism`](ringcast_n6_adversarial.prism) | DTMC model, N=6, RF=1 |
| [`ringcast_n6_adversarial_rf2.prism`](ringcast_n6_adversarial_rf2.prism) | DTMC model, N=6, RF=2 |
| [`adversarial.props`](adversarial.props) | Properties: P(partition) and P(connected) |
| [`adversarial_sweep.sh`](adversarial_sweep.sh) | Sweep over all adversary strategies, RF=1 |
| [`adversarial_sweep_rf2.sh`](adversarial_sweep_rf2.sh) | Sweep over all adversary strategies, RF=2 |

---

## RandCast — connectivity model (PRISM + Monte Carlo)

No adversary. Models the natural partition probability of a pure random directed
graph as a function of N and RF.

| File | Description |
|---|---|
| [`randcast_n6_partition_rf1.prism`](randcast_n6_partition_rf1.prism) | DTMC model, N=6, RF=1 — exact results, validated against Monte Carlo |
| [`randcast_n6_partition_rf2.prism`](randcast_n6_partition_rf2.prism) | DTMC model, N=6, RF=2 — exact results, validated against Monte Carlo |
| [`randcast_partition_mc.py`](randcast_partition_mc.py) | Monte Carlo simulation for arbitrary N and RF; includes threshold experiment (RF = ⌈ln N⌉) |

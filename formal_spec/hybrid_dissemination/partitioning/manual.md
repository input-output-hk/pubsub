# Running the Partitioning Models

## Prerequisites

**PRISM models:** PRISM 4.10 (or later) must be on your `PATH`. Verify with:

```
prism -version
```

**Monte Carlo scripts:** Python 3 (no extra packages required). Verify with:

```
python3 --version
```

All commands below assume you are in this directory (`partitioning/`).

## Files

| File | Description |
|---|---|
| `ringcast_n6_adversarial.prism` | DTMC model, RingCast RF=1 (one r-link per node) |
| `ringcast_n6_adversarial_rf2.prism` | DTMC model, RingCast RF=2 (two r-links per node) |
| `adversarial.props` | Property file (partition and connected probabilities) |
| `adversarial_sweep.sh` | Sweep script for RF=1: enumerates all adversary strategies |
| `adversarial_sweep_rf2.sh` | Sweep script for RF=2 |
| `randcast_n6_partition_rf1.prism` | DTMC model, RandCast RF=1 (no adversary) |
| `randcast_n6_partition_rf2.prism` | DTMC model, RandCast RF=2 (no adversary) |
| `randcast_partition_mc.py` | Monte Carlo simulation for RandCast, arbitrary N and RF |

## Running a single PRISM query

Each model takes boolean constants `dead1`..`dead5` specifying which nodes the adversary
kills (node 0 is the source and is never killed).

Example — adversary kills nodes 1 and 3 (RF=1):

```
prism ringcast_n6_adversarial.prism adversarial.props \
      -const dead1=true,dead2=false,dead3=true,dead4=false,dead5=false
```

PRISM prints two results: `P=? [F "partition"]` (probability some live node is
unreachable from the source) and `P=? [F "connected"]` (complement).

## Running the full sweep

The sweep scripts enumerate all C(5,k) adversary kill sets for k=0..4, run PRISM for
each, and report the maximum partition probability (the adversary's best strategy).

```
bash adversarial_sweep.sh       # RF=1
bash adversarial_sweep_rf2.sh   # RF=2
```

Each line of output shows the kill set S and the resulting partition probability.
The `>> Adversary's best` line at the end of each block gives the optimal strategy
for that budget k.

## Interpreting results

- **k=0, k=1**: partition probability is always 0 — H(N,2) is 2-connected, so no
  single node removal can disconnect the graph.
- **k=2**: optimal strategy is the *alternating cut* — kill both ring neighbors of a
  target node j. Partition probability converges to e^{−RF} as N grows.
- **k=3**: non-monotone peak — killing both neighbors of the source simultaneously
  gives higher partition probability than k=2 or k=4.

See `adversarial_partition_report.md` for the full analysis.

---

## RandCast Monte Carlo simulation

`randcast_partition_mc.py` estimates P(partition) for a pure random gossip graph
(no ring backbone, no adversary) via Monte Carlo sampling.

### Default table — N = 6..200, RF = 1..3

```
python3 randcast_partition_mc.py
```

### Threshold experiment — RF = ⌈ln(N)⌉ for N = 6..1000

Verifies the analytical connectivity threshold and compares simulated
P(partition) against the formula 1 − exp(−exp(−c)), c = RF − ln(N).

```
python3 randcast_partition_mc.py --threshold
```

### Single configuration

```
python3 randcast_partition_mc.py --N 50 --RF 4 --trials 200000
```

### Reproducible runs

Pass `--seed` to fix the random seed:

```
python3 randcast_partition_mc.py --seed 42
python3 randcast_partition_mc.py --threshold --seed 42
```

See `randcast_partition_report.md` for results and interpretation.

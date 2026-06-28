# A silent view-bias attack on Cyclon 

> An adversary that runs Cyclon's protocol externally with full fidelity can,
> by privately deleting some honest entries from its own view and by
> prioritising adversary-pointing entries when picking the random subset to
> gossip, distort the network's view distribution . 

## How to reproduce

[`cyclon_sim_biased.py`](cyclon_sim_biased.py) — simulator (Cyclon Enhanced Shuffling + attack).

```bash
# Single one-off run with custom parameters (full attack at μ=0.10):
python3 cyclon_sim_biased.py --mus 0.10 --p_drop 1.0 --bias_on 1 --T 120 --seeds 5

# Baseline sanity check (no attack — adversaries behave honestly):
python3 cyclon_sim_biased.py --p_drop 0 --bias_on 0

# Stealth-mode attack (no batch-size signal, ~2× amplification):
python3 cyclon_sim_biased.py --mus 0.10 --p_drop 0.5 --bias_on 1
```

All runs are seed-deterministic.

## 1. The attack at an intuitive level

Cyclon is a peer-sampling protocol. Every node maintains a small "view" — a set of `c` peer references — and periodically gossips a random subset of `ℓ` of those references with another peer chosen from the view. Over time, peer references diffuse uniformly across the network, and any node's view becomes a fresh, near-uniform random sample of the network's membership. 

An adversary that wants to break the uniform-sample property — to make honest nodes' views over-represent the adversary, in service of a hub or eclipse attack — does not need to break any cryptographic mechanism. It is sufficient for the adversary to do **two purely internal things**, neither of which produces any *protocol-level* anomaly:

1. **Silently forget honest peers.** Whenever the adversary's view holds a reference to a real honest node, the adversary may delete that reference from its own private storage. Nothing prevents this — the view is private state, and no other party can verify what the adversary stores.
Moreover, this might be needed even by honest node in case of connection failures.

2. **Selectively forward Sybil references.** When the protocol calls for the adversary to "pick a random subset" of its view to send during a gossip exchange, the adversary instead deliberately picks entries that point to its colluding Sybils first, and fills with honest entries only if it doesn't have enough Sybil ones. The receiver verifies signatures, ownership chains, and frequency timestamps on each forwarded descriptor — and they all check out, because every forwarded descriptor *is* a real descriptor that propagated through legitimate gossip.

Together these two private deviations cause honest nodes' views to accumulate adversary references far above the adversary's population share. Eventually: within tens of cycles — honest views become majority-adversary or entirely-adversary, and downstream protocols inherit those biased samples.

The attack works **because the steps Cyclon designates "uniformly at random" — what to keep in the view, what to forward — are private to the participant**. Any defence that authenticates *what* is sent leaves a hole for an adversary that lies about *what was randomly selected*.

## 2. The attack at a technical level

### 2.1 Notation

- `N` — number of nodes.
- `c` — view length (cache size).
- `ℓ` — shuffle length (the size of the subset gossiped per exchange), with `1 ≤ ℓ ≤ c`.
- A node's *view* is an ordered set of pairs `(id, age)` of size at most `c`.
- *Adversary set* `M ⊂ {0,…,N−1}`, `|M| = k`, with adversary fraction `μ = k/N`.
- *Honest set* `H = {0,…,N−1} \ M`.

Cyclon Enhanced Shuffling (Voulgaris et al. 2005) prescribes the following per-cycle protocol for an initiator `P`:

1. Increment the age of every entry in `view_P`.
2. Let `Q` be the entry in `view_P` of maximum age. Let `S` be a *uniformly random* subset of `view_P \ {Q}` of size `ℓ−1`.
3. Send `{(P, 0)} ∪ S` to `Q`.
4. `Q` replies with a *uniformly random* subset `R ⊂ view_Q` of size `min(ℓ, |view_Q|)`.
5. Both sides merge: discard duplicates and self-pointers; fill empty slots first, then displace sent entries; truncate to size `c`.

### 2.2 The adversary

A node `v ∈ M` runs Steps 1–5 of the protocol exactly as written, with the following two internal deviations, parametrised by `(p_drop, bias_on)`:

**Attack (a) — link drop.** Once per cycle, after Steps 1–5 for all initiators have run, the adversary scans its own view and removes each *honest-pointing* entry `(u, age)` with `u ∈ H` independently with probability `p_drop`. The view shrinks; nothing is fabricated or replaced. Adversary-pointing entries are never removed by this step.
.

**Attack (b) — biased subset.** Whenever an adversary needs to compute a "uniformly random subset of size `n`" in Step 2 (the `ℓ−1` random others when initiating) or Step 4 (the reply of size `min(ℓ, |view|)` when responding), it instead computes:

```
  A := { i : view[i].id ∈ M } sorted by a random permutation
  H := { i : view[i].id ∈ H } sorted by a random permutation
  output := first n elements of (A concatenated with H)
```

i.e., pick all adversary-pointing entries first, fall back to honest-pointing entries only when there aren't enough adversary entries to fill the subset.

### 2.3 What stays observable

Every externally observable field of adversary is identical to an honest run.

The only difference is **which** entries the adversary stores and **which** entries the adversary selects from that storage — both private to the adversary's process.

## 3. Why SecureCyclon's defences do apply

SecureCyclon (Antonov & Voulgaris 2023) introduces nine distinct mechanisms across §IV–V — two deterministic detectors (D3, D4), one consequence (D5), and six mitigations / sanity checks (D1, D2, D6, D7, D8, D9). In Cyclon when the node has insufficient number of descriptors then it is allowed to keep some of the descriptors in their view to keep the view well-populated. These descriptors have no restriction on how they are used. In case of SecureCyclon this mechanism is extended with the notion of non-swappable descriptors which means that a node can use non-swappable descriptor to initiate the exchange but if it swaps it then it eventually leads to violation of no-cloning policy. 



## 4. Implementation and validation

The simulator `cyclon_sim_biased.py` encodes Cyclon Enhanced Shuffling in §2.1's seven steps faithfully. All `N` nodes — honest and adversarial — initiate exactly once per cycle. The adversary's protocol externally is byte-for-byte identical to honest; the only differences are `apply_drop` (attack a) and the `biased=True` branch of `pick_indices` (attack b).


**Baseline check.** With `(p_drop=0, bias_on=False)`, adversaries behave honestly. The simulator reproduces the canonical Cyclon property `A(t) ≈ μ`. This is the *uniform sample* property the paper credits to enhanced shuffling.


```
  A(t) := mean over honest nodes v ∈ H of  |{(u, age) ∈ view_v(t) : u ∈ M}| / |view_v(t)|
```

— the average fraction of adversary references in an honest node's view at cycle `t`. Per request, only honest views enter the metric; adversary views are tracked separately as a diagnostic.

## 5. Empirical evidence

All experiments at `N = 200, c = 20, ℓ = 10, T = 120, seeds = 8`. Adversary placement is uniformly random; adversaries collude (share `M`).

### 5.1 Headline: A(t) under full attack vs. baseline

Under the full attack `(p_drop = 1, bias_on = True)`:

| μ | Baseline A(T) ≈ μ | Attack A(T) ± σ | Amplification |
|---|---|---|---|
| 0.00 | 0.000 | 0.000 | — |
| 0.05 | 0.050 | 0.124 ± 0.009 | **2.5×** |
| 0.10 | 0.102 | 0.400 ± 0.014 | **3.9×** |
| 0.15 | 0.152 | 0.933 ± 0.020 | **6.1×** |

At μ = 0.05, *one in every twenty nodes Sybil* causes one in every eight references in honest views to be adversary-pointing. At μ = 0.15 over 93% of honest references already point to adversaries.


### 5.2 Persistence under continued operation

Extending the run to `T = 300` shows the attack reaches a stable equilibrium with no decay. After mixing within ~60 cycles, `A(t)` stays at the equilibrium indefinitely.

| μ | A(60) | A(120) | A(200) | A(300) |
|---|---|---|---|---|
| 0.05 | 0.128 | 0.126 | 0.134 | 0.125 |
| 0.10 | 0.407 | 0.396 | 0.405 | 0.407 |
| 0.20 | 0.997 | 0.995 | 0.997 | 0.996 |


### 5.3 Sweep with μ

Sweeping `μ ∈ {0.02, 0.04, …, 0.34}` at `(N=200, c=20, ℓ=10, T=120, seeds=8)`

| μ | Baseline A(T) | Attack A(T) ± σ |
|---|---|---|
| 0.02 | 0.019 | 0.030 ± 0.006 |
| 0.04 | 0.040 | 0.096 ± 0.006 |
| 0.06 | 0.059 | 0.164 ± 0.006 |
| 0.08 | 0.080 | 0.268 ± 0.010 |
| 0.10 | 0.100 | 0.412 ± 0.009 |
| 0.12 | 0.120 | 0.606 ± 0.012 |
| 0.14 | 0.140 | 0.850 ± 0.020 |
| 0.16 | 0.160 | 0.973 ± 0.008 |
| **0.18** | 0.179 | **0.990 ± 0.004** |
| 0.20 | 0.198 | 0.996 ± 0.001 |
| 0.25 | 0.248 | 1.000 ± 0.000 |
| 0.30 | 0.300 | 1.000 ± 0.000 |
| 0.34 | 0.341 | 1.000 ± 0.000 |


### 5.4 Which attack does what (at μ = 0.20)


| Configuration | A(T=120) | Mechanism |
|---|---|---|
| Neither (baseline) | 0.203 | natural mixing equilibrium |
| Drop only (`p_drop=1, bias_on=False`) | 0.672 | adversary view becomes all-adversary; *random* picks from it then deliver mostly adversary refs |
| Bias only (`p_drop=0, bias_on=True`) | 0.291 | view stays diverse, biased pick concentrates adversary refs — but only the few that happen to be in view |
| Both (full attack) | 0.995 | synergistic: drop makes view adversary-dominated, bias picks them all |





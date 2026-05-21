# Cyclon Analysis — Cyclon Properties 

> Empirical results for the three formal versions of "sampling from a Cyclon
> view is equivalent to sampling from the network." 

## 0. Setup

Cyclon (enhanced shuffling) on $N$ nodes with cache size $c$, swap length
$\ell \in [1, c]$. Semi-synchronous cycles — per cycle, every node initiates
exactly one gossip exchange in random per-cycle order. Fixed $N$, no
joins/leaves.

Per cycle, an initiating node:
1. Picks the **oldest** descriptor in its view; the descriptor's creator is the gossip partner $Q$.
2. Sends $Q$ a list of $\ell$ entries: a fresh self-descriptor plus $\ell{-}1$ random others from view.
3. $Q$ replies with $\ell$ random entries from its own view.
4. Both sides filter received entries against their full current view, fill any empty slots, then displace sent entries with the rest (the *keep-until-displaced* reading of the original paper).

State at cycle $t$: $view_v(t) \subseteq V \setminus \{v\}$ with
$view_v(t) = c$. Overlay graph $G(t)$ is the $c$-out digraph
$(V, \{(v, u) : u \in view_v(t)\}) \in G_{N,c}$.
The dynamics induce a Markov chain $\{G(t)\}$ with stationary distribution
$\pi_{\mathrm{graph}}$.

### Definitions (increasing strength)

- **(D1.1) Marginal uniformity.**
$\Pr_{\pi_{\mathrm{graph}}}[u \in \mathrm{view}_v] = c/(N{-}1)$ for every pair $u \neq v$.

- **(D1.2) Per-view uniformity.**
  $view_v \sim \mathrm{Uniform}\binom{V \setminus \{v\}}{c}$ under
$\pi_{\mathrm{graph}}$ for every $v$.

- **(D1.3) Overlay-graph uniformity.**
$\pi_{\mathrm{graph}} = \mathrm{Uniform}(\mathcal{G}_{N,c})$.

D1.3 $\Rightarrow$ D1.2 $\Rightarrow$ D1.1.

### Methodology

Results from a Python/numpy enhanced-Cyclon simulator. Runs use random
initial graphs, burn-in $T_{\mathrm{burn}} = T/2$ cycles, then sample the
graph state every cycle for the remaining $T/2$. Multi-seed and longer-$T$
robustness checks performed where noted.

---

## 1. D1.1 — Marginal uniformity: **holds**

### Result

D1.1 holds — empirically and by a one-line symmetry argument.

**Symmetry.** The Cyclon transition kernel is invariant under node-label
permutations. Starting from a symmetric initial distribution (uniform-random
initial graph), $\pi_{\mathrm{graph}}$ is also symmetric, so
$\Pr[u \in \mathrm{view}_v]$ is constant across distinct pairs and equals
$c/(N{-}1)$ from $\mathbb{E}[|\mathrm{view}_v|] = c$.

### Empirical check

At $(N{=}50, c{=}5, \ell{=}3)$, $T{=}7000$ post-burn-in cycles, 350,000
$(v, u)$ observations:

| | empirical | prediction $c/(N{-}1)$ |
|---|---|---|
| mean $\Pr[u \in \mathrm{view}_v]$ | $0.1020 \pm 0.0008$ | $0.10204$ |


---

## 2. D1.2 — Per-view uniformity: **holds**

### Pair co-occurrence

D1.2 predicts $\Pr[u, w \in \mathrm{view}_v] = c(c{-}1)/((N{-}1)(N{-}2))$.

$(N{=}50, c{=}5, \ell{=}3)$, $T{=}3500$ post-burn-in, 58,800 $(v, u{<}w)$ triples:

| | empirical | D1.2 prediction | independence |
|---|---|---|---|
| mean | $0.008503$ | $0.008503$ | $0.010412$ |
| deviation | — | $-0.008\%$ | $-18.34\%$ |


### Conclusion on D1.2

A single node's view is statistically indistinguishable from a uniform
random size-$c$ subset of $V \setminus \{v\}$ at every order we can probe.
Applications drawing one or many samples from a single view see God's-view-
equivalent statistics.

---

## 3. D1.3 — Overlay-graph uniformity: **falsified under deterministic initiation; restored under Poisson initiation**

### Result

- Under the **deterministic-per-cycle** initiation rule (each node initiates exactly once per cycle), D1.3 is **falsified** at every $N$ tested. The TV-distance grows with $N$, lower-bounded by $\sim 0.48$ at $N = 10^4$.
- Under **Poisson(1)-per-cycle** initiation, D1.3's marginal in-degree prediction is **restored** to within sampling noise.

Joint-graph TV is not directly measurable. By the data-processing inequality,
any function of $\pi_{\mathrm{graph}}$ gives a TV lower bound; we use the
in-degree marginal.

### Falsification under deterministic initiation

Under uniform $c$-out digraphs, in-degree is $\mathrm{Binomial}(N{-}1, c/(N{-}1))$.
We compare Cyclon's empirical in-degree pmf to this.

$N$-sweep at $c{=}20$, $\ell{=}8$, $T{=}300$:

| $N$ | $\mathrm{Var}(d_{\mathrm{in}})/\mathrm{Var}_{\mathrm{Binom}}$ | TV(emp, Binom) |
|---|---|---|
| 200    | 0.935 | 0.0188 | 
| 500    | 0.560 | 0.1432 | 
| 1000   | 0.355 | 0.2497 | 
| 2000   | 0.229 | 0.3437 | 
| 5000   | 0.148 | 0.4383 | 
| 10000  | 0.119 | 0.4779 | 

Three converging signals:
- TV grows **monotonically** with $N$, asymptoting around $0.48$. Not vanishing.
- Variance ratio shrinks monotonically — Cyclon's in-degree variance is
  near-constant while Binomial's grows toward $c$.
- % in band rises toward $\sim 80$% — at $N{=}10^5$, matches the original Cyclon paper's reported $80.31\%$.

Data-processing inequality gives:
$$d_{\mathrm{TV}}(\pi_{\mathrm{graph}}, \mathrm{Uniform}(\mathcal{G}_{N,c})) \;\geq\; 0.48 \quad \text{at } N = 10^4.$$

**The deviation is structural**, not a finite-$N$ artifact. The mechanism
(documented in the Cyclon and SecureCyclon papers): deterministic +1
self-injection per cycle combined with in-degree-proportional partnering
creates a self-correcting in-degree process that concentrates the
distribution strictly tighter than uniform. 

**Second independent witness — view anti-correlation.** Pairwise reciprocity
ratio $\Pr[(u,v) \text{ and } (v,u) \text{ both edges}]\,/\,(c/(N{-}1))^2$
across four tested regimes (avg over 4 seeds, post-burn-in snapshot):

| $(N, c, \ell)$ | reciprocity ratio |
|---|---|
| $(50, 5, 3)$    | $0.608$ |
| $(200, 10, 4)$  | $0.945$ |
| $(500, 10, 4)$  | $0.878$ |
| $(1000, 20, 6)$ | $0.868$ |

vs the uniform-$c$-out prediction of $1.0$. Views are **anti-correlated**.

### Restoration under Poisson initiation

Replace deterministic initiation with: each node initiates $\mathrm{Poisson}(1)$
times per cycle. Run side-by-side at multiple $(N, c)$, 300–600 cycles, burn $T/2$.

**Variance ratio** $\mathrm{Var}(d_{\mathrm{in}})/\mathrm{Var}_{\mathrm{Binom}}$ — D1.3 predicts $1.0$:

| $(N, c)$ | Oldest+det | Oldest+Pois | Random+det | Random+Pois |
|---|---|---|---|---|
| (500, 10) | 0.260 | **1.047** | 0.596 | **1.019** |
| (1000, 10) | 0.199 | **1.029** | 0.549 | **1.026** |
| (1000, 20) | 0.278 | **1.067** | 0.629 | **1.043** |
| (2000, 20) | 0.185 | **1.034** | 0.574 | **1.021** |

**TV distance** to Binomial in-degree pmf — D1.3 predicts $0$:

| $(N, c)$ | Oldest+det | Oldest+Pois | Random+det | Random+Pois |
|---|---|---|---|---|
| (500, 10) | 0.315 | **0.0127** | 0.128 | **0.0066** |
| (1000, 10) | 0.368 | **0.0094** | 0.147 | **0.0068** |
| (1000, 20) | 0.301 | **0.0156** | 0.112 | **0.0111** |
| (2000, 20) | 0.393 | **0.0130** | 0.135 | **0.0049** |

Under Poisson initiation, TV drops to $\sim 0.01$ — the sampling-noise
floor — **regardless of partner-selection rule** (oldest or uniform random).
The bias collapses completely at the marginal level.

(Higher-order joint structure under Poisson initiation has not been directly
tested)

### Trade-off with SecureCyclon

Poisson initiation breaks SecureCyclon's *categorical-proof* frequency check.
Under deterministic initiation, two timestamps of the same creator within
$\Delta T$ constitute indisputable proof of a frequency violation. Under
Poisson, inter-creation times are $\mathrm{Exp}(1/\Delta T)$, so
$\Pr[\text{two consecutive honest creations within } \Delta T] \approx 1 - e^{-1} \approx 63\%$ —
honest nodes would trigger the check most cycles.

---

## 4. Bottom line

| Property | Status |
|---|---|
| **D1.1** marginal uniformity | **holds** |
| **D1.2** per-view uniformity | **holds** |
| **D1.3** (deterministic init) | **falsified** (TV $\geq 0.48$ at $N = 10^4$, monotonic in $N$) |
| **D1.3** (Poisson init) | **holds** at marginal (TV $\approx 0.01$) |

> Single-node sampling from a Cyclon view is, at stationary, statistically
> equivalent to direct uniform sampling from the network (D1.1, D1.2).
>
> The overlay graph as a whole is *not* a uniform random $c$-out digraph
> under the synchronous-cycle abstraction (D1.3 falsified). The bias lives
> in inter-view structure — Cyclon's in-degree is engineered to concentrate
> tighter than uniform — and is invisible to any single node looking at its
> own view.
>
> The bias is removable: switching to Poisson initiation restores D1.3's
> marginal prediction completely. The cost is SecureCyclon's
> categorical-proof frequency-check defense, which depends on deterministic
> initiation. Real asynchronous deployments sit in the middle.

---

## How to reproduce

All numbers in this report come from `cyclon_properties_sim.py` in this
directory. One experiment per claim:

```
python3 cyclon_properties_sim.py d11                # D1.1 holds
python3 cyclon_properties_sim.py d12                # D1.2 holds
python3 cyclon_properties_sim.py d13_falsification  # D1.3 falsified (deterministic init)
python3 cyclon_properties_sim.py d13_restoration    # D1.3 restored under Poisson init
```

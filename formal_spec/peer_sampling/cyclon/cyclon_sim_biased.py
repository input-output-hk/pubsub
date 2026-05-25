"""
cyclon_sim_biased.py — Cyclon under undetectable adversarial deviations.

Adversarial model
-----------------
Adversarial nodes run the Cyclon enhanced shuffling protocol *exactly*
(initiate every cycle, age entries, pick oldest as partner, send (self,0) +
random others, reply with random subset of view, integrate received entries
following the standard merge rules). They are externally indistinguishable
from honest peers — every message they send is well-formed and, in a
SecureCyclon deployment, can carry valid signatures.

The deviations are purely internal-state:

  (a) Link drop, parameterised by p_drop ∈ [0, 1].
      Once per cycle, after the adversary's exchanges have run, each
      honest-pointing entry in the adversary's view is deleted with
      probability p_drop, "as if it had never been there". The view shrinks;
      nothing is fabricated or replaced. Adversary-pointing entries are
      never deleted.

  (b) Biased subset, parameterised by bias_on ∈ {True, False}.
      When the adversary must choose an ℓ−1 or ℓ subset of its view to
      send during an exchange (the step the protocol specifies as uniform
      random), the adversary instead picks all adversary-pointing entries
      first and fills the remainder with honest-pointing entries chosen at
      random. Applies both when the adversary initiates and when it replies.

Both attacks are silent — they do not alter any externally observable
message field. They are undetectable by signature, ownership-chain, age,
or frequency checks.

Measurement
-----------
A(t) := mean over honest nodes v of (# adversary refs in view_v(t)) / |view_v(t)|.
Only honest nodes' views enter the metric; adversary views are tracked but
not measured.

Usage
-----
    python cyclon_sim_biased.py
    python cyclon_sim_biased.py --save out.png
    python cyclon_sim_biased.py --mus 0.05,0.10,0.20 --p_drop 1.0 --bias_on 1
    python cyclon_sim_biased.py --p_drop 0.5 --bias_on 1    # weaker attack
    python cyclon_sim_biased.py --p_drop 0   --bias_on 0    # baseline (honest)
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass

import numpy as np


# ---------------------------------------------------------------------------
# Subset picking — uniform or biased
# ---------------------------------------------------------------------------

def pick_indices(view: list[tuple[int, int]],
                 n: int,
                 *,
                 biased: bool,
                 adv_set: set[int],
                 rng: np.random.Generator,
                 exclude_idx: int | None = None) -> list[int]:
    """Pick `n` indices from `view` (excluding `exclude_idx` if given).

    `biased=False`  → uniform random without replacement.
    `biased=True`   → adversary-pointing entries first, then honest fillers.
    """
    candidates = [i for i in range(len(view)) if i != exclude_idx]
    if len(candidates) <= n:
        return candidates

    if biased:
        adv_cands = [i for i in candidates if view[i][0] in adv_set]
        hon_cands = [i for i in candidates if view[i][0] not in adv_set]
        if adv_cands:
            adv_cands = list(rng.permutation(adv_cands))
        if hon_cands:
            hon_cands = list(rng.permutation(hon_cands))
        picked = adv_cands[:n]
        if len(picked) < n:
            picked = picked + hon_cands[:n - len(picked)]
        return [int(i) for i in picked]
    chosen = rng.choice(candidates, size=n, replace=False)
    return [int(i) for i in chosen]


def apply_drop(view: list[tuple[int, int]],
               adv_set: set[int],
               p_drop: float,
               rng: np.random.Generator) -> list[tuple[int, int]]:
    """Delete each honest-pointing entry with probability p_drop. View shrinks."""
    if p_drop <= 0:
        return view
    survivors = []
    for entry in view:
        u, _ = entry
        if u not in adv_set and rng.random() < p_drop:
            continue
        survivors.append(entry)
    return survivors


# ---------------------------------------------------------------------------
# Cyclon enhanced exchange
# ---------------------------------------------------------------------------

def initial_views_random(N: int, c: int, rng: np.random.Generator
                         ) -> dict[int, list[tuple[int, int]]]:
    """Each node picks c uniform random others as initial view (age 0)."""
    views: dict[int, list[tuple[int, int]]] = {}
    for v in range(N):
        others = np.array([u for u in range(N) if u != v])
        chosen = rng.choice(others, size=c, replace=False)
        views[v] = [(int(u), 0) for u in chosen]
    return views


def cyclon_exchange(P: int,
                    views: dict[int, list[tuple[int, int]]],
                    c: int, ell: int,
                    rng: np.random.Generator,
                    is_adv: np.ndarray,
                    adv_set: set[int],
                    bias_on: bool) -> None:
    """One Cyclon enhanced exchange initiated by P. Mutates `views`.

    If P is adversarial and `bias_on`, P's ℓ−1 random others are picked with
    bias. Same for Q's reply if Q is adversarial. The merge step is identical
    in both cases — the only protocol-visible difference is *which* entries
    end up in the sent batches.

    Link dropping (attack (a)) is applied separately, once per cycle, by
    `one_cycle`.
    """
    if not views[P]:
        return

    view_P = [(u, age + 1) for (u, age) in views[P]]
    m_P = len(view_P)

    # Pick partner Q = oldest entry
    oldest_idx = int(np.argmax([age for (_, age) in view_P]))
    Q = view_P[oldest_idx][0]

    # P picks ℓ−1 random others to send (biased if P is adversarial)
    n_others = min(ell - 1, m_P - 1)
    p_biased = bool(is_adv[P]) and bias_on
    random_sent_idx = pick_indices(view_P, n_others,
                                    biased=p_biased, adv_set=adv_set,
                                    rng=rng, exclude_idx=oldest_idx)
    sent_idx_set = set(random_sent_idx) | {oldest_idx}
    sent_from_P = [(P, 0)] + [view_P[i] for i in random_sent_idx]

    # Q's reply (biased if Q is adversarial)
    view_Q = views[Q]
    m_Q = len(view_Q)
    n_reply = min(ell, m_Q)
    q_biased = bool(is_adv[Q]) and bias_on
    reply_idx = pick_indices(view_Q, n_reply,
                              biased=q_biased, adv_set=adv_set,
                              rng=rng)
    sent_from_Q = [view_Q[i] for i in reply_idx]

    # --- P updates view ---
    kept_P = [view_P[i] for i in range(m_P) if i not in sent_idx_set]
    in_view_P = {u for (u, _) in kept_P}
    accepted_P: list[tuple[int, int]] = []
    for (u, age) in sent_from_Q:
        if u == P or u in in_view_P:
            continue
        accepted_P.append((u, age))
        in_view_P.add(u)
        if len(kept_P) + len(accepted_P) >= c:
            break
    new_view_P = kept_P + accepted_P
    # Refill (retain unfilled sent randoms) if short of c
    if len(new_view_P) < c:
        for i in random_sent_idx:
            if len(new_view_P) >= c:
                break
            (u, age) = view_P[i]
            if u not in in_view_P:
                new_view_P.append((u, age))
                in_view_P.add(u)
    views[P] = new_view_P[:c]

    # --- Q updates view ---
    sent_set_Q = set(reply_idx)
    kept_Q = [view_Q[i] for i in range(m_Q) if i not in sent_set_Q]
    in_view_Q = {u for (u, _) in kept_Q}
    accepted_Q: list[tuple[int, int]] = []
    for (u, age) in sent_from_P:
        if u == Q or u in in_view_Q:
            continue
        accepted_Q.append((u, age))
        in_view_Q.add(u)
        if len(kept_Q) + len(accepted_Q) >= c:
            break
    new_view_Q = kept_Q + accepted_Q
    if len(new_view_Q) < c:
        for i in reply_idx:
            if len(new_view_Q) >= c:
                break
            (u, age) = view_Q[i]
            if u not in in_view_Q:
                new_view_Q.append((u, age))
                in_view_Q.add(u)
    views[Q] = new_view_Q[:c]


def one_cycle(views: dict[int, list[tuple[int, int]]],
              c: int, ell: int,
              rng: np.random.Generator,
              is_adv: np.ndarray,
              adv_set: set[int],
              bias_on: bool,
              p_drop: float) -> None:
    """Every node initiates one exchange this cycle, in random order.
    After all exchanges, each adversary applies link drop to its view.
    """
    N = len(views)
    order = list(rng.permutation(N))
    for P in order:
        cyclon_exchange(P, views, c, ell, rng, is_adv, adv_set, bias_on)
    if p_drop > 0:
        for v in range(N):
            if is_adv[v]:
                views[v] = apply_drop(views[v], adv_set, p_drop, rng)


# ---------------------------------------------------------------------------
# Metric
# ---------------------------------------------------------------------------

def adversary_fraction_per_view(views, is_adv_real, honest_ids) -> np.ndarray:
    """Per-honest-node fraction of adversary refs in the view."""
    out = []
    for P in honest_ids:
        view = views[P]
        if not view:
            out.append(0.0)
            continue
        k = sum(1 for (u, _) in view if is_adv_real[u])
        out.append(k / len(view))
    return np.array(out)


def A_of_t(views, is_adv_real, honest_ids) -> float:
    return float(adversary_fraction_per_view(views, is_adv_real, honest_ids).mean())


def mean_honest_view_size(views, honest_ids) -> float:
    return float(np.mean([len(views[v]) for v in honest_ids]))


def mean_adv_view_size(views, adv_ids) -> float:
    if not adv_ids:
        return 0.0
    return float(np.mean([len(views[v]) for v in adv_ids]))


# ---------------------------------------------------------------------------
# Experiment driver
# ---------------------------------------------------------------------------

@dataclass
class ExperimentConfig:
    N: int = 100
    c: int = 20
    ell: int = 10
    T: int = 80
    mu: float = 0.10
    seed: int = 0
    bias_on: bool = True
    p_drop: float = 1.0


def run_one(cfg: ExperimentConfig):
    rng = np.random.default_rng(cfg.seed)
    k = int(round(cfg.N * cfg.mu))
    perm = rng.permutation(cfg.N)
    adv_set = set(int(x) for x in perm[:k])
    honest_ids = [v for v in range(cfg.N) if v not in adv_set]
    is_adv_real = np.array([v in adv_set for v in range(cfg.N)], dtype=bool)

    views = initial_views_random(cfg.N, cfg.c, rng)
    A_trace = [A_of_t(views, is_adv_real, honest_ids)]
    adv_view_size = [mean_adv_view_size(views, list(adv_set))]
    for _ in range(cfg.T):
        one_cycle(views, cfg.c, cfg.ell, rng, is_adv_real, adv_set,
                  cfg.bias_on, cfg.p_drop)
        A_trace.append(A_of_t(views, is_adv_real, honest_ids))
        adv_view_size.append(mean_adv_view_size(views, list(adv_set)))
    per_view_final = adversary_fraction_per_view(views, is_adv_real, honest_ids)
    return (np.array(A_trace), per_view_final, np.array(adv_view_size))


def sweep_mu(cfg_template: ExperimentConfig,
             mus: list[float],
             seeds: int):
    out = {}
    for mu in mus:
        traces, finals, vsizes = [], [], []
        for s in range(seeds):
            cfg = ExperimentConfig(N=cfg_template.N, c=cfg_template.c,
                                   ell=cfg_template.ell, T=cfg_template.T,
                                   mu=mu, seed=s,
                                   bias_on=cfg_template.bias_on,
                                   p_drop=cfg_template.p_drop)
            tr, pv, vs = run_one(cfg)
            traces.append(tr); finals.append(pv); vsizes.append(vs)
        traces = np.array(traces)
        vsizes = np.array(vsizes)
        out[mu] = dict(
            mean=traces.mean(axis=0),
            std=traces.std(axis=0),
            per_view=np.concatenate(finals),
            adv_view_size=vsizes.mean(axis=0),
        )
    return out


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main() -> None:
    p = argparse.ArgumentParser(
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
        description='Cyclon under undetectable adversarial deviations.')
    p.add_argument('--N', type=int, default=100)
    p.add_argument('--c', type=int, default=20)
    p.add_argument('--ell', type=int, default=10)
    p.add_argument('--T', type=int, default=80)
    p.add_argument('--seeds', type=int, default=5)
    p.add_argument('--mus', type=str, default='0.0,0.05,0.10,0.20,0.30')
    p.add_argument('--p_drop', type=float, default=1.0,
                   help='per-cycle probability of dropping each honest entry from an adversary view')
    p.add_argument('--bias_on', type=int, default=1,
                   help='1 = biased subset (adv first), 0 = uniform random')
    p.add_argument('--save', type=str, default='cyclon_sim_biased.png')
    args = p.parse_args()

    mus = [float(x) for x in args.mus.split(',')]
    cfg = ExperimentConfig(N=args.N, c=args.c, ell=args.ell, T=args.T,
                           bias_on=bool(args.bias_on),
                           p_drop=args.p_drop)

    print(f'Cyclon biased-adversary simulation:')
    print(f'  N={args.N}, c={args.c}, ℓ={args.ell}, T={args.T}, seeds={args.seeds}')
    print(f'  p_drop = {args.p_drop}, bias_on = {bool(args.bias_on)}')
    print()
    runs = sweep_mu(cfg, mus, args.seeds)
    print('  μ      A(0)      A(T/2)    A(T)      adv |view|(T)')
    for mu in mus:
        r = runs[mu]
        tr = r['mean']
        vs = r['adv_view_size']
        print(f'  {mu:0.2f}   {tr[0]:0.4f}   {tr[args.T//2]:0.4f}   {tr[-1]:0.4f}'
              f'   {vs[-1]:0.2f}')

    # Plot
    import matplotlib.pyplot as plt
    cmap = plt.get_cmap('viridis')
    fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(13, 5))
    for i, mu in enumerate(mus):
        r = runs[mu]
        colour = cmap(i / max(1, len(mus) - 1))
        ax1.plot(r['mean'], label=f'μ={mu:0.2f}', color=colour)
        ax1.fill_between(range(len(r['mean'])),
                         r['mean'] - r['std'], r['mean'] + r['std'],
                         alpha=0.15, color=colour)
        ax2.plot(r['adv_view_size'], label=f'μ={mu:0.2f}', color=colour)
    ax1.set_xlabel('cycle t')
    ax1.set_ylabel('A(t)  —  mean adversary-ref fraction in honest views')
    ax1.set_title(f'View bias  (p_drop={args.p_drop}, bias_on={bool(args.bias_on)})')
    ax1.grid(alpha=0.3); ax1.legend(); ax1.set_ylim(0, 1)
    ax2.set_xlabel('cycle t')
    ax2.set_ylabel('mean adversary view size')
    ax2.axhline(args.c, ls='--', color='grey', alpha=0.5, label=f'c={args.c}')
    ax2.set_title('Adversary view shrinkage from drop')
    ax2.grid(alpha=0.3); ax2.legend(); ax2.set_ylim(0, args.c * 1.1)
    fig.suptitle(f'Cyclon under undetectable deviations  (N={args.N}, c={args.c}, ℓ={args.ell})')
    plt.tight_layout()
    plt.savefig(args.save, dpi=120)
    print(f'\nfigure saved to {args.save}')


if __name__ == '__main__':
    main()

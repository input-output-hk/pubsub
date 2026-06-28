#!/usr/bin/env python3
"""reproduce_faithful.py — Silent attacks on SecureCyclon, FAITHFUL reproducer.

A self-contained driver over `securecyclon.py` (which is stdlib-only and was
validated against the paper's Fig 2/5/6 results). Crucially, the honest baseline
runs the FULL protocol — honest nodes keep their views full at ℓ via the §V-A
non-swappable repair (a simplified chain-tracker that omits it lets honest views
collapse to ~8.7/20 and badly inflates the eclipse) — plus tit-for-tat
one-at-a-time transfer, sample cache + the two provable checks (D3 frequency,
D4 clone), redemption cache, and proof-flooded blacklisting. Every attack here is
therefore measured against the fortified protocol, not a plain-Cyclon strawman.

This file + `securecyclon.py` are the whole auditable unit: no numpy / matplotlib
needed to RUN (only the standard library).

Attacks (combine via --attacks=A,B,C). All are silent: genuine, rate-honest,
single-chain descriptors only — the adversary varies only WHAT it sends, redeems,
or withholds, which SecureCyclon's checks cannot prove (see §II.C / §III).

    bias          Network-wide biased subset: a malicious node fills its swap
                  slots toward honest peers with adversary-pointing descriptors
                  first and hoards the legitimate ones it receives. Amplifies the
                  malicious link share toward the m/(n-m) ceiling. (No target.)

    concentrate   Targeted: hoard adversary ammo for the victim — reciprocate
                  legit-first to NON-victim honest peers (stay engaged, keep
                  acquiring victim-tokens) while reserving adversary descriptors
                  for the victim. Drives the victim's local malicious fraction
                  past the network-wide m/(n-m) average. (Needs a target.)

    refuse        Targeted + selective silence: a malicious node declines to
                  REPLY only when a NON-victim honest peer invites it (it still
                  engages the victim fully). Indistinguishable from churn; D8
                  tit-for-tat is a damage-limiter, not a detector. (Needs a target.)

    healer        Targeted: from the samples it already receives, the adversary
                  learns which honest peers currently hold a victim-token (the
                  victim's "healers") and targets them too, choking the victim's
                  re-heal supply at the source. (Needs a target.)

    token_dup     Targeted: LINEAR prefix-extension duplication of victim-tokens
                  across the coalition (A→B→C kept, A→B→C→D forwarded, …). Every
                  copy is a prefix of the next, so the D4 clone check never fires;
                  manufactures redeemable victim-tokens to multiply contact rate.
                  This is the one lever that silently breaks token conservation.
                  (Needs a target.)

Metrics reported (steady-state window):
    A_T_mean   victim's local malicious-view fraction (mean over window)
    A_T_max    its peak
    eclipse%   fraction of window cycles with A_T >= 0.80 (full-eclipse moments)
    A_mean     network-wide malicious link share (the m/(n-m) reference metric)
    view       honest avg_view_fill / min  (FAITHFULNESS check: must stay ≈ ℓ)
    in-deg     victim's honest in-degree (#honest nodes that can still reach it)
    D3 / D4    provable-violation detections (silent attacks => 0)

Examples:

    # Honest baseline (no attack) — shows views full and A_mean ≈ μ:
    python3 reproduce_faithful.py --mu 0.15

    # Headline targeted eclipse (concentration + selective refuse), multi-seed:
    python3 reproduce_faithful.py --mu 0.15 --attacks concentrate,refuse \\
        --seeds 1,2,3,4,5

    # Full μ-sweep of the headline:
    for mu in 0.05 0.10 0.15 0.20 0.30; do
        python3 reproduce_faithful.py --mu $mu --attacks concentrate,refuse --seeds 1,2,3
    done

    # Network-wide biased subset only (no target) — approaches m/(n-m):
    python3 reproduce_faithful.py --mu 0.20 --attacks bias

    # Strongest stack (adds healer-targeting + token duplication):
    python3 reproduce_faithful.py --mu 0.10 \\
        --attacks concentrate,refuse,healer,token_dup --seeds 1,2,3
"""

import argparse
import statistics
import sys

try:
    from securecyclon import Simulator, NodeKind
except ImportError:
    sys.exit("error: run from this folder (next to securecyclon.py), "
             "or add it to PYTHONPATH.")


TARGETED = {'concentrate', 'refuse', 'healer', 'token_dup'}
KNOWN_ATTACKS = {'bias'} | TARGETED


class Attacks:
    def __init__(self, s):
        names = {x.strip() for x in s.split(',') if x.strip()}
        unknown = names - KNOWN_ATTACKS
        if unknown:
            raise SystemExit(f"Unknown attack(s): {sorted(unknown)}. "
                             f"Known: {sorted(KNOWN_ATTACKS)}")
        self.bias        = 'bias' in names
        self.concentrate = 'concentrate' in names
        self.refuse      = 'refuse' in names
        self.healer      = 'healer' in names
        self.token_dup   = 'token_dup' in names
        self._names = names

    @property
    def any(self):
        return bool(self._names)

    @property
    def targeted(self):
        return bool(self._names & TARGETED)

    def enabled(self):
        return sorted(self._names) if self._names else ['(none — honest baseline)']


def pick_victim(sim, requested):
    """Return an HONEST victim id (eclipsing a malicious node is meaningless)."""
    honest = [i for i in range(sim.n) if i not in sim.mal_set]
    if not honest:
        raise SystemExit("no honest nodes (mu too high)")
    if requested is not None and requested >= 0 and requested not in sim.mal_set:
        return requested
    return honest[0]


def run_one(N, l, s, mu, target_req, atk, seed, cycles, attack_start, window):
    kind = NodeKind.BIAS if atk.any else NodeKind.HONEST
    sim = Simulator(
        n=N, view_len=l, swap=s, malicious_frac=mu,
        attack_kind=kind, attack_start=attack_start, seed=seed,
        eclipse_starve=atk.concentrate,
        eclipse_refuse_invites=atk.refuse,
        eclipse_token_dup=atk.token_dup,
        healer_from_samples=atk.healer,
        eclipse_hoard=atk.targeted,   # hoard victim-tokens (only meaningful when targeted)
    )
    victim = pick_victim(sim, target_req)
    if atk.targeted:
        sim.eclipse_targets = {victim}
    if atk.healer:
        sim.eclipse_victim = victim

    at, am, vfill, vmin, indeg = [], [], [], [], []
    honest_ids = [i for i in range(N) if i not in sim.mal_set]
    for c in range(cycles):
        sim.step()
        if c >= cycles - window:
            v = sim.nodes[victim]
            owned = list(v.view.values()) + list(v.nonswap.values())
            if owned:
                at.append(sum(1 for d in owned if d.creator in sim.mal_set) / len(owned))
            m = sim.metrics()
            am.append(m['mal_link_frac'])
            vfill.append(m['avg_view_fill'])
            vmin.append(m['min_view_fill'])
            ind = 0
            for h in honest_ids:
                if h == victim or h in sim.blacklist:
                    continue
                nd = sim.nodes[h]
                if victim in nd.view or victim in nd.nonswap:
                    ind += 1
            indeg.append(ind)
    if not at:
        at = [0.0]
    return {
        'victim': victim,
        'A_T_mean': statistics.fmean(at),
        'A_T_max': max(at),
        'eclipse': statistics.fmean(1.0 if x >= 0.80 else 0.0 for x in at),
        'A_mean': statistics.fmean(am),
        'view_mean': statistics.fmean(vfill),
        'view_min': min(vmin),
        'indeg': statistics.fmean(indeg),
        'det': len(sim.detections),
    }


def fmt_pm(xs):
    m = statistics.fmean(xs)
    sd = statistics.pstdev(xs) if len(xs) > 1 else 0.0
    return m, sd


def main():
    p = argparse.ArgumentParser(
        formatter_class=argparse.RawDescriptionHelpFormatter, description=__doc__)
    p.add_argument('--N', type=int, default=200, help='network size (default 200)')
    p.add_argument('--l', type=int, default=20, help='view length ℓ (default 20)')
    p.add_argument('--s', type=int, default=3, help='swap length s (paper default 3)')
    p.add_argument('--mu', type=float, default=0.15, help='Byzantine fraction (default 0.15)')
    p.add_argument('--cycles', type=int, default=200, help='cycles to simulate (default 200)')
    p.add_argument('--attack-start', type=int, default=50, help='attack activation cycle (default 50)')
    p.add_argument('--window', type=int, default=75, help='steady-state measurement window (last N cycles)')
    p.add_argument('--target', type=int, default=-1,
                   help='victim node id; -1 = first honest node (default)')
    p.add_argument('--attacks', type=str, default='',
                   help='comma-separated: ' + ', '.join(sorted(KNOWN_ATTACKS)))
    p.add_argument('--seed', type=int, default=1)
    p.add_argument('--seeds', type=str, default=None,
                   help='comma-separated seeds for multi-seed mean ± std')
    args = p.parse_args()

    atk = Attacks(args.attacks)
    seeds = [int(x) for x in args.seeds.split(',')] if args.seeds else [args.seed]
    tgt = None if args.target < 0 else args.target

    print(f"N={args.N}  ℓ={args.l}  s={args.s}  mu={args.mu}  cycles={args.cycles}  "
          f"attack_start={args.attack_start}  window={args.window}")
    print(f"Attacks: {atk.enabled()}    (faithful SecureCyclon: V-A repair, "
          f"tit-for-tat, D3/D4, blacklisting)")
    print()

    keys = ['A_T_mean', 'A_T_max', 'eclipse', 'A_mean', 'view_mean', 'view_min', 'indeg', 'det']
    acc = {k: [] for k in keys}
    victim = None
    for sd in seeds:
        r = run_one(args.N, args.l, args.s, args.mu, tgt, atk, sd,
                    args.cycles, args.attack_start, args.window)
        victim = r['victim']
        for k in keys:
            acc[k].append(r[k])
        if len(seeds) > 1:
            print(f"  seed={sd:>3}  A_T={r['A_T_mean']:.3f}  max={r['A_T_max']:.3f}  "
                  f"eclipse={100*r['eclipse']:5.1f}%  A_mean={r['A_mean']:.3f}  "
                  f"view={r['view_mean']:.1f}  indeg={r['indeg']:.1f}  D3/D4={r['det']}")

    k_mal = int(round(args.N * args.mu))
    ceil = k_mal / (args.N - k_mal)
    print()
    print(f"--- summary ({len(seeds)} seed{'s' if len(seeds) > 1 else ''}, "
          f"victim=node {victim}) ---")
    m, sd = fmt_pm(acc['A_T_mean']); print(f"  A_T_mean (victim mal-view)  = {m:.3f} ± {sd:.3f}   "
                                            f"[m/(n-m) = {ceil:.3f}]")
    m, sd = fmt_pm(acc['A_T_max']);  print(f"  A_T_max                     = {m:.3f} ± {sd:.3f}")
    m, sd = fmt_pm(acc['eclipse']);  print(f"  eclipse%  (A_T >= 0.80)     = {100*m:.1f} ± {100*sd:.1f}")
    m, sd = fmt_pm(acc['A_mean']);   print(f"  A_mean   (network share)    = {m:.3f} ± {sd:.3f}")
    m, sd = fmt_pm(acc['view_mean']);print(f"  honest view fill (avg)      = {m:.2f} / {args.l}   "
                                            f"(min seen {min(acc['view_min'])})")
    m, sd = fmt_pm(acc['indeg']);    print(f"  victim honest in-degree     = {m:.1f}")
    print(f"  D3+D4 detections            = {sum(acc['det'])} total across seeds")


if __name__ == '__main__':
    main()

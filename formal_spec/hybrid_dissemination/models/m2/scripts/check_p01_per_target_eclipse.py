#!/usr/bin/env python3
"""Per-target eclipse rate: validation of the exact closed form.

Validates

    P_ecl(j) = C(k,RF)/C(N-1,RF)   (~ mu^RF)

-- the probability that all RF of a node's pulled forwarders are (silent)
adversaries, i.e. the eclipse floor of M2/M3 coverage -- against the shared
M2 model (m2_model.py) in two ways:

  (1) Analytic identities of the closed form -- exact, no sampling:
      degenerate limits (k=0, k<RF, RF=0) and monotonicity in k.

  (2) Monte-Carlo -- the empirical per-target eclipse frequency across sampled
      M2 graphs vs the closed form, in several regimes, including an
      exact-zero regime (k < RF, where NO eclipse must ever be observed).

      Acceptance: |z| <= 4, where z = (p_MC - p_closed) / SE and SE is the
      across-trial standard error of mean(M)/H.

Exit code 0 iff all checks pass.
"""

import argparse
import math
import random
import sys

from m2_model import M2Params, mean_var, sample_graphs


# regimes: (label, params, default trials)
REGIMES = [
    ("running example        ", M2Params(N=20000, k=2000, RF=2), 100),
    ("stress RF=1 mu=0.30    ", M2Params(N=2000, k=600, RF=1), 2000),
    ("stress RF=1 mu=0.50    ", M2Params(N=2000, k=1000, RF=1), 2000),
    ("operating RF=2 mu=0.30 ", M2Params(N=2000, k=600, RF=2), 1000),
    ("high RF    RF=5 mu=0.40", M2Params(N=2000, k=800, RF=5), 2000),
    ("exact zero (k<RF)      ", M2Params(N=2000, k=1, RF=2), 200),
]


def check_identities() -> list:
    """Exact analytic identities of the closed form (no sampling)."""
    results = []

    p = M2Params(N=2000, k=0, RF=2)
    results.append(("k=0   => P = 0 (no feasibility floor)", p.p_eclipse() == 0.0, ""))

    p = M2Params(N=2000, k=1, RF=2)
    results.append(("k<RF  => P = 0 (pull factor empty)", p.p_eclipse() == 0.0, ""))

    p = M2Params(N=2000, k=600, RF=0)
    results.append(("RF=0  => P = 1 (no forwarders at all)", p.p_eclipse() == 1.0, ""))

    ps = [M2Params(N=2000, k=k, RF=2).p_eclipse() for k in range(2, 1999, 8)]
    mono = all(a < b for a, b in zip(ps, ps[1:]))
    results.append(("P strictly increasing in k (RF=2 grid)", mono, ""))

    p = M2Params(N=2000, k=1998, RF=2)   # k = N-2: all-but-one adversarial
    ref = math.comb(1998, 2) / math.comb(1999, 2)
    results.append(("hypergeometric == comb ratio at k=N-2",
                    math.isclose(p.p_eclipse(), ref, rel_tol=1e-12), ""))

    return results


def check_mc(trials_override, seed) -> list:
    """Monte-Carlo eclipse frequency vs closed form, per regime."""
    rng = random.Random(seed)
    results = []
    print(f"  {'regime':<24} {'trials':>6} {'p closed':>12} {'p MC':>12} "
          f"{'rel.diff':>9} {'z':>7}")
    for label, params, default_trials in REGIMES:
        trials = trials_override or default_trials
        p_closed = params.p_eclipse()
        counts = [g.eclipsed_count() for g in sample_graphs(params, trials, rng)]
        m, v = mean_var(counts)
        p_mc = m / params.H
        se = math.sqrt(v / trials) / params.H
        if p_closed == 0.0:
            # exact-zero regime: no eclipse may EVER be observed
            ok = max(counts) == 0
            print(f"  {label:<24} {trials:>6} {p_closed:>12.4e} {p_mc:>12.4e} "
                  f"{'--':>9} {'exact':>7}   {'ok' if ok else 'FAIL'}")
            results.append((f"MC {label.strip()}", ok,
                            f"max count {max(counts)} (must be 0)"))
            continue
        z = (p_mc - p_closed) / se if se > 0 else float("inf")
        rel = (p_mc - p_closed) / p_closed
        ok = abs(z) <= 4
        print(f"  {label:<24} {trials:>6} {p_closed:>12.4e} {p_mc:>12.4e} "
              f"{rel:>+9.3%} {z:>+7.2f}   {'ok' if ok else 'FAIL'}")
        results.append((f"MC {label.strip()}", ok, f"z={z:+.2f}"))
    return results


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--trials", type=int, default=None,
                    help="override the per-regime trial counts")
    ap.add_argument("--seed", type=int, default=12345)
    args = ap.parse_args()

    print("Per-target eclipse rate: validation")
    print("=" * 74)

    print("(1) analytic identities (exact)")
    id_results = check_identities()
    for name, ok, detail in id_results:
        print(f"  [{'ok  ' if ok else 'FAIL'}] {name}"
              + (f"  ({detail})" if detail else ""))
    print()

    print("(2) Monte-Carlo eclipse frequency vs exact closed form")
    mc_results = check_mc(args.trials, args.seed)
    print()

    failures = [n for n, ok, _ in id_results + mc_results if not ok]
    print("=" * 74)
    if failures:
        print(f"RESULT: {len(failures)} FAILURE(S): " + "; ".join(failures))
        return 1
    print("RESULT: PASS -- exact closed form confirmed in all regimes")
    return 0


if __name__ == "__main__":
    sys.exit(main())

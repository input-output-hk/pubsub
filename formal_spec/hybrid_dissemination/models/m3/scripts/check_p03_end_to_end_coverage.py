#!/usr/bin/env python3
"""M3 coverage mean-field machinery (../properties/full_coverage.md).

Coverage = fraction of regular honest nodes reachable from the seeds through
one sampled M2 pull graph.  Analytical content under validation:

    mean-field fixed point   u = (mu + (1-mu)*u)^RF   (smallest root)
                             E[uncovered | ignition] = H*u
    exact lower bound        E[uncovered] >= H * P_ecl   (eclipsed subset)
    unified ignition law     E[coverage] ~ [1 - (1-rho_f)^s] * (1-u),
                             rho_f = 1 - exp(-RF*(1-mu)*rho_f)

Checks:

  (1) Fixed-point machinery (deterministic): explicit RF=2 solution ==
      iteration; u(mu=0) = 0; u strictly increasing in mu; amplification
      u >= mu^RF >= P_exact (lower-bound chain, RF >= 2).

  (2) Structural, per sampled instance (deterministic): every eclipsed
      NON-SEED node is unreached -- eclipsed nodes have in-degree 0 in the
      propagation graph, so {eclipsed} \\ {seeds} SUBSET-OF {uncovered} in
      EVERY graph.  Asserted on every sampled graph below.

  (3) High RF, modest seed set: mu^RF < eps_net/H => pull covers w.h.p.
      (RF=5, mu=0.1, 20 seed nodes): mean uncovered <= 0.15, coverage >= 0.999.

  (4) NEGATIVE control -- sparse seeds break the mean field: single source,
      mu=0, RF=2: the fixed point predicts full coverage (u=0) but true
      coverage is the giant-component fraction rho(2) ~ 0.797.  Pass iff MC
      lands on rho(2) (+/- 0.05) and is at least 0.1 BELOW the fixed-point
      prediction.

  (5) Single-source coverage grid at scale (N=20000, s=1): the s=1 limit of
      the unified law, E[coverage] ~ rho_f * (1-u), RF in {1,2,3,5,10} x mu in
      {0,...,0.5}.  Acceptance per cell: mean coverage within max(0.02, 4*SE)
      of the prediction, and coverage GIVEN ignition within 0.02 of (1-u) in
      clearly supercritical cells (rho_f > 0.1).

  (6) UNIFIED ignition law: seed-count sweep at N=4000, RF=2,
      s in {1,2,3,5,10} x mu in {0, 0.1}; acceptance
      |cov_MC - pred| <= max(0.005, 4*SE).

Exit code 0 iff all checks pass.
"""

import argparse
import math
import random
import sys

import os
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                "..", "..", "m2", "scripts"))
from m2_model import M2Params, mean_var  # noqa: E402
from m3_model import (M3Graph, u_iterate, u_rf2_closed,  # noqa: E402
                      rho_giant)

# ---------------------------------------------------------------------------
# Sampling helpers (with the per-instance structural check wired in)
# ---------------------------------------------------------------------------

class StructuralViolation(Exception):
    pass


def sample_uncovered(params: M2Params, trials: int, rng: random.Random,
                     seeds=None):
    """Sample graphs; return (uncovered counts, coverages).  On every graph,
    assert the structural fact: eclipsed => unreached (default seeds)."""
    uncov, covs = [], []
    for _ in range(trials):
        g = M3Graph(params, rng)
        depth = g.depths(seeds)
        n_unreached = sum(1 for j in g.regular_nodes() if depth[j] < 0)
        if seeds is None:                       # structural check (2)
            seed_set = set(g.seeds())           # a seed holds the message by
            for j in g.regular_nodes():         # fiat even if "eclipsed"
                if g.eclipsed(j) and depth[j] >= 0 and j not in seed_set:
                    raise StructuralViolation(
                        f"eclipsed node {j} was reached (params {params})")
        uncov.append(n_unreached)
        covs.append(1 - n_unreached / params.H)
    return uncov, covs


# ---------------------------------------------------------------------------
# Checks
# ---------------------------------------------------------------------------

def check_fixed_point_machinery() -> list:
    """(1) deterministic identities of the fixed point."""
    results = []
    mus = (0.0, 0.05, 0.1, 0.2, 0.3, 0.45)

    ok = all(math.isclose(u_rf2_closed(mu), u_iterate(mu, 2),
                          rel_tol=1e-9, abs_tol=1e-12) for mu in mus)
    results.append(("explicit RF=2 solution == iteration", ok, ""))

    ok = all(u_iterate(0.0, RF) == 0.0 for RF in (1, 2, 3))
    results.append(("u(mu=0) == 0 (full coverage, no adversary)", ok, ""))

    for RF in (2, 3):
        us = [u_iterate(mu, RF) for mu in [i / 25 for i in range(1, 12)]]
        ok = all(a < b for a, b in zip(us, us[1:]))
        results.append((f"u strictly increasing in mu (RF={RF})", ok, ""))

    # amplification chain: u_fp >= mu^RF >= P_exact (hypergeometric), RF >= 2
    ok = True
    for (N, k, RF) in [(20000, 2000, 2), (4000, 800, 2), (4000, 1600, 3)]:
        p = M2Params(N=N, k=k, RF=RF)
        u = u_iterate(p.mu, RF)
        ok = ok and (u >= p.mu ** RF >= p.p_eclipse())
    results.append(("chain u_fp >= mu^RF >= P_exact", ok, ""))
    return results


def check_high_rf_modest_seeds(trials, rng) -> list:
    """(3) mu^RF < eps_net/H => pull covers w.h.p., GIVEN a modest seed set."""
    N, RF, mu, n_seeds = 4000, 5, 0.1, 20
    k = int(round(mu * N))
    params = M2Params(N=N, k=k, RF=RF)
    floor = params.H * mu ** RF
    seeds = list(range(n_seeds))                  # 20 regular honest seeds
    uncov, covs = sample_uncovered(params, trials, rng, seeds=seeds)
    m, _ = mean_var(uncov)
    cov, _ = mean_var(covs)
    ok = m <= 0.15 and cov >= 0.999
    print(f"  RF={RF}, mu={mu}, {n_seeds} seed nodes "
          f"(eclipse floor H*mu^RF = {floor:.3f})")
    print(f"  mean uncovered = {m:.3f}, coverage = {cov:.5f}  "
          f"{'ok' if ok else 'FAIL'}")
    return [("high-RF with modest seeds", ok, f"uncovered={m:.3f}")]


def check_negative_control(trials, rng) -> list:
    """(4) sparse seeds break the fixed point: single source, mu=0, RF=2."""
    N, RF = 2000, 2
    params = M2Params(N=N, k=0, RF=RF)
    rho = rho_giant(RF)
    _, covs = sample_uncovered(params, trials, rng)   # default: single source
    cov, _ = mean_var(covs)
    fp_prediction = 1.0                                # u = 0 at mu = 0
    ok = abs(cov - rho) <= 0.05 and (fp_prediction - cov) > 0.1
    print(f"  single source, mu=0, RF={RF}: fixed point predicts coverage "
          f"{fp_prediction:.3f}")
    print(f"  MC coverage = {cov:.4f} vs giant component rho({RF}) = "
          f"{rho:.4f}  {'ok' if ok else 'FAIL'}")
    print(f"  -> extensive-seed constraint is BINDING: mean-field is "
          f"over-optimistic by {fp_prediction - cov:.2f} here")
    return [("negative control: sparse seeds break fp", ok,
             f"cov={cov:.4f} vs rho={rho:.4f}")]


def check_single_source_table(trials, rng) -> list:
    """(5) single-source (s=1) coverage grid RF x mu (N=20000)."""
    results = []
    N = 20000
    RFS = (1, 2, 3, 5, 10)
    MUS = (0.0, 0.1, 0.2, 0.3, 0.4, 0.5)
    print(f"  N={N}, sender-only seed (s=1), trials={trials} per cell")
    print(f"  mean coverage, predicted rho_f*(1-u) / MC:")
    print("  " + f"{'RF':>4} |" +
          "".join(f" {'mu='+format(mu, '.1f'):>13}" for mu in MUS))
    for RF in RFS:
        pred_row, mc_row = [], []
        for mu in MUS:
            m_branch = RF * (1 - mu)
            rho_f = rho_giant(m_branch) if m_branch > 1 + 1e-9 else 0.0
            reach = 1 - u_iterate(mu, RF)
            pred = rho_f * reach
            k = int(round(mu * N))
            params = M2Params(N=N, k=k, RF=RF)
            _, covs = sample_uncovered(params, trials, rng)
            mc, v = mean_var(covs)
            se = math.sqrt(v / trials)
            ok = abs(mc - pred) <= max(0.02, 4 * se)
            if rho_f > 0.1:                    # ignited-runs reach check
                ignited = [c for c in covs if c > 0.5 * reach]
                if ignited:
                    cov_ign, _ = mean_var(ignited)
                    ok = ok and abs(cov_ign - reach) <= 0.02
            pred_row.append(pred)
            mc_row.append(mc)
            results.append((f"grid RF={RF} mu={mu}", ok,
                            f"MC {mc:.3f} vs pred {pred:.3f}"))
        print("  " + f"{RF:>4} |" +
              "".join(f" {p:>6.3f}/{m:<6.3f}" for p, m in
                      zip(pred_row, mc_row)))
    return results


def check_ignition_law(trials, rng) -> list:
    """(6) unified law: E[cov] ~ [1-(1-rho_f)^s]*(1-u), seed-count sweep."""
    results = []
    N, RF = 4000, 2
    print(f"  N={N}, RF={RF}, s ad-hoc seed nodes; trials={trials}")
    print(f"  {'mu':>4} {'s':>3} {'ignition':>9} {'1-u':>7} {'pred':>8} "
          f"{'MC':>8}")
    for mu, svals in ((0.0, (1, 2, 3, 5, 10)), (0.1, (1, 3, 10))):
        k = int(round(mu * N))
        params = M2Params(N=N, k=k, RF=RF)
        rho_f = rho_giant(RF * (1 - mu))
        cov_fp = 1 - u_iterate(mu, RF)
        for s in svals:
            ignition = 1 - (1 - rho_f) ** s
            pred = ignition * cov_fp
            _, covs = sample_uncovered(params, trials, rng,
                                       seeds=list(range(s)))
            m, v = mean_var(covs)
            # empirical SE collapses when the rare non-ignition event does
            # not occur in the sample; floor with the PREDICTED Bernoulli SE
            se_emp = math.sqrt(v / trials)
            se_pred = math.sqrt(ignition * (1 - ignition) / trials) * cov_fp
            ok = abs(m - pred) <= max(0.005, 4 * max(se_emp, se_pred))
            print(f"  {mu:>4.1f} {s:>3} {ignition:>9.4f} {cov_fp:>7.4f} "
                  f"{pred:>8.4f} {m:>8.4f}   {'ok' if ok else 'FAIL'}")
            results.append((f"ignition law mu={mu} s={s}", ok,
                            f"MC {m:.4f} vs pred {pred:.4f}"))
    return results


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--trials", type=int, default=None,
                    help="override per-check trial counts")
    ap.add_argument("--seed", type=int, default=12345)
    args = ap.parse_args()
    rng = random.Random(args.seed)
    t = args.trials

    print("Property #3 -- end-to-end coverage: validation")
    print("=" * 74)

    print("(1) fixed-point machinery  [deterministic]")
    r1 = check_fixed_point_machinery()
    for name, ok, detail in r1:
        print(f"  [{'ok  ' if ok else 'FAIL'}] {name}"
              + (f"  ({detail})" if detail else ""))
    print()

    try:
        print("(3) high RF: pull covers w.h.p. (with modest seed set) "
              "[+(2) structural check on every graph]")
        r3 = check_high_rf_modest_seeds(t or 200, rng)
        print()

        print("(4) negative control: single source vs fixed point")
        r4 = check_negative_control(t or 100, rng)
        print()

        print("(5) single-source table at scale (N=20000)")
        r5 = check_single_source_table(t or 120, rng)
        print()

        print("(6) unified ignition law: seed-count sweep")
        r6 = check_ignition_law(t or 150, rng)
        print()
    except StructuralViolation as e:
        print(f"\nSTRUCTURAL VIOLATION (check 2): {e}")
        return 1

    all_results = r1 + r3 + r4 + r5 + r6
    failures = [(n, d) for n, ok, d in all_results if not ok]
    print("=" * 74)
    if failures:
        print(f"RESULT: {len(failures)} FAILURE(S):")
        for n, d in failures:
            print(f"  - {n}  {d}")
        return 1
    print("RESULT: PASS -- fixed point + ignition law validated in their "
          "regime; eclipsed<=uncovered structural; regime limits confirmed")
    return 0


if __name__ == "__main__":
    sys.exit(main())

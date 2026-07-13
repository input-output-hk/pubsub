#!/usr/bin/env python3
"""The M2 dissemination model: sampler + exact single-round eclipse forms.

M2 (see ../README.md): pull relaying only, silent adversaries.  Each regular
honest node privately picks RF forwarders uniformly; a pick landing on an
adversary is a dead edge.  This module is deliberately about M2 ONLY -- the
single sampled graph and its per-target eclipse.  Multi-hop coverage,
delivery depth and initiation-link seeding are M3 concerns and live in
../m3/m3_model.py (which imports M2Graph from here).

Node layout:

    [0, H)   regular honest
    [H, N)   adversarial   (silent: never forward)

Edges of one sampled graph:

    regular pull:  f -> j   iff regular honest j picked f as one of its RF
                            forwarders; a pick on an adversary is a dead edge
                            (kept in the raw picks -- observables need it).

Provides:

    M2Params        parameters + exact single-round closed forms:
                    q_pull (= per-target eclipse probability), p_eclipse
    M2Graph         ONE sampled instance: raw picks + single-round eclipse
                    observables (pull_failed / eclipsed)
    sample_graphs   Monte-Carlo iterator of independent M2Graph instances
    mean_var        unbiased sample mean / variance

Run `python3 m2_model.py` for a self-test.  No dependencies beyond stdlib.
"""

from __future__ import annotations

import math
import random
from typing import Callable, Iterator, List, Optional, Sequence
from dataclasses import dataclass


# ---------------------------------------------------------------------------
# Parameters and exact single-round closed forms
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class M2Params:
    """M2 parameter set.  N = H + k with H >= 1 regular honest nodes."""

    N: int   # total nodes
    k: int   # adversarial (silent) nodes
    RF: int  # regular pull request count (uniform; per-node RF via rf_of)

    def __post_init__(self) -> None:
        if self.N < 2:
            raise ValueError("N must be >= 2")
        if not 0 <= self.k < self.N:
            raise ValueError("need at least one regular honest node (k < N)")
        if not 0 <= self.RF <= self.N - 1:
            raise ValueError("RF must be in [0, N-1]")

    # -- derived quantities --------------------------------------------------

    @property
    def H(self) -> int:
        """Regular honest node count."""
        return self.N - self.k

    @property
    def mu(self) -> float:
        """Adversarial fraction k/N."""
        return self.k / self.N

    # -- exact closed forms ----------------------------------------------------

    def q_pull(self, rf: Optional[int] = None) -> float:
        """Exact P(all rf pulled forwarders adversarial) = C(k,rf)/C(N-1,rf).

        Evaluated as the product prod_{i<rf} (k-i)/(N-1-i) for float stability.
        rf defaults to self.RF (pass a value for per-node budgets).
        """
        rf = self.RF if rf is None else rf
        if self.k < rf:
            return 0.0
        v = 1.0
        for i in range(rf):
            v *= (self.k - i) / (self.N - 1 - i)
        return v

    def p_eclipse(self, rf: Optional[int] = None) -> float:
        """Exact per-target eclipse probability:

            P_ecl = C(k,rf)/C(N-1,rf)  ~  mu^rf
        """
        return self.q_pull(rf)


# ---------------------------------------------------------------------------
# One sampled instance of the M2 random graph
# ---------------------------------------------------------------------------

class M2Graph:
    """One sample of the M2 random pull graph, with raw picks retained.

    Exposes the sampled edges and the SINGLE-ROUND eclipse observables only.
    Multi-hop reachability (coverage, delivery depth, sinks) is an M3 concern
    and lives on M3Graph in ../../m3/scripts/m3_model.py.

    rf_of: optional callable j -> RF_j giving a heterogeneous per-node pull
    budget; defaults to the uniform params.RF.
    """

    def __init__(self, params: M2Params, rng: random.Random,
                 rf_of: Optional[Callable[[int], int]] = None) -> None:
        self.params = params
        N = params.N
        self.adv_start = params.H
        pool = range(N - 1)  # sample from N-1 slots, index-shift skips self

        # regular pull picks (raw -- adversarial picks kept)
        self.pull_picks: List[List[int]] = [
            [r if r < j else r + 1
             for r in rng.sample(pool, params.RF if rf_of is None else rf_of(j))]
            for j in range(self.adv_start)
        ]

    # -- node classes ---------------------------------------------------------

    def is_regular(self, i: int) -> bool:
        return i < self.adv_start

    def is_adversarial(self, i: int) -> bool:
        return i >= self.adv_start

    def regular_nodes(self) -> range:
        return range(self.adv_start)

    # -- single-round observables ----------------------------------------------

    def picks_of(self, j: int) -> List[int]:
        """Raw pull picks of regular honest j (may include adversaries)."""
        return self.pull_picks[j]

    def pull_failed(self, j: int) -> bool:
        """True iff ALL of j's pulled forwarders are adversarial."""
        return all(f >= self.adv_start for f in self.picks_of(j))

    def eclipsed(self, j: int) -> bool:
        """Eclipse event: all of j's forwarders are (silent) adversaries."""
        return self.pull_failed(j)

    def eclipsed_count(self) -> int:
        """M = number of eclipsed regular honest nodes in this round."""
        return sum(1 for j in self.regular_nodes() if self.eclipsed(j))


# ---------------------------------------------------------------------------
# Monte-Carlo helpers
# ---------------------------------------------------------------------------

def sample_graphs(params: M2Params, trials: int, rng: random.Random,
                  rf_of: Optional[Callable[[int], int]] = None
                  ) -> Iterator[M2Graph]:
    """Yield `trials` independent M2Graph samples."""
    for _ in range(trials):
        yield M2Graph(params, rng, rf_of)


def mean_var(xs: Sequence[float]) -> tuple:
    """Unbiased sample mean and variance."""
    n = len(xs)
    m = sum(xs) / n
    v = sum((x - m) ** 2 for x in xs) / (n - 1) if n > 1 else 0.0
    return m, v


# ---------------------------------------------------------------------------
# Self-test (M2 only: closed forms + single-round eclipse sampler)
# ---------------------------------------------------------------------------

def _selftest() -> int:
    failures = 0

    def check(name: str, ok: bool, detail: str = "") -> None:
        nonlocal failures
        status = "ok  " if ok else "FAIL"
        print(f"  [{status}] {name}" + (f"  ({detail})" if detail else ""))
        if not ok:
            failures += 1

    print("m2_model self-test")
    print("=" * 70)

    # (1) closed forms vs math.comb (exact combinatorial reference)
    print("(1) closed forms vs math.comb")
    for (N, k, RF) in [(2000, 600, 1), (2000, 1000, 1), (20000, 2000, 2),
                       (500, 100, 3), (100, 40, 2), (100, 0, 2), (100, 1, 2)]:
        p = M2Params(N=N, k=k, RF=RF)
        ref = (math.comb(k, RF) / math.comb(N - 1, RF)) if k >= RF else 0.0
        ok = math.isclose(p.q_pull(), ref, rel_tol=1e-12, abs_tol=1e-300)
        check(f"q_pull N={N} k={k} RF={RF}", ok,
              f"{p.q_pull():.6e} vs {ref:.6e}")

    # (2) identities
    print("(2) identities")
    check("k=0  => p_eclipse = 0",
          M2Params(N=2000, k=0, RF=2).p_eclipse() == 0.0)
    check("k<RF => p_eclipse = 0",
          M2Params(N=2000, k=1, RF=2).p_eclipse() == 0.0)
    check("RF=0 => p_eclipse = 1 (no forwarders at all)",
          M2Params(N=2000, k=600, RF=0).p_eclipse() == 1.0)
    ps = [M2Params(N=2000, k=k, RF=2).p_eclipse() for k in range(2, 1999, 8)]
    check("P strictly increasing in k (RF=2 grid)",
          all(a < b for a, b in zip(ps, ps[1:])))

    rng = random.Random(12345)

    # (3) sampler vs closed form: eclipse frequency (known stress regime)
    print("(3) sampler vs closed form (eclipse frequency)")
    p = M2Params(N=2000, k=600, RF=1)
    trials = 400
    counts = [g.eclipsed_count() for g in sample_graphs(p, trials, rng)]
    m, v = mean_var(counts)
    se = math.sqrt(v / trials)
    z = (m - p.H * p.p_eclipse()) / se
    check("E[M] matches H*p_eclipse (|z| <= 4)", abs(z) <= 4,
          f"MC {m:.2f} vs closed {p.H * p.p_eclipse():.2f}, z={z:+.2f}")

    # (4) structural: mu=0 => no eclipse
    print("(4) structural check at mu = 0")
    p = M2Params(N=2000, k=0, RF=2)
    ecl = sum(g.eclipsed_count() for g in sample_graphs(p, 50, rng))
    check("no eclipsed node at k=0", ecl == 0)

    print("=" * 70)
    print("self-test:", "PASS" if failures == 0 else f"{failures} FAILURE(S)")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(_selftest())

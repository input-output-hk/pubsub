#!/usr/bin/env python3
"""M5 transmission unreliability: per-message delivery under iid send loss.

Loss model: every honest->honest send is dropped iid with probability p_fail
at send time (sends to adversaries never matter for coverage); r per-link
retries make the per-send failure p_fail^(r+1) -- each attempt is a fresh
Bernoulli.  Under any p_fail > 0 the per-epoch guarantee ("every message of
every publisher") fails almost surely as messages per epoch grow, so the
headline quantities are PER-MESSAGE:

    E[missed]   expected honest nodes that miss one message
    eps_msg     P(the message misses >= 1 honest node)

Guiding identity (leading order): a message crosses an in-edge unless the
pick is adversarial or the send is lost -- per-edge failure
mu_eff = mu + (1-mu) p_fail, the mu-shift curve read at the churn formula
with p = p_fail.  Two accounting corrections separate loss from churn:

  (i)  H does not shrink -- a node behind a lossy link still needs the
       message (under churn a down node leaves the coverage requirement);
  (ii) the muted-publisher out-term loses its factor H -- a message has ONE
       publisher (under churn every honest node is a potential victim).

M5's per-epoch budget is out-dominated at (9, 8), so correction (ii)
roughly doubles its loss tolerance relative to the churn identity.  Exact
single-defect law (P = p_fail^(r+1)):

    q_in  = E[P^D_in] * (1 - (1-P) k_out/(N-1))^(H-1),   D_in  = honest
            count among the node's k_in picks
    q_out = E[P^D_out] * (1 - (1-P) k_in/(N-1))^(H-1),   D_out = honest
            count among the publisher's k_out picks
    eps_msg   = 1 - (1 - q_out)(1 - q_in)^(H-1)
    E[missed] = (H-1)(q_in + q_out - q_in q_out)

(at p_fail = 0 these reduce to the published coverage-law terms).  Backs
../properties/transmission_unreliability.md.

Usage: python3 sweep_m5_pfail.py [--mc] [--trials T] [--seed SEED]

Default run is closed-form only (fast).  --mc (LONG, ~3 min; never run by
CI) adds: (a) the p_fail = 0 anchor -- the loss-injected flood must equal
the plain flood exactly per graph and reproduce the published cost numbers;
(b) the degree-mixture check -- the law is sum_d H pmf(d) P^d, so MC
validates the honest-degree pmfs class by class (the loss part is exact
arithmetic); (c) full loss-injected MC at two elevated cells with
P(miss) ~ 0.1 / 0.4; (d) the retry sweep -- attempted-vs-delivered
bandwidth accounting and depth percentiles at p_fail in {0.01, 0.05, 0.1}
x r in {0, 1, 2}.
"""

from __future__ import annotations

import argparse
import math
import os
import random
import sys
from collections import deque

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from m5_model import M5Params, M5Graph              # noqa: E402
from sweep_m5_cost import flood                     # noqa: E402

N = 20_000
MU = 0.20
K = int(round(MU * N))
H = N - K
K_IN = 9
K_OUT = 8
DELTA = 1e-4
GRID = [0.001, 0.005, 0.01, 0.02, 0.05, 0.10]
BW_CELLS = [0.01, 0.05, 0.10]
RETRIES = (0, 1, 2)
R_EPOCH = (1e3, 1e6)

# published mu = 0.2 operating point (comparison.md section 2)
PUB_MSGS, PUB_HOPS_FULL, PUB_HOPS_MEAN = 217_562, 5.0, 3.9


# ---------------------------------------------------------------------------
# exact per-message single-defect law
# ---------------------------------------------------------------------------

def q_own(m: int, P: float) -> float:
    """E[P^D], D = # honest among m uniform picks (no replacement) from the
    N-1 others (H-1 honest, K adversarial): exact per-message failure of a
    chosen edge set whose honest edges each fail iid with prob P."""
    if m == 0:
        return 1.0
    cn = math.comb(N - 1, m)
    tot = 0.0
    for j in range(max(0, m - K), min(m, H - 1) + 1):
        tot += math.comb(H - 1, j) * math.comb(K, m - j) / cn * P ** j
    return tot


def q_acc(m: int, P: float) -> float:
    """P(no delivery over accepted links): each of the other H-1 honest
    nodes picks the target w.p. m/(N-1) and its send is lost w.p. P."""
    return (1.0 - (1.0 - P) * m / (N - 1)) ** (H - 1)


def q_in(P: float) -> float:
    """Per-message in-cut of a fixed honest node: all k_in own picks fail
    AND every honest out-picker's send is lost."""
    return q_own(K_IN, P) * q_acc(K_OUT, P)


def q_out(P: float) -> float:
    """Per-message out-cut of the publisher: all k_out own picks fail AND
    every honest in-picker's fetch is lost."""
    return q_own(K_OUT, P) * q_acc(K_IN, P)


def eps_msg(p: float, r: int = 0) -> float:
    P = p ** (r + 1)
    qi, qo = q_in(P), q_out(P)
    return 1.0 - (1.0 - qo) * (1.0 - qi) ** (H - 1)


def e_missed(p: float, r: int = 0) -> float:
    P = p ** (r + 1)
    qi, qo = q_in(P), q_out(P)
    return (H - 1) * (qi + qo - qi * qo)


def churn_E(mu_eff: float) -> float:
    """The published mu-shift law (per-epoch defect expectation) at mu_eff:
    the identity's uncorrected reading."""
    return M5Params(N=N, k=int(round(mu_eff * N)),
                    k_in=K_IN, k_out=K_OUT).E_defects()


def bisect_p(f, target: float, lo: float = 0.0, hi: float = 0.999) -> float:
    """Largest p with (increasing) f(p) <= target; nan if f(lo) > target."""
    if f(lo) > target:
        return float("nan")
    if f(hi) <= target:
        return hi
    for _ in range(60):
        mid = (lo + hi) / 2
        if f(mid) > target:
            hi = mid
        else:
            lo = mid
    return lo


def bw_factor(p: float, r: int) -> float:
    """Expected attempts per send with r retries (truncated geometric)."""
    return (1.0 - p ** (r + 1)) / (1.0 - p)


def timeouts_per_delivered(p: float, r: int) -> float:
    """E[attempts - 1 | delivered] -- the latency price on delivered sends."""
    if p == 0.0:
        return 0.0
    s = sum((i + 1) * p ** i * (1 - p) for i in range(r + 1))
    return s / (1.0 - p ** (r + 1)) - 1.0


# ---------------------------------------------------------------------------
# loss-injected flood
# ---------------------------------------------------------------------------

def flood_pfail(adj, seed_depths, H, rng, p_fail: float, retries: int):
    """Cascade from pre-seeded (node, depth) pairs with iid send loss.

    Fire-once on first successful receipt; no resend on the arrival link;
    each send is attempted up to retries+1 times, stopping at the first
    success.  Returns (attempted sends, delivered copies, honest depths).
    At p_fail = 0 this is bit-identical to sweep_m5_cost.flood."""
    depth = [-1] * len(adj)
    parent = [-1] * len(adj)
    dq = deque()
    for v, d in seed_depths:
        if depth[v] < 0:
            depth[v] = d
            dq.append(v)
    attempted = delivered = 0
    rand = rng.random
    while dq:
        v = dq.popleft()
        dv1 = depth[v] + 1
        av = parent[v]
        for w in adj[v]:
            if w == av:                    # don't resend back on the arrival link
                continue
            success = False
            a = 0
            while a <= retries:            # up to retries+1 attempts,
                a += 1                     # stopping at the first success
                if rand() >= p_fail:
                    success = True
                    break
            attempted += a
            if success:
                delivered += 1
                if depth[w] < 0:
                    depth[w] = dv1
                    parent[w] = v
                    dq.append(w)
    ds = [depth[v] for v in range(H) if depth[v] >= 0]
    return attempted, delivered, ds


def m5_message(g: M5Graph, rng: random.Random, p_fail: float, retries: int):
    """One message from publisher 0 through the lossy directed cascade."""
    return flood_pfail(g.adj, [(0, 0)], H, rng, p_fail, retries)


def pmf_chosen(m: int):
    """pmf of the honest count among m uniform picks from the N-1 others
    (exact hypergeometric) -- the degree mixture behind q_own."""
    cn = math.comb(N - 1, m)
    return [math.comb(H - 1, j) * math.comb(K, m - j) / cn
            if 0 <= m - j <= K else 0.0 for j in range(m + 1)]


def pmf_accepted(m: int, hi: int):
    """pmf (0..hi) of the honest accepted count, Binomial(H-1, m/(N-1))
    (exact: each of the H-1 other honest nodes picks the target
    independently) -- the degree mixture behind q_acc."""
    q = m / (N - 1)
    return [math.comb(H - 1, j) * q ** j * (1 - q) ** (H - 1 - j)
            for j in range(hi + 1)]


def pmf_conv(own_m: int, acc_m: int, hi: int):
    """pmf (0..hi) of a direction's total honest degree: own picks +
    accepted picks (independent, so the mixture is the convolution)."""
    a, b = pmf_chosen(own_m), pmf_accepted(acc_m, hi)
    out = [0.0] * (hi + 1)
    for i, x in enumerate(a):
        if not x:
            continue
        for j, y in enumerate(b):
            if i + j <= hi:
                out[i + j] += x * y
    return out


# ---------------------------------------------------------------------------
# MC sections (--mc)
# ---------------------------------------------------------------------------

def mc_anchor(rng: random.Random, trials: int) -> None:
    """(a) p_fail = 0 anchor: the loss-injected flood must equal the plain
    flood exactly per graph, and reproduce the published cost numbers."""
    p = M5Params(N=N, k=K, k_in=K_IN, k_out=K_OUT)
    mism = 0
    sends = maxd = meand = 0.0
    for _ in range(trials):
        g = M5Graph(p, rng)
        ref_sends, ref_max, ref_mean, ref_r = flood(g.adj, 0)
        at, de, ds = m5_message(g, rng, 0.0, 0)
        if not (at == de == ref_sends and max(ds) == ref_max
                and math.isclose(sum(ds) / len(ds), ref_mean, rel_tol=1e-12)
                and len(ds) == ref_r):
            mism += 1
        sends += at
        maxd += max(ds)
        meand += sum(ds) / len(ds)
    sends /= trials
    maxd /= trials
    meand /= trials
    print(f"  exact equality vs plain flood: "
          f"{'ok' if mism == 0 else 'FAIL'} ({mism}/{trials} mismatches)")
    print(f"  published anchor: msgs {sends:,.0f} vs {PUB_MSGS:,} "
          f"({100 * (sends / PUB_MSGS - 1):+.2f} %), hops full "
          f"{maxd:.2f} vs {PUB_HOPS_FULL}, mean {meand:.2f} vs "
          f"{PUB_HOPS_MEAN}")


def _report_mix(label: str, pmf, samples, graphs: int, d_hi: int) -> None:
    """Per degree class d: expected count per graph H*pmf(d) vs the measured
    mean, z where the class is measurable (>= 25 expected over the run)."""
    print(f"  {label}: {'d':>3} {'law/graph':>10} {'MC/graph':>10} {'z':>6}")
    for d in range(min(d_hi, len(pmf) - 1) + 1):
        law = H * pmf[d]
        cs = [s.get(d, 0) for s in samples]
        m = sum(cs) / graphs
        var = sum((x - m) ** 2 for x in cs) / (graphs - 1)
        se = math.sqrt(var / graphs)
        if law * graphs >= 25 and se > 0:
            z = f"{(m - law) / se:>+6.2f}"
        else:
            z = "     -"
        print(f"  {'':>{len(label)}}  {d:>3} {law:>10.4g} {m:>10.4g} {z}")


def mc_degree_mix(rng: random.Random, graphs: int) -> None:
    """(b) degree-mixture law check.  The single-defect law is the mixture
    E[missed] = sum_d H pmf(d) P^d over honest-degree classes: the loss part
    P^d is exact arithmetic, so what MC must validate is pmf class by class.
    Classes too rare to measure (law/graph ~ 0) are exact combinatorics --
    the same terms the p_fail = 0 coverage law stands on."""
    p = M5Params(N=N, k=K, k_in=K_IN, k_out=K_OUT)
    ins, outs = [], []
    for _ in range(graphs):
        g = M5Graph(p, rng)
        indeg = [0] * H
        for v in range(H):
            for w in g.adj[v]:
                indeg[w] += 1
        h: dict = {}
        for v in range(H):
            h[indeg[v]] = h.get(indeg[v], 0) + 1
        ins.append(h)
        h = {}
        for v in range(H):
            d = len(g.adj[v])
            h[d] = h.get(d, 0) + 1
        outs.append(h)
    _report_mix("honest in-degree (q_in, hypergeom * binomial)",
                pmf_conv(K_IN, K_OUT, 40), ins, graphs, 12)
    _report_mix("honest out-degree (q_out, hypergeom * binomial)",
                pmf_conv(K_OUT, K_IN, 40), outs, graphs, 12)


def mc_elevated(rng: random.Random) -> None:
    """(c) full loss-injected MC vs the exact law at elevated cells."""
    p = M5Params(N=N, k=K, k_in=K_IN, k_out=K_OUT)
    print(f"  {'p_fail':>7} {'eps law':>9} {'eps MC':>9} {'bad/trials':>11} "
          f"{'z':>6} {'E law':>9} {'E MC':>9}")
    for target, T in ((0.10, 400), (0.40, 250)):
        pc = round(bisect_p(eps_msg, target), 3)
        pred = eps_msg(pc)
        bad = 0
        missed_tot = 0
        for _ in range(T):
            g = M5Graph(p, rng)
            _at, _de, ds = m5_message(g, rng, pc, 0)
            miss = H - len(ds)
            missed_tot += miss
            bad += miss > 0
        mc = bad / T
        se = math.sqrt(max(mc, 1 / T) * (1 - mc) / T)
        z = (mc - pred) / se
        print(f"  {pc:>7.3f} {pred:>9.4f} {mc:>9.4f} {bad:>5}/{T:<5} "
              f"{z:>+6.2f} {e_missed(pc):>9.4f} {missed_tot / T:>9.4f}")


def mc_retries(rng: random.Random, trials: int) -> None:
    """(d) retry sweep: attempted-vs-delivered accounting (the closed form
    x(1-p^(r+1))/(1-p)) and depth percentiles under loss."""
    p = M5Params(N=N, k=K, k_in=K_IN, k_out=K_OUT)
    base = H * (K_IN + K_OUT) * (H - 1) / (N - 1)
    print(f"  {'p_fail':>7} {'r':>2} {'attempted':>10} {'closed':>10} "
          f"{'diff%':>7} {'delivered':>10} {'del/att':>8} {'miss/msg':>9} "
          f"{'hops mean':>9} {'p99':>5} {'max':>5}")
    for pf in BW_CELLS:
        for r in RETRIES:
            at = de = miss = 0.0
            depths = []
            for _ in range(trials):
                g = M5Graph(p, rng)
                a, d, ds = m5_message(g, rng, pf, r)
                at += a
                de += d
                miss += H - len(ds)
                depths.extend(ds)
            at /= trials
            de /= trials
            closed = base * bw_factor(pf, r)
            depths.sort()
            p99 = depths[int(0.99 * (len(depths) - 1))]
            print(f"  {pf:>7.3f} {r:>2} {at:>10,.0f} {closed:>10,.0f} "
                  f"{100 * (at / closed - 1):>+7.2f} {de:>10,.0f} "
                  f"{de / at:>8.4f} {miss / trials:>9.3f} "
                  f"{sum(depths) / len(depths):>9.2f} {p99:>5} "
                  f"{max(depths):>5}")


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------

def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--mc", action="store_true",
                    help="loss-injected MC: anchor, degree mixture, elevated "
                         "cells, retry sweep (LONG, ~3 min; never run by CI)")
    ap.add_argument("--trials", type=int, default=40,
                    help="graphs for the anchor / retry cells (default 40/20)")
    ap.add_argument("--seed", type=int, default=20260813)
    args = ap.parse_args()

    print(f"M5 transmission unreliability -- (k_in, k_out) = "
          f"({K_IN}, {K_OUT}), N = {N}, mu = {MU}, delta = {DELTA:g}")

    # consistency anchor: the p_fail = 0 law equals the published closed form
    pp = M5Params(N=N, k=K, k_in=K_IN, k_out=K_OUT)
    assert (math.isclose(q_in(0.0), pp.p_in_isolated(), rel_tol=1e-9)
            and math.isclose(q_out(0.0), pp.p_out_isolated(), rel_tol=1e-9)), \
        "p_fail = 0 law anchor broken"

    print("\n(1) per-message law across the p_fail grid (r = 0)")
    print(f"  {'p_fail':>7} {'mu_eff':>7} {'E churn-id':>11} "
          f"{'E[missed]':>10} {'eps_msg':>10}")
    for pf in [0.0] + GRID:
        mu_eff = MU + (1 - MU) * pf
        print(f"  {pf:>7.3f} {mu_eff:>7.4f} {churn_E(mu_eff):>11.3e} "
              f"{e_missed(pf):>10.3e} {eps_msg(pf):>10.3e}")

    p_id = bisect_p(lambda q: 1 - math.exp(-churn_E(MU + (1 - MU) * q)),
                    DELTA)
    print(f"\n  churn-identity budget (mu-shift curve, per-epoch "
          f"accounting): p_fail <= {p_id:.4f}")
    for r in RETRIES:
        pm = bisect_p(lambda q: eps_msg(q, r), DELTA)
        print(f"  per-message budget (eps_msg <= delta), r = {r}: "
              f"p_fail <= {pm:.4f}")

    print("\n(2) retry economics -- smallest r meeting eps_msg <= delta")
    print(f"  {'p_fail':>7} {'eps(r=0)':>10} {'r*':>3} {'eps(r*)':>10} "
          f"{'bandwidth':>9} {'timeouts/del':>12}")
    for pf in GRID:
        rstar = next((r for r in RETRIES if eps_msg(pf, r) <= DELTA), None)
        if rstar is None:
            print(f"  {pf:>7.3f} {eps_msg(pf):>10.3e}   -          -  "
                  f"(no r <= {RETRIES[-1]} meets delta)")
            continue
        print(f"  {pf:>7.3f} {eps_msg(pf):>10.3e} {rstar:>3} "
              f"{eps_msg(pf, rstar):>10.3e} x{bw_factor(pf, rstar):>7.4f} "
              f"{timeouts_per_delivered(pf, rstar):>12.4f}")

    # Per-epoch reading.  eps_msg has a STRUCTURAL floor at p_fail = 0
    # (graph randomness, drawn once per epoch and shared by every message),
    # so the naive union bound eps_msg <= delta/R is unreachable for any
    # R >= 2.  The correct split is graph-once + loss-fresh:
    #     P(bad epoch) <= P(structural defect) + R * eps_loss(p, r)
    # with eps_loss = eps_msg(p, r) - eps_msg(0), the loss-added mass whose
    # randomness is fresh per message.  The loss budget is delta's headroom
    # over the published structural P(bad), divided by R.
    struct = eps_msg(0.0)
    p_bad0 = 1 - math.exp(-H * (q_in(0.0) + q_out(0.0)))
    headroom = DELTA - p_bad0
    print(f"\n(3) per-epoch reading -- structural floor eps_msg(0) = "
          f"{struct:.3e}, P(bad graph) = {p_bad0:.3e}, loss headroom "
          f"delta - P(bad) = {headroom:.3e}")
    print("    largest p_fail with R * [eps_msg(p) - eps_msg(0)] <= headroom")
    print(f"  {'R':>10} " + " ".join(f"{'r=' + str(r):>10}" for r in RETRIES))
    for Rr in R_EPOCH:
        row = " ".join(
            f"{bisect_p(lambda q: eps_msg(q, r) - struct, headroom / Rr):>10.2e}"
            for r in RETRIES)
        print(f"  {Rr:>10.0e} {row}")

    if not args.mc:
        return
    rng = random.Random(args.seed)
    print(f"\n(4) MC -- seed {args.seed}")
    print(f"(4a) p_fail = 0 anchor ({args.trials} graphs)")
    mc_anchor(rng, args.trials)
    print("(4b) degree-mixture law check (100 graphs)")
    mc_degree_mix(rng, 100)
    print("(4c) loss-injected MC at elevated cells")
    mc_elevated(rng)
    print(f"(4d) retry sweep ({max(args.trials // 2, 10)} graphs/cell)")
    mc_retries(rng, max(args.trials // 2, 10))


if __name__ == "__main__":
    main()

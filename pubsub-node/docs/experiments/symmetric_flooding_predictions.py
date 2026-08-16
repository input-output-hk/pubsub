#!/usr/bin/env python3
"""Exact-arithmetic prediction ledger for the symmetric flooding pass.

Population: N = 4000, K = 16 honest picks over the symmetric pair gate
at width B. The adversarial class is S Sybil flooders (silent relays,
symmetric handshake, same pinned B, no pick count — every admissible
pair dialed, uncapped acceptance), so S doubles as the ambient
adversarial count: mu = S/N. Honest acceptance is the ADR 0042
admissions budget C — fresh peer arrivals spend it, crossings are
exempt, the node's own picks never count.

Per honest node, exact binomial arithmetic (E18 conventions —
`gated_symmetric_predictions.py`):

  pool: honest part h ~ Bin(H-1, 1/B), Sybil part a ~ Bin(S, 1/B)
  m  = E[min(K,p)/p], p = 1 + Bin(N-2, 1/B)  -- prob a pool member picks me
  given (h, a): mm = min(K, h+a)/(h+a)       -- prob I pick a given member

  routes of my realised edges (uncapped):
    mutual honest   = sum h*m*mm           own-only honest = sum h*mm*(1-m)
    admitted honest = sum h*m*(1-mm)       (fresh -- spends my budget)
    mutual Sybil    = sum a*mm             (every Sybil pick of mine crosses)
    admitted Sybil  = sum a*(1-mm)         (fresh)

  budget race (the E12 fair-arrival contention model, enumerated
  exactly; the pilot-calibrated form): the pick split is WITHOUT
  replacement -- given (h, a) the node draws exactly kp = min(K, h+a)
  distinct pool members, so its Sybil picks are hypergeometric,
  picked_s ~ Hyper(h+a, a, kp), and the fresh Sybil load is
  deterministic given the split: f_s = a - picked_s (every unpicked
  admissible Sybil dials). Fresh honest arrivals f_h ~
  Bin(h - picked_h, m) (unpicked honest members pick me independently).
  The budget admits min(C, f_h + f_s); refusals hit each class in
  proportion to its arriving load (verified 48/48 in directional E12).
  A binomial per-member pick approximation misses at second order --
  the deliverable contrast-pair and pilot config comments carry that
  earlier registration; the miss and this calibration are documented in
  the report. Refused honest fresh arrivals are whole lost edges,
  charged at both ends: the dialer-side echo (my own-only dials to
  honest peers face the same race at the far end) is applied as an
  independent-victim first-order correction to realised degree.

Scheme-A contrast cells (the pre-ADR both-role scan, run at the
pre-change tool commit) use the stated uniform-interleaving model: at a
victim, admission events (ALL arrivals -- fresh and crossings, the scan
knows no exemption) and mirror events (my accepted own dials, never
refused) interleave uniformly; every event holds a slot until the scan
reaches C_A, after which every admission is refused. Hence accepted
admissions = A * min(1, C_A/(A + M)) (hypergeometric first-C_A draw),
refusals split by arrival composition -- the crossing share of refusals
is the measured veto channel. First-order: M uses the uncapped own-pick
count, and post-refusal mirror rescue (a refused crossing whose own dial
the peer accepted) is ignored for honest peers, counted for Sybils
(they accept everything).

Isolation: the E18 two-channel law composes at mu = S/N unchanged by
the cap (empty-pool and all-picks-adversarial are pre-acceptance
geometry); starvation adds a third channel only where refusals bite --
the ledger prints the E18 base and the starvation means separately, and
the pilot calibrates their composition before the grid relies on it.
"""

import math
import sys

N = 4000
K = 16


def lchoose(n, k):
    if k < 0 or k > n:
        return -math.inf
    return math.lgamma(n + 1) - math.lgamma(k + 1) - math.lgamma(n - k + 1)


def binom_pmf(n, p, k):
    if k < 0 or k > n:
        return 0.0
    if p == 0.0:
        return 1.0 if k == 0 else 0.0
    lp = lchoose(n, k) + k * math.log(p) + (n - k) * math.log1p(-p)
    return math.exp(lp)


def pmf_range(n, p, tol=1e-15):
    """(k, pmf) pairs covering all but < tol of the mass."""
    if n == 0 or p == 0.0:
        return [(0, 1.0)]
    mean = n * p
    sd = math.sqrt(n * p * (1 - p))
    lo = max(0, int(mean - 10 * sd) - 2)
    hi = min(n, int(mean + 10 * sd) + 2)
    return [(k, q) for k in range(lo, hi + 1) if (q := binom_pmf(n, p, k)) > tol]


def member_pick_prob(B):
    """m = E[min(K,p)/p], p = 1 + Bin(N-2, 1/B)."""
    return sum(q * min(K, k + 1) / (k + 1) for k, q in pmf_range(N - 2, 1.0 / B))


def hyper_pmf(pop, successes, draws):
    """(k, pmf) pairs of Hypergeometric(pop, successes, draws)."""
    out = []
    for k in range(max(0, draws - (pop - successes)), min(successes, draws) + 1):
        q = math.exp(
            lchoose(successes, k) + lchoose(pop - successes, draws - k) - lchoose(pop, draws)
        )
        if q > 1e-12:
            out.append((k, q))
    return out


def cell(B, S, cap, scheme="budget"):
    """One cell's per-honest-node expectations. Returns a dict."""
    H = N - S
    m = member_pick_prob(B)
    # Joint pool enumeration: route expectations, and (capped cells) the
    # without-replacement fair race accumulated inside the same loop.
    own_h = mut_h = adm_h = mut_s = adm_s = 0.0
    race_adm_h = race_adm_s = race_ref_h = race_ref_s = race_p_ref_h = 0.0
    for h, qh in pmf_range(H - 1, 1.0 / B):
        for a, qa in pmf_range(S, 1.0 / B):
            q = qh * qa
            p = h + a
            mm = min(K, p) / p if p else 0.0
            mut_h += q * h * m * mm
            own_h += q * h * mm * (1 - m)
            adm_h += q * h * m * (1 - mm)
            mut_s += q * a * mm
            adm_s += q * a * (1 - mm)
            if cap is None or scheme != "budget" or p == 0:
                continue
            kp = min(K, p)
            for picked_s, q3 in hyper_pmf(p, a, kp):
                fs = a - picked_s  # every unpicked admissible Sybil dials
                picked_h = kp - picked_s
                for fh, q4 in pmf_range(h - picked_h, m):
                    w = q * q3 * q4
                    total = fh + fs
                    if total <= cap:
                        race_adm_h += w * fh
                        race_adm_s += w * fs
                        continue
                    share = cap / total
                    race_adm_h += w * fh * share
                    race_adm_s += w * fs * share
                    race_ref_h += w * fh * (1 - share)
                    race_ref_s += w * fs * (1 - share)
                    if fh > 0:
                        race_p_ref_h += w
    routes = {
        "own_only_h": own_h,
        "mutual_h": mut_h,
        "admitted_h": adm_h,
        "mutual_s": mut_s,
        "admitted_s": adm_s,
    }
    out = {"B": B, "S": S, "cap": cap, "m": m, **routes}
    out["d_uncapped"] = own_h + mut_h + adm_h + mut_s + adm_s
    out["sybil_uncapped"] = mut_s + adm_s  # the exact S/B in expectation
    if cap is None:
        return out

    if scheme == "budget":
        a_h, a_s, r_h, r_s, p_r = (
            race_adm_h,
            race_adm_s,
            race_ref_h,
            race_ref_s,
            race_p_ref_h,
        )
        # Dialer-side echo: my own-only dials to honest peers face the
        # same race at the far end (independent-victim first order).
        surv = a_h / adm_h if adm_h > 0 else 1.0
        out.update(
            admitted_h_capped=a_h,
            admitted_s_capped=a_s,
            refused_h=r_h,
            refused_s=r_s,
            p_any_honest_refusal=p_r,
            own_only_h_capped=own_h * surv,
            d_capped=own_h * surv + mut_h + mut_s + a_h + a_s,
            sybil_capped=mut_s + a_s,
        )
    else:  # scheme A: the both-role scan, uniform-interleaving model
        arrivals_h = mut_h + adm_h  # crossings are admissions too
        arrivals_s = mut_s + adm_s
        arrivals = arrivals_h + arrivals_s
        mirrors = own_h + mut_h + mut_s  # uncapped own picks
        events = arrivals + mirrors
        accept_share = min(1.0, cap / events) if events > 0 else 1.0
        refused = arrivals * (1 - accept_share)
        crossings = mut_h + mut_s
        out.update(
            admitted_all=arrivals * accept_share,
            refused_all=refused,
            refused_crossing=refused * (crossings / arrivals) if arrivals else 0.0,
            refused_crossing_h=refused * (mut_h / arrivals) if arrivals else 0.0,
            # Sybil edges survive a refused crossing (the Sybil accepted
            # my dial); honest mutual edges refused on BOTH sides die --
            # single-sided survival counted at first order.
            sybil_capped=mut_s + adm_s * accept_share,
        )
    return out


def cell_ordered(B, S, cap):
    """The ordered comparison predicate's cell (ADR 0043): P(X dials Y) =
    m/B for a picking node (Y in X's out-pool times picked), 1/B for the
    flooder (dials its whole out-pool). Directions are independent draws,
    so the crossing discount is (m/B) — near-vacuous — and, unlike the
    unordered pair, own-only Sybil edges exist (a Sybil dials back only
    if the victim sits in ITS pool). The race uses independent per-class
    binomial loads: the unordered pass's pool coupling acts here only
    through the tiny crossing discount."""
    H = N - S
    m = member_pick_prob(B)
    q = m / B  # a picking node dials a given peer
    routes = {
        "mutual_h": (H - 1) * q * q,
        "own_only_h": (H - 1) * q * (1 - q),
        "admitted_h": (H - 1) * q * (1 - q),
        "mutual_s": S * q * (1.0 / B),
        "own_only_s": S * q * (1 - 1.0 / B),
        "admitted_s": S * (1.0 / B) * (1 - q),
    }
    out = {"B": B, "S": S, "cap": cap, "m": m, **routes}
    out["d_uncapped"] = sum(routes.values())
    out["sybil_uncapped"] = routes["mutual_s"] + routes["own_only_s"] + routes["admitted_s"]
    if cap is not None:
        adm_h = adm_s = ref_h = ref_s = p_ref = 0.0
        for fh, qh in pmf_range(H - 1, q * (1 - q)):
            for fs, qs in pmf_range(S, (1.0 / B) * (1 - q)):
                w = qh * qs
                t = fh + fs
                if t <= cap:
                    adm_h += w * fh
                    adm_s += w * fs
                    continue
                share = cap / t
                adm_h += w * fh * share
                adm_s += w * fs * share
                ref_h += w * fh * (1 - share)
                ref_s += w * fs * (1 - share)
                if fh > 0:
                    p_ref += w
        surv = adm_h / routes["admitted_h"] if routes["admitted_h"] > 0 else 1.0
        out.update(
            admitted_h_capped=adm_h,
            admitted_s_capped=adm_s,
            refused_h=ref_h,
            refused_s=ref_s,
            p_any_honest_refusal=p_ref,
            own_only_h_capped=routes["own_only_h"] * surv,
            sybil_capped=routes["mutual_s"] + routes["own_only_s"] + adm_s,
        )
    return out


def ordered_isolation(B, S):
    """The ordered construction's honest-isolation constant at gate width
    B (open acceptance, K picks): inbound and outbound honest edges ride
    INDEPENDENT coin sets — P(no inbound) = (1 - m/B)^(H-1); outbound uses
    the E18 avoid term over the out-pool (empty, or all min(K, pool)
    picks landing on adversarial members). Past the out-pool saturation
    boundary B > (N-1)/K the picks cover the whole pool and both
    exponents become pool-driven — the regime where the ordered tail
    equals the unordered pair's at equal total density."""
    H = N - S
    m = member_pick_prob(B)
    no_inbound = (1 - m / B) ** (H - 1)
    no_outbound = 0.0
    for h, qh in pmf_range(H - 1, 1.0 / B):
        for a, qa in pmf_range(S, 1.0 / B):
            q = qh * qa
            if h == 0:
                no_outbound += q
                continue
            if h + a <= K or a < K:
                continue
            no_outbound += q * math.exp(lchoose(a, K) - lchoose(h + a, K))
    return H * no_inbound * no_outbound


def e18_isolation(B, S):
    """The E18 two-channel isolation constant at mu = S/N (gated picks)."""
    H = N - S
    m = member_pick_prob(B)
    tot = 0.0
    for h, qh in pmf_range(H - 1, 1.0 / B):
        f = qh * (1.0 - m) ** h
        if f < 1e-24 and h > (H - 1) / B:
            break
        if h == 0:
            tot += f
            continue
        inner = 0.0
        for a, qa in pmf_range(S, 1.0 / B):
            if h + a <= K or a < K:
                continue
            inner += qa * math.exp(lchoose(a, K) - lchoose(h + a, K))
        tot += f * inner
    return H * tot


def show(c, extra=""):
    base = (
        f"B={c['B']:>3} S={c['S']:>4} cap={str(c['cap']):>4}  m={c['m']:.4f}"
        f"  d0={c['d_uncapped']:6.2f}"
        f"  routes h(own/mut/adm)={c['own_only_h']:5.2f}/{c['mutual_h']:5.2f}/{c['admitted_h']:5.2f}"
        f"  s(mut/adm)={c['mutual_s']:5.2f}/{c['admitted_s']:5.2f}"
    )
    print(base + extra)


if __name__ == "__main__":
    args = sys.argv[1:]
    if len(args) >= 3:
        B, S = int(args[0]), int(args[1])
        cap = None if args[2] == "open" else int(args[2])
        scheme = args[3] if len(args) > 3 else "budget"
        if scheme == "ordered":
            c = cell_ordered(B, S, cap)
            for k, v in c.items():
                print(f"{k:>24}: {round(v, 4) if isinstance(v, float) else v}")
            e = ordered_isolation(B, S)
            print(f"{'ordered E_iso':>24}: {e:.3e}   P(bad)~{1 - math.exp(-e):.4g}")
            sys.exit(0)
        c = cell(B, S, cap, scheme)
        for k, v in c.items():
            print(f"{k:>24}: {v if isinstance(v, int) else round(v, 4)}")
        e = e18_isolation(B, S)
        print(f"{'E18 E_iso (mu=S/N)':>24}: {e:.3e}   P(bad)~{1 - math.exp(-e):.4g}")
        sys.exit(0)

    # The grid-shaping table: uncapped routes and loads per (B, S).
    for B in (50, 125, 250):
        for S in (40, 400, 800):
            show(cell(B, S, None))
        print()

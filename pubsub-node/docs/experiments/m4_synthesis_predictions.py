#!/usr/bin/env python3
"""(N, K)-parameterised prediction ledger for the M4 synthesis pass.

Generalises the committed E18/E19 ledgers (gated_symmetric_predictions.py,
symmetric_flooding_predictions.py — both pinned at N = 4000, K = 16) so
the same validated forms speak at the CIP operating shape (N = 20 000,
K = 9, mu = 0.2 — configs/experiments/comparisons/m4-n20k-rf9.toml) and
at the gated candidates around it. Reproduction record: at
--n=4000 --k=16 this ledger reproduces the committed E19 ledger exactly
(the budget cell 50/800/16 to the fourth decimal; the capsweep rows at
250/1600 including both measured anchors, C = 3 -> P(bad) 0.1965 and
C = 12 -> 0.1482); an 8-run live probe at N = 20 000, K = 9, B = 500,
C = 23 matched d = 20.77 measured vs 20.78 predicted with max degree
exactly K + C. Forms carried over verbatim, only N/K lifted to
parameters:

  pool          lambda = (N-1)/B; honest part h ~ Bin(H-1, 1/B),
                Sybil part a ~ Bin(S, 1/B)
  pick prob     m = E[min(K, p)/p], p = 1 + Bin(N-2, 1/B)
  degree        d = routes summed (the E18 d = lambda*m*(2-m) shape)
  E18 isolation H * E[(1-m)^h * P(all min(K,pool) picks adversarial)]
                (two channels; empty-pool + all-picks-adversarial)
  budget race   without-replacement pick split (hypergeometric) + fresh
                honest Bin(h - picked_h, m); refusals proportional
  composition   cap_composition: the E19 section-6 first-order form
  design rules  E18 pool floor (N-1)/B >= ln(H/delta)/(1-mu)
                E10/E18 window edge r = (N-1)/(B*K) >~ 3 (gated-symmetric)
                E19 cap anchor: fresh honest load K(1-m)(1-mu) + c*sqrt(load)

Usage:
  m4_synthesis_predictions.py table              # B ladder at the CIP shape
  m4_synthesis_predictions.py cell B [cap]       # one cell's routes + race
  m4_synthesis_predictions.py capsweep B [C...]  # cap trade-off curve at B
  (N, K, S overridable via flags --n --k --s; defaults 20000, 9, 4000)

The cell/capsweep race models FLOODER adversaries (every admissible pair
dialed — the E19 convention). For a homogeneous picking population (the
E18-style coverage cells, adversaries = silent relays WITH picks) run
cell with --s=0: every pool member picks at rate m, so the routes and
race are the class-blind geometry, the linked-peer class splits
binomially at mu, and coverage still reads e18_isolation at the real S.
"""
import math
import sys

DEFAULT_N, DEFAULT_K, DEFAULT_S = 20000, 9, 4000


def lchoose(n, k):
    if k < 0 or k > n:
        return -math.inf
    return math.lgamma(n + 1) - math.lgamma(k + 1) - math.lgamma(n - k + 1)


def binom_pmf(n, p, k):
    if k < 0 or k > n:
        return 0.0
    if p == 0.0:
        return 1.0 if k == 0 else 0.0
    if p >= 1.0:
        return 1.0 if k == n else 0.0
    return math.exp(
        lchoose(n, k) + k * math.log(p) + (n - k) * math.log(1 - p)
    )


def pmf_range(n, p, tol=1e-15):
    if n == 0 or p == 0.0:
        return [(0, 1.0)]
    if p >= 1.0:  # the ungated limit, B = 1: the pool is everyone
        return [(n, 1.0)]
    mean = n * p
    sd = math.sqrt(n * p * (1 - p))
    lo = max(0, int(mean - 10 * sd) - 2)
    hi = min(n, int(mean + 10 * sd) + 2)
    return [(k, q) for k in range(lo, hi + 1) if (q := binom_pmf(n, p, k)) > tol]


def hyper_pmf(pop, successes, draws):
    out = []
    for k in range(max(0, draws - (pop - successes)), min(successes, draws) + 1):
        q = math.exp(
            lchoose(successes, k) + lchoose(pop - successes, draws - k) - lchoose(pop, draws)
        )
        if q > 1e-12:
            out.append((k, q))
    return out


def member_pick_prob(N, K, B):
    return sum(q * min(K, k + 1) / (k + 1) for k, q in pmf_range(N - 2, 1.0 / B))


def cell(N, K, B, S, cap=None):
    """The E19 joint-pool cell, (N, K)-parameterised. Same accumulators."""
    H = N - S
    m = member_pick_prob(N, K, B)
    own_h = mut_h = adm_h = mut_s = adm_s = 0.0
    r_adm_h = r_adm_s = r_ref_h = r_ref_s = r_p_ref = 0.0
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
            if cap is None or p == 0:
                continue
            kp = min(K, p)
            for picked_s, q3 in hyper_pmf(p, a, kp):
                fs = a - picked_s
                picked_h = kp - picked_s
                for fh, q4 in pmf_range(h - picked_h, m):
                    w = q * q3 * q4
                    total = fh + fs
                    if total <= cap:
                        r_adm_h += w * fh
                        r_adm_s += w * fs
                        continue
                    share = cap / total
                    r_adm_h += w * fh * share
                    r_adm_s += w * fs * share
                    r_ref_h += w * fh * (1 - share)
                    r_ref_s += w * fs * (1 - share)
                    if fh > 0:
                        r_p_ref += w
    out = {
        "B": B, "S": S, "cap": cap, "m": m,
        "own_only_h": own_h, "mutual_h": mut_h, "admitted_h": adm_h,
        "mutual_s": mut_s, "admitted_s": adm_s,
        "d_uncapped": own_h + mut_h + adm_h + mut_s + adm_s,
        "sybil_uncapped": mut_s + adm_s,
    }
    if cap is not None:
        surv = r_adm_h / adm_h if adm_h > 0 else 1.0
        out.update(
            admitted_h_capped=r_adm_h, admitted_s_capped=r_adm_s,
            refused_h=r_ref_h, refused_s=r_ref_s, p_any_honest_refusal=r_p_ref,
            own_only_h_capped=own_h * surv,
            d_capped=own_h * surv + mut_h + mut_s + r_adm_h + r_adm_s,
            sybil_capped=mut_s + r_adm_s,
        )
    return out


def e18_isolation(N, K, B, S):
    H = N - S
    m = member_pick_prob(N, K, B)
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


def all_picks_adversarial(N, k_picks, B, S):
    """Per-node probability that ALL of its min(k_picks, pool) gated
    picks land on adversarial pool members — including the empty-pool
    case (no picks at all). The directional 'deaf' coin, and equally the
    M3 seed-failure coin with (s-1, B_publisher). B = 1 is the ungated
    limit (the pool is the whole population)."""
    out = 0.0
    H = N - S
    for h, qh in pmf_range(H - 1, 1.0 / B):
        for a, qa in pmf_range(S, 1.0 / B):
            p = h + a
            if p == 0:
                out += qh * qa
                continue
            kp = min(k_picks, p)
            out += qh * qa * math.exp(lchoose(a, kp) - lchoose(p, kp))
    return out


def directional_isolation(N, K, B, S, gate_only=False):
    """The directional (M2 / M3-relay-seam) gated isolation channels,
    per population: E_deaf (no honest relay upstream — my picks/pool
    betrayed me; my own-dial arm) and E_mute (no honest node picked me —
    the inbound arm; unrescued, i.e. M2's mute-publisher channel).
    The two directions ride independent coins (each directed pair its
    own draw), so the channels multiply H separately and P(bad) ~
    1 - exp(-(E_deaf + E_mute)) at first order. gate_only = dial every
    survivor (no pick count): both channels collapse to the empty-pool
    coin e^(-(1-mu)lambda) — E10's measured gate-only doubling.

    Validation record (E10, measured at N = 4000, K = 16, S = 800):
    ungated -> 0.0088 (the M2 law); gated picks at r = 2 (B = 125) ->
    law-exact (measured pooled 0.00872); gate-only at B = 235 -> 0.0078
    vs E10's prediction 0.0079 and measured 0.0085; gate-only at
    B = 250 -> 0.0177 vs measured 0.0193 [0.0154, 0.0240] (the 2x
    doubling). M2 ungated op point N = 20000, K = 24 -> 7.4e-5 (the
    published <= 1e-4 row)."""
    H = N - S
    m_d = 1.0 if gate_only else member_pick_prob(N, K, B)
    e_mute = H * (1 - m_d / B) ** (H - 1)
    if gate_only:
        e_deaf = H * sum(
            qh for h, qh in pmf_range(H - 1, 1.0 / B) if h == 0
        )
    else:
        e_deaf = H * all_picks_adversarial(N, K, B, S)
    return e_deaf, e_mute


def m3_isolation(N, K, B_r, S, s, B_p):
    """M3's gated isolation: the relay-seam deaf channel unrescued
    (publisher links carry only the seeder's OWN publications, so a
    deaf node still fails the every-publisher check for every publisher
    that did not seed it directly), plus the mute channel rescued by
    the node's own s-1 seeds (own publications ride relay downstream
    UNION seeds — a mute publisher is stranded only if all its seeds
    are adversarial too, the cross-seam product of the alignment
    record). Ungated seams = width 1.

    Validation: ungated at the published M3 op point (N = 20000,
    K = 12, s = 8) -> ~8e-5 against the <= 1e-4 row."""
    e_deaf, e_mute = directional_isolation(N, K, B_r, S)
    seed_fail = all_picks_adversarial(N, s - 1, B_p, S)
    return e_deaf, e_mute * seed_fail


def cap_composition(N, K, B, S, rho):
    """The E19 section-6 first-order increment, (N, K)-parameterised."""
    H = N - S
    m = member_pick_prob(N, K, B)
    delta = 0.0
    for h, qh in pmf_range(H - 1, 1.0 / B):
        if h == 0:
            continue
        for a, qa in pmf_range(S, 1.0 / B):
            p = h + a
            mm = min(K, p) / p
            arms = mm * (1 - m) + m * (1 - mm)
            dead_capped = 1 - (m * mm + arms * (1 - rho))
            dead_open = 1 - (m * mm + arms)
            delta += qh * qa * (dead_capped**h - dead_open**h)
    return H * delta


def design_table(N, K, S, bs):
    """The B ladder: every design rule's column at each candidate width."""
    H = N - S
    mu = S / N
    b_pool_floor = (N - 1) / (math.log(H / 1e-4) / (1 - mu))
    print(f"N={N} K={K} S={S} (mu={mu:.2f})  pool floor B<= {b_pool_floor:.0f} "
          f"(delta=1e-4)   window edge r>=3: B<= {(N - 1) / (3 * K):.0f}")
    print(f"{'B':>6} {'r':>6} {'lambda':>7} {'m':>7} {'d':>7} "
          f"{'E_iso':>10} {'P(bad)':>9} {'fresh_h':>8} {'fresh_s':>8} "
          f"{'C(c=2)':>7} {'C(c=3)':>7}")
    for B in bs:
        lam = (N - 1) / B
        r = lam / K
        c = cell(N, K, B, S)
        e = e18_isolation(N, K, B, S)
        fresh_h = c["admitted_h"]      # honest fresh-arrival load per victim
        fresh_s = c["admitted_s"]
        c2 = fresh_h + fresh_s + 2 * math.sqrt(fresh_h + fresh_s)
        c3 = fresh_h + fresh_s + 3 * math.sqrt(fresh_h + fresh_s)
        print(f"{B:>6} {r:>6.2f} {lam:>7.1f} {c['m']:>7.4f} "
              f"{c['d_uncapped']:>7.2f} {e:>10.3e} {1 - math.exp(-e):>9.3g} "
              f"{fresh_h:>8.3f} {fresh_s:>8.3f} {math.ceil(c2):>7} {math.ceil(c3):>7}")


def compare(N, S):
    """The gated model comparison at the CIP shape — the ungated
    model-comparison graph's reliability dimension redone with gated
    forms, under the equal-attack-surface normalization. Surface =
    victims a deposit-priced identity can touch on the RELAY seam:
    the pair draw's one coin covers both directions (surface =
    lambda = (N-1)/B), the directional draw's two independent coins
    pay twice (surface = 2(N-1)/B_r) — measured via the E19 ordered
    arm. The structural consequence the table exhibits: at equal
    surface the pair draw runs its pools at TWICE the directional
    pool size, and M3's deaf channel is pool-limited — no pick count
    repairs a pool of surface/2. (M3's publisher seam adds its own
    surface 2(N-1)/B_p for seed-intake attacks — reported, not summed:
    it prices a different attack.) Provenance per row printed."""
    rows = [
        ("M4 ungated K=9 (CIP op point)",
         e18_isolation(N, 9, 1, S), float("inf"),
         "law; measured 200/200 (seed 851)"),
        ("M4 gated K=9  B=500 C=23",
         e18_isolation(N, 9, 500, S), (N - 1) / 500,
         "measured 400/400 (seed 1141)"),
        ("M4 gated K=10 B=500 C=23",
         e18_isolation(N, 10, 500, S), (N - 1) / 500,
         "measured 2x 400/400 (seeds 1139/1140)"),
        ("M3 ungated K=12 s=8 (op point)",
         sum(m3_isolation(N, 12, 1, S, 8, 1)), float("inf"),
         "law; published op-point row"),
        ("M3 gated K=13 s=7 B=769 (r=2 max)",
         sum(m3_isolation(N, 13, 769, S, 7, 769)), 2 * (N - 1) / 769,
         "derived (forms E10-validated)"),
        ("M3 gated K=12 s=8 B=833 (r=2)",
         sum(m3_isolation(N, 12, 833, S, 8, 833)), 2 * (N - 1) / 833,
         "derived"),
        ("M3 gated K=13 s=7 B=1000 (M4-equal surface)",
         sum(m3_isolation(N, 13, 1000, S, 7, 1000)), 2 * (N - 1) / 1000,
         "derived; no K meets 1e-4 here (pool-limited deaf)"),
        ("M3 gated K=13 s=7 B=1250 (the cliff pair)",
         sum(m3_isolation(N, 13, 1250, S, 7, 1250)), 2 * (N - 1) / 1250,
         "measured 17/400 bad, all deaf-class (seed 1145)"),
        ("M4 gated K=10 B=625 C=23 (the cliff pair)",
         e18_isolation(N, 10, 625, S), (N - 1) / 625,
         "measured 400/400 (seed 1146)"),
    ]
    print(f"N={N} S={S} (mu={S / N:.2f})  target P(bad) <= 1e-4")
    print(f"{'configuration':>44} {'P(bad)':>10} {'surface':>8}  provenance")
    for label, e, surface, prov in rows:
        p = 1 - math.exp(-e)
        s_txt = "open" if surface == float("inf") else f"{surface:.0f}"
        print(f"{label:>44} {p:>10.3g} {s_txt:>8}  {prov}")


def capsweep(N, K, B, S, caps):
    base = cell(N, K, B, S)
    e_base = e18_isolation(N, K, B, S)
    print(f"N={N} K={K} B={B} S={S}  fresh h/s={base['admitted_h']:.3f}/"
          f"{base['admitted_s']:.3f}  floor mutual_s={base['mutual_s']:.3f}"
          f"  uncapped E_iso={e_base:.3e} P(bad)={1 - math.exp(-e_base):.4g}")
    print(f"{'C':>5} {'rho':>8} {'blocked_s':>10} {'refused_h':>10} "
          f"{'dE_iso':>10} {'P(bad)':>9}")
    for cap in caps:
        c = cell(N, K, B, S, cap)
        rho = c["refused_h"] / c["admitted_h"] if c["admitted_h"] > 0 else 0.0
        blocked = c["admitted_s"] - c["admitted_s_capped"]
        de = cap_composition(N, K, B, S, rho)
        print(f"{cap:>5} {rho:>8.5f} {blocked:>10.4f} {c['refused_h']:>10.4f} "
              f"{de:>10.3e} {1 - math.exp(-(e_base + de)):>9.4g}")


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    flags = {a.split("=")[0].lstrip("-"): int(a.split("=")[1])
             for a in sys.argv[1:] if a.startswith("--")}
    N = flags.get("n", DEFAULT_N)
    K = flags.get("k", DEFAULT_K)
    S = flags.get("s", DEFAULT_S)
    mode = args[0] if args else "table"
    if mode == "table":
        bs = [int(b) for b in args[1:]] or [250, 400, 500, 625, 740, 845, 1000, 1250]
        design_table(N, K, S, bs)
    elif mode == "cell":
        B = int(args[1])
        cap = int(args[2]) if len(args) > 2 else None
        for k, v in cell(N, K, B, S, cap).items():
            print(f"{k:>22}: {v if isinstance(v, int) or v is None else round(v, 4)}")
        e = e18_isolation(N, K, B, S)
        print(f"{'E18 E_iso':>22}: {e:.3e}   P(bad)~{1 - math.exp(-e):.4g}")
    elif mode == "capsweep":
        B = int(args[1])
        caps = [int(c) for c in args[2:]] or [2, 3, 4, 6, 8, 10, 12, 16, 20]
        capsweep(N, K, B, S, caps)
    elif mode == "compare":
        compare(N, S)
    elif mode == "directional":
        B = int(args[1])
        gate_only = len(args) > 2 and args[2] == "gate-only"
        d, m = directional_isolation(N, K, B, S, gate_only)
        e = d + m
        print(f"E_deaf={d:.4e}  E_mute={m:.4e}  E={e:.4e}  "
              f"P(bad)~{1 - math.exp(-e):.4g}")
    elif mode == "m3":
        B_r, s, B_p = int(args[1]), int(args[2]), int(args[3])
        d, m = m3_isolation(N, K, B_r, S, s, B_p)
        e = d + m
        print(f"E_deaf={d:.4e}  E_mute_seedfail={m:.4e}  E={e:.4e}  "
              f"P(bad)~{1 - math.exp(-e):.4g}")

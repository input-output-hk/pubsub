#!/usr/bin/env python3
"""Exact-arithmetic prediction ledger for the gated-symmetric pass.

Population: N = 4000, adversarial count 800 (mu = 0.2), K = 16 picks.
Pair gate: unordered pair passes with prob 1/B, both ends agree (N-039).

Per-node quantities, all computed with exact binomial sums (no Poisson
or large-r approximations):

  pool of self:          p0 ~ Bin(3999, 1/B)
  pool of a pool-member: p  ~ 1 + Bin(3998, 1/B)   (conditioned to contain me)
  m  = E[min(K,p)/p]      -- prob a given pool member picks me
  out = E[min(K,p0)]      -- own realised picks (= lambda*m, cross-checked)
  d   = lambda*m*(2-m)    -- realised mean degree (out + in - mutual overlap)
  overlap = lambda*m^2    -- mutual-pick collisions (the '~8 links at B=125')

Isolation of an honest node (h honest / a adversarial pool members):
  I = sum_h P(h) * (1-m)^h * E_a[ A(h, a) ]
  A(h,a): h=0 -> 1; h+a<=K -> 0 (I pick everyone, so I pick an honest);
          else C(a,K)/C(h+a,K)  (my K picks all land on adversaries).
  h ~ Bin(3199, 1/B), a ~ Bin(800, 1/B).

E_iso = 3200 * I;  P(bad) ~= 1 - exp(-E_iso)  (isolated-node dominance,
same first-order convention as the formal M4 law).
"""
import math

N = 4000
ADV = 800
HON = N - ADV          # 3200
K = 16
MU = ADV / N

def lchoose(n, k):
    if k < 0 or k > n:
        return -math.inf
    return math.lgamma(n + 1) - math.lgamma(k + 1) - math.lgamma(n - k + 1)

def binom_pmf(n, p, k):
    if k < 0 or k > n:
        return 0.0
    lp = lchoose(n, k) + k * math.log(p) + (n - k) * math.log1p(-p)
    return math.exp(lp)

def pmf_range(n, p, tol=1e-18):
    """(k, pmf) pairs covering all but < tol of the mass."""
    mean = n * p
    sd = math.sqrt(n * p * (1 - p))
    lo = max(0, int(mean - 12 * sd) - 2)
    hi = min(n, int(mean + 12 * sd) + 2)
    out = []
    for k in range(lo, hi + 1):
        q = binom_pmf(n, p, k)
        if q > tol or (lo <= mean <= hi):
            out.append((k, q))
    return out

def member_pick_prob(B):
    """m = E[min(K,p)/p], p = 1 + Bin(3998, 1/B)."""
    tot = 0.0
    for k, q in pmf_range(N - 2, 1.0 / B):
        p = k + 1
        tot += q * min(K, p) / p
    return tot

def own_picks(B):
    tot = 0.0
    for k, q in pmf_range(N - 1, 1.0 / B):
        tot += q * min(K, k)
    return tot

def avoid_term(h, B):
    """E_a[A(h,a)] over a ~ Bin(800, 1/B)."""
    if h == 0:
        return 1.0
    tot = 0.0
    for a, q in pmf_range(ADV, 1.0 / B):
        if h + a <= K:
            continue          # pool <= K: I pick the whole pool -> pick an honest
        if a < K:
            continue          # cannot place all K picks on adversaries
        tot += q * math.exp(lchoose(a, K) - lchoose(h + a, K))
    return tot

def isolation_gated_picks(B):
    m = member_pick_prob(B)
    tot = 0.0
    for h, q in pmf_range(HON - 1, 1.0 / B):
        f = q * (1.0 - m) ** h
        if f < 1e-24 and h > (HON - 1) / B:
            break
        tot += f * avoid_term(h, B)
    return tot

def isolation_gate_only(B):
    return (1.0 - 1.0 / B) ** (HON - 1)

def naive_transfer(d):
    """M4 law read at realised degree: RF_eff = d/2, product form."""
    rf = d / 2.0
    return HON * math.exp(-rf * (math.log(1.0 / MU) + (1.0 - MU)))

def pbad(e_iso):
    return 1.0 - math.exp(-e_iso)

print(f"{'cell':<22}{'lam':>8}{'r':>7}{'m':>8}{'out':>7}{'d':>8}"
      f"{'ovl':>7}{'I_iso':>11}{'E_iso':>11}{'P(bad)':>10}{'naive@d':>11}")

cells = [10, 50, 125, 167, 250, 500]
for B in cells:
    lam = (N - 1) / B
    r = lam / K
    m = member_pick_prob(B)
    out = own_picks(B)
    d = lam * m * (2.0 - m)
    ovl = lam * m * m
    iso = isolation_gated_picks(B)
    e = HON * iso
    print(f"{'gated-picks B=' + str(B):<22}{lam:>8.2f}{r:>7.3f}{m:>8.4f}"
          f"{out:>7.2f}{d:>8.2f}{ovl:>7.2f}{iso:>11.3e}{e:>11.3e}"
          f"{pbad(e):>10.4g}{naive_transfer(d):>11.3e}")
    # cross-check: out must equal lam*m
    assert abs(out - lam * m) < 1e-6, (out, lam * m)

for B in (125, 250):
    lam = (N - 1) / B
    iso = isolation_gate_only(B)
    e = HON * iso
    print(f"{'gate-only B=' + str(B):<22}{lam:>8.2f}{'-':>7}{'1':>8}"
          f"{lam:>7.2f}{lam:>8.2f}{lam:>7.2f}{iso:>11.3e}{e:>11.3e}"
          f"{pbad(e):>10.4g}{naive_transfer(lam):>11.3e}")

# ungated cells (no gate): twin K'=8 and the seed-44 anchor K=16
for Kp, label in ((8, "ungated twin K=8"), (16, "anchor K=16")):
    d = 2 * Kp - Kp * Kp / (N - 1)
    picks_adv = math.exp(lchoose(ADV, Kp) - lchoose(N - 1, Kp))
    no_inbound = (1.0 - Kp / (N - 1)) ** (HON - 1)
    iso = picks_adv * no_inbound
    e = HON * iso
    print(f"{label:<22}{'-':>8}{'-':>7}{'-':>8}{Kp:>7.2f}{d:>8.2f}"
          f"{Kp*Kp/(N-1):>7.2f}{iso:>11.3e}{e:>11.3e}{pbad(e):>10.4g}"
          f"{naive_transfer(d):>11.3e}")

# derived design constants
print()
lam_cross = K * (math.log(1.0 / MU) + (1.0 - MU)) / (1.0 - MU)
print(f"channel crossover: lambda = {lam_cross:.1f}  (r = {lam_cross/K:.2f},"
      f"  B = {(N-1)/lam_cross:.0f})")
for delta in (1e-3, 1e-4, 1e-5):
    lam_floor = math.log(HON / delta) / (1.0 - MU)
    print(f"pool floor for delta={delta:g}: lambda >= {lam_floor:.1f}"
          f"  (B <= {(N-1)/lam_floor:.0f})")

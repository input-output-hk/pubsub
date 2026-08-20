import math
from math import log, floor, comb

mu, delta = 0.2, 1e-4

def B_pool(N):
    H = (1 - mu) * N
    return (N - 1) * (1 - mu) / log(H / delta)

def B_head(N, k):
    return (N - 1) / (2 * k)

def ceil_(N, k):
    bp, bh = floor(B_pool(N)), floor(B_head(N, k))
    return bp, bh, min(bp, bh)

print("== continuous ceilings, k=10 ==")
for N in [5, 9, 10, 11, 12, 20, 33, 40, 41, 50, 99, 100, 500, 999, 1000,
          3000, 4000, 9999, 10000, 20000, 99999, 100000]:
    bp, bh, m = ceil_(N, 10)
    print(f"N={N:>7}  B_pool={B_pool(N):11.4f}->{bp:<6} B_head={B_head(N,10):10.4f}->{bh:<6} min={m}")

print()
print("== k lever: max achievable B at each band bottom, over k ==")
for N in [10, 100, 1000, 10000]:
    print(f"N={N}: floor(B_pool)={floor(B_pool(N))}")
    for k in [6, 7, 8, 9, 10, 11, 12]:
        print(f"   k={k:>2} head={floor(B_head(N,k)):>5} min={min(floor(B_pool(N)),floor(B_head(N,k))):>5}")

print()
print("== pool/headroom crossover at k=10: ln(8000N)=16 ==")
print("N* =", math.exp(16) / 8000)
for N in [900, 1000, 1100, 1111, 1112, 1200, 1500]:
    binding = 'pool' if B_pool(N) < B_head(N, 10) else 'headroom'
    print(f"  N={N}: pool={B_pool(N):.4f} head={B_head(N,10):.4f} binding={binding}")

print()
print("== first N where gate can legally open (min ceiling >= 2), k=10 ==")
for N in range(2, 80):
    if ceil_(N, 10)[2] >= 2:
        print("  first N with min>=2 at k=10:", N, ceil_(N, 10))
        break
for N in range(2, 80):
    if floor(B_pool(N)) >= 2:
        print("  first N with B_pool>=2 (any k):", N, f"{B_pool(N):.4f}")
        break

print()
print("== availability tail: eligible ~ Binom(N-1, 1/B) ==")
def tail(n, p, k):
    return sum(comb(n, i) * p**i * (1 - p)**(n - i) for i in range(0, k))

for (N, B) in [(100, 2), (100, 3), (100, 4), (100, 5), (120, 4), (150, 4), (200, 4),
               (300, 4), (999, 4), (1000, 20), (1000, 40), (1000, 49), (1000, 50),
               (2000, 40), (3000, 40), (9999, 40), (10000, 200), (10000, 400),
               (10000, 439), (20000, 400), (20000, 500)]:
    t_all = tail(N - 1, 1 / B, 10)
    t_hon = tail(int(round((1 - mu) * N)) - 1, 1 / B, 10)
    print(f"N={N:>6} B={B:>4} E[elig]={(N-1)/B:9.2f} P(elig<10)={t_all:.3e} honest-only P(<10)={t_hon:.3e}")

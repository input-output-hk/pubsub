from math import log, floor, comb, lgamma, exp
mu, delta = 0.2, 1e-4

def logcomb(n, i):
    return lgamma(n + 1) - lgamma(i + 1) - lgamma(n - i + 1)

def tail(n, p, k):
    """P(Binom(n,p) < k) computed in log space."""
    s = 0.0
    for i in range(0, k):
        s += exp(logcomb(n, i) + i * log(p) + (n - i) * log(1 - p))
    return s

def B_avail(N, k, union=True):
    """largest B s.t. availability failure meets delta (honest candidates only)"""
    H = (1 - mu) * N
    n = int(round(H)) - 1
    best = 1
    for B in range(2, 200000):
        t = tail(n, 1.0 / B, k)
        if union:
            t = t * H
        if t <= delta:
            best = B
        else:
            break
    return best

print("== calibration: told B_target ~ 730 at N=20000, k=9 ==")
for (N, k) in [(20000, 9), (20000, 10), (4000, 10), (4000, 9)]:
    bu = B_avail(N, k, union=True)
    bp = B_avail(N, k, union=False)
    print(f"N={N} k={k}: B_avail(union over H)={bu}  B_avail(per-node)={bp}   (N-1)/(3k)={(N-1)/(3*k):.1f}")

print()
print("== proxy forms at N=20000,k=9 ==")
print("  (N-1)/(3k) =", 19999 / 27)
print("  eligible at B=730:", 19999 / 730, "= ", 19999 / 730 / 9, "x k")

print()
print("== candidate patterns: 3*10^(d-3) vs 4*10^(d-3), k=10 ==")
def B_pool(N): return (N - 1) * (1 - mu) / log((1 - mu) * N / delta)
def B_head(N, k): return (N - 1) / (2 * k)
for N in [100, 1000, 10000]:
    print(f"-- N={N}: floor(pool)={floor(B_pool(N))} floor(head,k=10)={floor(B_head(N,10))} "
          f"B_avail(union,k=10)={B_avail(N,10,True)} B_avail(per-node,k=10)={B_avail(N,10,False)}")
    for B in [2, 3, 4, 5]:
        BB = B * (N // 100)
        H = (1 - mu) * N
        t = tail(int(round(H)) - 1, 1.0 / BB, 10)
        print(f"    B={BB:>4} E[elig_all]={(N-1)/BB:7.2f} E[elig_honest]={(H-1)/BB:7.2f} "
              f"per-node P(<10)={t:.3e}  union*H={t*H:.3e}")

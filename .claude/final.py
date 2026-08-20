from math import log, floor
mu, delta = 0.2, 1e-4
K = 10

def B_pool(N): return (N - 1) * (1 - mu) / log((1 - mu) * N / delta)
def B_head(N, k=K): return (N - 1) / (2 * k)
def ceiling(N, k=K): return min(floor(B_pool(N)), floor(B_head(N, k)))

BANDS = [(1, 9, 1), (10, 99, 1), (100, 999, 3), (1000, 9999, 30), (10000, None, 300)]

print("=== SAFETY AT EACH BAND FLOOR (k=10) ===")
for lo, hi, B in BANDS:
    bp, bh = B_pool(lo), B_head(lo)
    print(f"floor N={lo:>6}  B_pool={bp:12.4f} ->floor {floor(bp):>5}   "
          f"B_head={bh:11.4f} ->floor {floor(bh):>5}   min={ceiling(lo):>5}   "
          f"chosen B={B:>4}  OK={'yes' if (B == 1 or B <= ceiling(lo)) else 'NO'}")

print()
print("=== B_target proxy: calibrated ratio elig/k = 3.04 at the one known point ===")
print(f"  known: N=20000 k=9 B_target~730 -> elig=(N-1)/B={19999/730:.2f} = {19999/730/9:.3f} k")
print("  so B_target ~ (N-1)/(3.04*k); at k=10 that is (N-1)/30.4")
for lo, hi, B in BANDS[2:]:
    print(f"  floor N={lo:>6}: proxy B_target={(lo-1)/30.4:9.2f}  chosen B={B:>4}  "
          f"elig at floor=(N-1)/B={(lo-1)/B:7.2f} = {(lo-1)/B/K:.2f} k")

print()
print("=== DILUTION LOSS: ceiling at band top / band B ===")
worst = (0, None, None)
for lo, hi, B in BANDS:
    if hi is None:
        for N in [20000, 99999, 10**6, 10**7]:
            print(f"  open band, N={N:>9}: ceiling={ceiling(N):>7}  B={B}  loss={ceiling(N)/B:8.2f}x")
        continue
    c = ceiling(hi)
    c = max(c, 1)
    loss = c / B
    print(f"  band {lo}-{hi}: ceiling at top N={hi} is {c:>5}  B={B:>4}  loss={loss:6.2f}x")
    if loss > worst[0]:
        worst = (loss, hi, B)
print(f"  WORST closed-band loss: {worst[0]:.2f}x at N={worst[1]} (B={worst[2]})")

print()
print("=== REAL POPULATIONS ===")
for N, label in [(10, "wallet-backend providers"), (30, "wallet-backend, larger"),
                 (3000, "always-on stake pools"), (4000, "evidence point"),
                 (20000, "evidence point / specified B=500")]:
    d = len(str(N))
    B = {1: 1, 2: 1, 3: 3, 4: 30}.get(d, 300)
    c = max(ceiling(N), 1)
    print(f"  N={N:>6} ({label:<32}) band={d}-digit B={B:>4} k={K} "
          f"ceiling={c:>5} loss={c/B:6.2f}x elig=(N-1)/B={(N-1)/B:8.2f}")

print()
print("=== k LEVER: what dropping to k=8 would buy (pattern 4/40/400) ===")
for lo in [100, 1000, 10000]:
    for k in [8, 9, 10]:
        print(f"  N={lo:>6} k={k:>2}: pool={floor(B_pool(lo)):>5} head={floor(B_head(lo,k)):>5} "
              f"targetproxy={(lo-1)/(3.04*k):8.2f} -> max B={min(floor(B_pool(lo)),floor(B_head(lo,k)),int((lo-1)/(3.04*k))):>5}")
print(f"  worst loss with 4/40/400: {ceiling(999)}/4 = {ceiling(999)/4:.2f}x  vs 3/30/300: {ceiling(999)}/3 = {ceiling(999)/3:.2f}x")

print()
print("=== gate switch-off / clique ===")
print("  B=1 for every N <= 99 -> gate off below 100, on from 100 up")
for N in [9, 10, 11, 12, 20]:
    print(f"  N={N}: eligible=N-1={N-1}, k={K} -> {'complete graph (k >= N-1)' if K >= N-1 else 'gate off, picks %d of %d' % (K, N-1)}")
print()
print("  monotonicity check (both ceilings increase in N):")
prev = -1
for N in [100, 200, 500, 1000, 5000, 10000, 50000, 100000, 10**6]:
    c = ceiling(N)
    assert c > prev
    prev = c
print("  OK: min(B_pool,B_headroom) strictly increasing over sampled N")

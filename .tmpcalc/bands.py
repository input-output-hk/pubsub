import math
mu = 0.2
delta = 1e-4

def bpool(N):
    H = (1 - mu) * N
    return math.floor((N - 1) * (1 - mu) / math.log(H / delta))

def bhead(N, k):
    return math.floor((N - 1) / (2 * k))

def ceil_(N, k):
    return min(bpool(N), bhead(N, k))

print("=== gate switch-on scan, k=10 ===")
print("  N  Bpool Bhead ceil")
for N in range(28, 50):
    print(f"{N:4d} {bpool(N):5d} {bhead(N,10):5d} {ceil_(N,10):4d}")

print()
print("=== ceilings at key populations (k=10) ===")
print("      N   Bpool  Bhead10 Bhead9 Bhead8 Bhead6  binds")
for N in [41,79,80,159,160,319,320,639,640,1249,1250,2499,2500,3000,3999,4000,4999,5000,9999,10000,19999,20000,20001,40000,50000,100000]:
    p = bpool(N); h = bhead(N,10)
    which = 'pool' if p < h else ('head' if h < p else 'tie')
    print(f"{N:7d} {p:7d} {h:8d} {bhead(N,9):6d} {bhead(N,8):6d} {bhead(N,6):6d}  {which}")

print()
print("=== where pool overtakes headroom (k=10) ===")
for N in range(100, 3000):
    if bpool(N) <= bhead(N, 10):
        print("pool <= headroom first at N =", N, bpool(N), bhead(N,10))
        break
for N in range(3000, 100, -1):
    if bhead(N, 10) < bpool(N):
        print("last N where headroom strictly binds:", N, bpool(N), bhead(N,10))
        break

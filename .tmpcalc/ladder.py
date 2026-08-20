import math
mu = 0.2
delta = 1e-4

def bpool(N):
    return math.floor((N - 1) * (1 - mu) / math.log((1 - mu) * N / delta))

def bhead(N, k):
    return math.floor((N - 1) / (2 * k))

def ceil_(N, k=10):
    return min(bpool(N), bhead(N, k))

# smallest N at which the computable ceiling reaches each power-of-two target
targets = [2, 4, 8, 16, 32, 64, 128, 256, 512, 1024]
first = {}
for t in targets:
    for N in range(2, 400000):
        if ceil_(N) >= t:
            first[t] = N
            break
print("target B -> smallest N whose ceiling reaches it (k=10)")
for t in targets:
    N = first[t]
    print(f"  B>={t:5d}  N={N:7d}   Bpool={bpool(N):6d} Bhead={bhead(N,10):6d}")

import math
mu, delta, k = 0.2, 1e-4, 10

def bpool(N):
    return math.floor((N - 1) * (1 - mu) / math.log((1 - mu) * N / delta))
def bhead(N, k=k):
    return math.floor((N - 1) / (2 * k))
def ceil_(N, k=k):
    return min(bpool(N), bhead(N, k))

Bs = [1, 2, 4, 8, 16, 32, 64, 128, 256, 512]
bottom = {1: 2}
for B in Bs[1:]:
    for N in range(2, 500000):
        if ceil_(N) >= B:
            bottom[B] = N
            break

print("band                B    k  | Bpool(lo) Bhead(lo) ceil(lo) | ceil(hi)  loss=ceil(hi)/B | (lo-1)/B  (hi)/B")
for i, B in enumerate(Bs):
    lo = bottom[B]
    if i + 1 < len(Bs):
        hi = bottom[Bs[i + 1]] - 1
        chi = ceil_(hi)
        loss = chi / B
        hi_s, chi_s, loss_s = f"{hi}", f"{chi}", f"{loss:.3f}"
    else:
        hi_s, chi_s, loss_s = "open", "-", "-"
        hi = None
    rng = f"{lo}-{hi_s}" if i else f"2-{hi_s}"
    print(f"{rng:>14} {B:6d} {k:4d} | {bpool(lo):9d} {bhead(lo):9d} {ceil_(lo):8d} | {chi_s:>8} {loss_s:>10} |"
          f" {((lo-1)/B):7.1f} {'' if hi is None else f'{(hi-1)/B:7.1f}'}")

print()
print("ratio of successive band bottoms (near-doubling check)")
prev = None
for B in Bs[1:]:
    if prev: print(f"  {bottom[prev]:6d} -> {bottom[B]:6d}   x{bottom[B]/bottom[prev]:.3f}")
    prev = B

print()
print("=== can a smaller k advance any band to the next power of two? ===")
print("(needs Bpool(lo) >= 2B; Bpool does not depend on k)")
for i, B in enumerate(Bs[1:-1]):
    lo = bottom[B]
    print(f"  band bottom N={lo:6d}: B={B:4d}  next={2*B:4d}  Bpool={bpool(lo):5d}  "
          f"{'possible' if bpool(lo) >= 2*B else 'BLOCKED by Bpool'}")

print()
print("=== largest k each band bottom supports at its B (headroom) ===")
for B in Bs[1:]:
    lo = bottom[B]
    kmax = (lo - 1) // (2 * B)
    print(f"  N={lo:6d} B={B:4d}  kmax={kmax}")

print()
print("=== real populations ===")
for N in [8, 10, 12, 20, 40, 41, 3000, 4000, 20000, 24438, 40000]:
    B = max(b for b in Bs if bottom[b] <= N)
    print(f"  N={N:6d} -> band B={B:4d}  continuous ceiling={ceil_(N):6d} "
          f"(pool {bpool(N)}, head {bhead(N)})  loss={ceil_(N)/B:.2f}  eligible=(N-1)/B={(N-1)/B:.1f}")

print()
print("=== ln(H/delta) at band bottoms (pool-bound bands) ===")
for B in [64, 128, 256, 512]:
    lo = bottom[B]
    print(f"  N={lo:6d}  ln(0.8N/1e-4)={math.log(0.8*lo/delta):.3f}  honest eligible=0.8*(N-1)/B={0.8*(lo-1)/B:.2f}")

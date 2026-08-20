import math
mu, delta = 0.2, 1e-4
bottoms = [(1, 2, 40), (2, 41, 80), (4, 81, 160), (8, 161, 320), (16, 321, 640),
           (32, 641, 1293), (64, 1294, 2703), (128, 2704, 5641), (256, 5642, 11750),
           (512, 11751, None)]
for B, lo, hi in bottoms:
    num = (lo - 1) * (1 - mu)
    L = math.log((1 - mu) * lo / delta)
    print(f"B={B:4d} lo={lo:6d}: 0.8*(N-1)={num:9.1f}  ln(0.8N/1e-4)={L:7.4f}  "
          f"Bpool={num/L:9.3f}->{math.floor(num/L):5d}   Bhead=(N-1)/20={(lo-1)/20:9.2f}->{(lo-1)//20:5d}"
          f"   B<=min? {B <= min(math.floor(num/L), (lo-1)//20)}")
    if hi:
        ph = math.floor((hi - 1) * 0.8 / math.log(0.8 * hi / delta)); hh = (hi - 1) // 20
        print(f"          hi={hi:6d}: Bpool={ph:6d} Bhead={hh:6d} ceiling={min(ph,hh):6d}  loss={min(ph,hh)/B:.3f}")
print()
for N in [20000, 24438, 30000, 40000]:
    c = min(math.floor((N-1)*0.8/math.log(0.8*N/delta)), (N-1)//20)
    print(f"open top band at N={N:6d}: ceiling={c:5d} loss vs 512 = {c/512:.2f}")

# Silent Attacks on SecureCyclon — Team Report

Silent = genuine, rate-honest, single-chain descriptors only; the adversary varies only *which*
peer it contacts, *which* descriptors it sends, and *what* it withholds — none of which
SecureCyclon's nine defences can prove. All results are measured against the **faithful full
protocol** (honest views stay full at ℓ=20 via §V-A repair, tit-for-tat, D3/D4 checks,
blacklisting). Config: N=200, ℓ=20, s=3, attack@cycle 50, 200 cycles, 3 seeds, victim = first
honest node.

## Attacks

- **`bias`** — network-wide biased subset: fill swap slots toward honest peers adversary-pointing
  first, hoard the legitimate descriptors received. Amplifies the malicious link share toward the
  conservation ceiling `m/(n−m)`; no single node eclipsed.
- **`concentrate`** — targeted: reciprocate *legitimate* descriptors to non-victims while hoarding
  adversary-pointing ammunition to spend only on the victim T.
- **`refuse`** — targeted selective silence: decline to *reply* only when a *non-victim* honest peer
  initiates contact (engage T fully). A non-responding partner is indistinguishable from churn.
- **`healer`** — targeted: learn T's current healers from received samples and target them too,
  cutting T's re-heal supply at the source.
- **`token_dup`** — targeted: linear prefix-extension duplication of victim-tokens (`A→B→C` kept,
  `A→B→C→D` forwarded); every copy is a prefix of the next, so D4 never fires. Silent *only* while
  strictly linear and aimed at T — aimed at healers it forks chains and D4 fires (mass blacklisting).

## Results — targeted silent eclipse

Victim's local malicious-view fraction `A_T_mean` (with eclipse% = share of cycles A_T≥0.8), for
the three **silent** stacks. All rows: **det=0**, honest views full (~20).

| μ | `m/(n−m)` | `concentrate,refuse` | `+healer` | `+token_dup` |
|------|-----------|----------------------|-----------|--------------|
| 0.05 | 0.053 | 0.100 (0%) | 0.107 (0%) | 0.179 (0%) |
| 0.10 | 0.111 | 0.277 (0%) | 0.263 (0%) | 0.350 (0%) |
| 0.15 | 0.176 | 0.523 (2%) | 0.439 (0%) | 0.562 (2%) |
| 0.20 | 0.250 | 0.743 (44%) | 0.725 (37%) | 0.803 (68%) |
| 0.30 | 0.429 | 0.986 (100%) | 0.990 (100%) | 0.993 (100%) |

The victim sees 2–3× the network-wide `m/(n−m)` average (the bound is an average, not a per-node
guarantee). From μ≈0.20 full eclipse is common; at μ≈0.30 it is total and the victim's honest
in-degree collapses to ~0 (full bidirectional isolation). `+token_dup` raises A_T across the range;
`+healer` keeps A_T similar but chokes the victim's honest in-degree harder (e.g. 2.7 vs 5.3 at
μ=0.20). **`healer` and `token_dup` together are excluded** — duplicating actively-circulated
healer-tokens forks chains → D4 fires (mass blacklisting).

## How to run

Self-contained, **stdlib only** (no numpy/matplotlib):

```bash
# run from this folder (reproduce_faithful.py + securecyclon.py)
python3 reproduce_faithful.py --mu 0.15                                   # honest baseline (views full, A_mean≈μ)
python3 reproduce_faithful.py --mu 0.20 --attacks bias --seeds 1,2,3      # network-wide ceiling m/(n−m)
for atk in concentrate,refuse concentrate,refuse,healer concentrate,refuse,token_dup; do
  for mu in 0.05 0.10 0.15 0.20 0.30; do                                  # the three silent table columns
    python3 reproduce_faithful.py --mu $mu --attacks $atk --seeds 1,2,3
  done
done
python3 reproduce_faithful.py --mu 0.10 --attacks concentrate,refuse,healer,token_dup --seeds 1,2,3  # LOUD control (D4 fires)
```

`reproduce_faithful.py` drives `securecyclon.py` (the paper-validated full protocol; honest views
stay full at ℓ=20). Run `python3 reproduce_faithful.py --help` for the full option/attack reference.

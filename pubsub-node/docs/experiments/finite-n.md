# Where the coverage laws lose accuracy, and in what

**The laws run about 2 % optimistic pooled over the whole corpus. Is that a
finite-population effect?**

The coverage laws are asymptotic approximations, so the natural explanation for
a small systematic bias is that it grows as the population shrinks. That is a
testable claim, and testing it took four rounds, three of which produced a
reading the next round overturned. The corrections are kept here rather than
tidied away, because the method is most of what this document has to offer.

## Provenance

| | |
|---|---|
| Tool commit | as recorded in each cell's `manifest.json` |
| Configurations | [`configs/experiments/finite-n/`](../../configs/experiments/finite-n/) — 16 cells, all at μ = 0.2, 60 000 draws each, master seeds 901–934 |
| Scoring | each cell against its own design's law at that cell's parameters, recomputed here and checked to agree with the published law values to 0.01 % on all 25 corpus cells |

## 1. What was run, and what came back

| design | parameters | N | bad / draws | law | measured / law | z | seed | group |
|---|---|---:|---:|---:|---:|---:|---:|---|
| M1 | F = 11 | 1,000 | 6,468 / 60,000 | 0.10873 | **0.9914 ± 0.0123** | -0.73 | 901 | ladder |
| M2 | RF = 11 | 1,000 | 6,637 / 60,000 | 0.11376 | **0.9724 ± 0.0119** | -2.42 | 902 | ladder |
| M3 | RF = 6, s = 4 | 1,000 | 6,201 / 60,000 | 0.09515 | **1.0862 ± 0.0138** | +6.85 | 903 | ladder |
| M4 | RF = 4 | 1,000 | 2,891 / 60,000 | 0.04958 | **0.9719 ± 0.0181** | -1.57 | 904 | ladder |
| M5 | (4, 4) | 1,000 | 5,875 / 60,000 | 0.09670 | **1.0126 ± 0.0132** | +1.01 | 905 | ladder |
| M2 | RF = 11 | 1,000 | 6,482 / 60,000 | 0.11376 | **0.9497 ± 0.0118** | -4.41 | 915 | control |
| M3 | RF = 6, s = 4 | 1,000 | 6,083 / 60,000 | 0.09515 | **1.0655 ± 0.0137** | +5.20 | 913 | control |
| M3 | RF = 7, s = 3 | 1,000 | 7,301 / 60,000 | 0.11788 | **1.0323 ± 0.0121** | +2.89 | 914 | control |
| M2 | RF = 12 | 2,000 | 6,004 / 60,000 | 0.10277 | **0.9737 ± 0.0126** | -2.18 | 921 | gradient |
| M2 | RF = 14 | 8,000 | 4,922 / 60,000 | 0.08381 | **0.9788 ± 0.0140** | -1.57 | 923 | gradient |
| M3 | RF = 7, s = 4 | 2,000 | 4,033 / 60,000 | 0.06424 | **1.0464 ± 0.0165** | +2.98 | 922 | gradient |
| M3 | RF = 8, s = 4 | 8,000 | 5,744 / 60,000 | 0.09597 | **0.9975 ± 0.0132** | -0.20 | 924 | gradient |
| M2 | RF = 11 | 2,000 | 12,688 / 60,000 | 0.21457 | **0.9855 ± 0.0087** | -1.85 | 933 | fixed k |
| M2 | RF = 11 | 4,000 | 22,777 / 60,000 | 0.38310 | **0.9909 ± 0.0066** | -1.76 | 934 | fixed k |
| M3 | RF = 6, s = 4 | 2,000 | 11,771 / 60,000 | 0.18440 | **1.0639 ± 0.0098** | +7.44 | 931 | fixed k |
| M3 | RF = 6, s = 4 | 4,000 | 21,373 / 60,000 | 0.33738 | **1.0558 ± 0.0072** | +9.76 | 932 | fixed k |


## 2. Round one: the hypothesis fails

Five designs at N = 1 000, matched so each cell's law sat near 0.1. Pooled,
they came to **1.008 ± 0.006**, against 1.027 ± 0.012 at N = 4 000 and
1.004 ± 0.016 at N = 20 000. If the bias grew as the population shrank, N =
1 000 should have been the worst; it was among the best. **The hypothesis
does not survive its first test.**

## 3. Round two: pooling was hiding two effects

The pooled figure was concealing something. Per design at N = 1 000, M3 came
in 6 % *above* its law and M2 4 % *below*, each many standard errors out, and
the two nearly cancel. M1, M4 and M5 sat within 2.4 σ. A control cell — M2
repeated on a fresh seed — was what exposed it: it was expected to be dull and
came back at z = −4.4, which ruled out an M3-specific defect and reframed the
result as design-dependent deviation in both directions.

Before going further the law implementation was checked against the 25
published corpus cells and reproduces every one to 0.01 %, so what follows is
not a transcription error.

## 4. Round three: a confound, self-inflicted

A gradient ladder at N = 2 000 and 8 000 appeared to show both designs decaying
smoothly toward their laws as the population grew. It did not. Holding each
cell's law near 0.1 requires raising the pick count as N rises, so the ladder
varied **two** things at once:

| N | M2 pick count | M3 pick count |
|---:|---:|---:|
| 1,000 | 11 | 6 |
| 2,000 | 12 | 7 |
| 8,000 | 14 | 8 |

"The deviation shrinks with population" and "the deviation shrinks with fanout"
fit that data equally well. The ladder could not separate them, and reading it
as the former was wrong.

## 5. Round four: hold the pick count, vary the population

The disambiguating cells hold each design's pick count at its N = 1 000 value
and vary N alone. The law value rises as a result, which costs nothing: more
failures per draw is more power, not less.

**M3 at RF = 6, s = 4**

| N | measured / law |
|---:|---:|
| 1,000 | 1.059 ± 0.008 |
| 2,000 | 1.064 ± 0.010 |
| 4,000 | 1.056 ± 0.007 |

Flat. Eight tenths of a percentage point of spread across a fourfold change in
population, inside the error bars. **M3's deviation is not a population effect.**
Sorted by pick count instead, across every M3 cell measured anywhere, it is
about 6 % at RF = 6 and around 2 % at the RF = 12–13 its operating point uses.

**M2 at RF = 11**

| N | measured / law |
|---:|---:|
| 1,000 | 0.961 ± 0.008 |
| 2,000 | 0.986 ± 0.009 |
| 4,000 | 0.991 ± 0.007 |

Monotone, three percentage points, and the two ends differ by 2.9 σ. **M2's
deviation is a population effect**, decaying as an asymptotic approximation
should.

## 6. What this establishes

- The pooled 2 % is **not one phenomenon**. It is at least two, of opposite
  sign, which happen to cancel.
- **M3's law is optimistic at low fanout, at any population tested.** This is
  not a small-topic caveat: it applies at deployment scale wherever the pick
  count is small.
- **M2's law is pessimistic at small populations** and converges as they grow.
- Both are mild where it matters. Both designs' operating points sit where
  their deviation is around 2 %, which moves a target of 10⁻⁴ to about
  1.02 × 10⁻⁴.
- The consequence for a *comparison* is larger than for either design alone:
  two designs whose deviations differ by several percent in opposite directions
  cannot be compared to better than that, and the margins separating the
  candidates are of that order.

## 7. Limits

- **Two designs, not five.** M1, M4 and M5 were measured only at N = 1 000 and
  only in the first round. Whether they carry deviations of their own, and of
  which kind, is untested.
- **The mechanism is unidentified.** That M3's deviation tracks the pick count
  is measured; *why* is not. It is consistent with the small-component
  approximation in the law's second term, which the seeding links dominate at
  low RF, but nothing here demonstrates that.
- **One adversarial fraction.** Every cell is at μ = 0.2.
- **Interpolation only.** No cell here sits above N = 8 000, so the behaviour
  at the 20 000 the proposal targets is carried by the published corpus rather
  than by this work.

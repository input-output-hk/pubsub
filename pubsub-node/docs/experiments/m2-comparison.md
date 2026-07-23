# M2 comparison — experiments framework vs the formal M2 model

The experiments framework's shipped M2-comparison demonstration, executed
manually per the feature quickstart. The comparison **informs, it does not
gate**: its purpose is to show the instrument and the formal simulators
measure the same protocol the same way, and to surface any convention
differences worth raising.

## Provenance

| | |
|---|---|
| Tool commit | `493bb2f` (post-015-rebase; all three sections re-executed at this commit). The re-executed artifacts are **byte-identical** to the original execution's (tool commits `e14a3f2`/`60b8e84`, pre-rebase lineage): `runs.jsonl` and `aggregates.json` verified by direct diff for all three sections, manifests differing only in the tool commit and the fan-out kind's rename — the 015 integration (relay-only populations over the re-keyed link model, `forward-to-relays` fan-out) leaves the M2 instrument's output unchanged to the byte |
| Operating point | `configs/experiments/m2-operating-point.toml`, master seed **42**, 40 runs, ~13–23 min at `--workers 1` (release build; ~30 GB peak per in-flight run) |
| Bulk-regime point | `configs/experiments/m2-bulk-regime.toml`, master seed **4016**, 8000 runs, ~30 min at default workers (~1.3 GB per in-flight run) |
| Grid-cell check | the operating-point configuration with `target_degree = 16` (both classes), `runs_per_experiment = 150`, `master_seed = 20016`; ~37 min at `--workers 1` |
| Reference values | `formal_spec/hybrid_dissemination/models/comparison.md` §2 and `models/m2/properties/full_coverage.md` §2–§3 |

Raw artifacts are deliberately **not** committed: the tool commit and master
seeds above reproduce them byte-for-byte —

```sh
cargo build --release --features experiments --bins
./target/release/experiments --config configs/experiments/m2-operating-point.toml --out results/m2-op/ --workers 1
./target/release/experiments --config configs/experiments/m2-bulk-regime.toml  --out results/m2-bulk/
```

Parameter mapping: the model's μ is its adversarial fraction (μ = k/N; its
"dead" relays accept and never forward), which maps to
`adversarial_fraction` with `silent-relay` fan-out here. The model has no
separate churn term, so these configurations run churn-free.

## 1. Operating point — cost and latency means (N = 20 000, μ = 0.2, RF = 24)

Population: 16 000 honest, 4 000 silent adversaries; uniform-sampler dial
(exactly-RF picks, the model's selection family), accept-from-all,
forward-to-relays. All 40 runs formed good topologies and delivered full
coverage — consistent with the model's P(bad) ≈ 7.3×10⁻⁵ at this point
(resolving that probability needs ~10⁴ runs; 40 runs certify only its
order-of-magnitude absence).

| quantity | published (comparison.md, M2 row) | measured (mean over 40 runs) | deviation |
|---|---|---|---|
| honest→honest sends per message | 307 153 | **307 182.2** | +0.0095 % |
| copies per honest node | 19.2 | **19.20** | — |
| hops, full coverage | 4.8 | **4.80** (8 runs at 4, 32 at 5) | exact |
| hops, mean first receipt | 3.6 | **3.59** | −0.3 % |

Depth distribution (pooled honest first receipts over all 40 runs; wave 0 =
the publisher's own record, 640 000 receipts total):

| wave | 0 | 1 | 2 | 3 | 4 | 5 |
|---|---|---|---|---|---|---|
| receipts | 40 | 801 | 15 268 | 229 254 | 394 250 | 387 |

Supporting numbers: sends to adversarial recipients 76 820.8 per message
(expectation A·RF·H/(N−1) ≈ 76 804; the gap is within a 40-run mean's
sampling noise); total sends p50/p90/p99 =
383 971 / 384 337 / 384 621; duplication ratio (redundant arrivals over all
arrivals) 0.948, the flooding-with-dedup cost the RF = 24 sizing implies.

**Deviation notes.** The honest→honest sends figure differs from the
published table by 29 messages in ~3×10⁵ (0.0095 %). The pre-split-horizon
expectation for exactly-RF uniform picks is H·RF·(H−1)/(N−1) ≈ 307 196;
split-horizon suppression (a node never echoes to its first deliverer, so
mutual-pick pairs drop one send) accounts for the measured mean sitting
~14 below it, and the residual 29-message gap to the published value is
≈ 0.7σ of a 40-run mean — finite-sample noise, not a protocol difference.
The depth means agree to the published table's precision.

## 2. Bulk-regime point — P(good) vs the coverage law (N = 4 000, μ = 0.2, RF = 16)

The named validation point from `full_coverage.md` §3's small-N tail ladder
(`sweep_m2_cost.py --coverage`): P(bad) predicted 0.0088, measured there
0.0081 (65 bad / 8000 trials). This sweep runs the same 8000 trials.

| source | bad / trials | P(bad) | 95 % interval |
|---|---|---|---|
| coverage law (guiding formula) | — | 0.0088 | — |
| formal Monte Carlo | 65 / 8000 | 0.0081 | — (±1σ ≈ ±0.0010) |
| **this framework** | **76 / 8000** | **0.0095** | Wilson [0.0076, 0.0119] |

Agreement: the law's prediction and the formal Monte-Carlo value both lie
inside our Wilson 95 % interval. Against the law's expected 70.4 bad graphs
the observed 76 is z ≈ +0.67; against the formal Monte Carlo, z ≈ +0.93 —
both within one standard deviation, i.e. ordinary sampling noise.

Two structural cross-checks land exactly as the model describes:

- **The out-defect dominates.** All 76 bad graphs score
  min-publisher-coverage below 0.05 — muted publishers whose serving sets
  are entirely dead (condensation singleton sinks) — matching the model's
  claim that e^{−RF(1−μ)} ≫ μ^RF at every μ ≤ 0.2 regime.
- **A sink is exposed only by its own drain.** The sampled-publisher
  publish drain achieved full coverage in **all 8000 runs**, including the
  76 bad graphs: a muted publisher is invisible to any other publisher's
  dissemination. Goodness therefore must come from the strong-connectivity
  pass over the realised graph, not from sampled drains — the framework's
  two-instrument design, and the reason its per-run structural invariant is
  `full_coverage ≥ good` rather than equality.

## 3. Grid-cell cross-check — P(good) at full N (N = 20 000, μ = 0.2, RF = 16)

The two shipped points leave one gap: at the operating point (RF = 24)
P(bad) ≈ 7×10⁻⁵ is unresolvable by either side's Monte Carlo, so the only
P(good) comparison above runs at N = 4 000. This check closes the gap by
re-running a cell the formal N = 20 000 validation grid actually measured
— RF = 16, μ = 0.2, **150 graphs**, matching the grid's own sample size
(`full_coverage.md` §3: P(good) predicted 0.957, measured 0.973).

| source | good / graphs | P(good) |
|---|---|---|
| coverage law (guiding formula) | — | 0.957 |
| formal Monte Carlo (150 graphs) | ≈ 146 / 150 (reported as 0.973) | 0.973 |
| **this framework** | **145 / 150** | **0.9667**, Wilson 95 % [0.9243, 0.9857] |

The estimate lands between the law's prediction and the formal measurement,
with both inside the Wilson interval (z = −0.58 against the law, +0.48
against the formal Monte Carlo) — statistically indistinguishable at
matched sample sizes, in a regime where 150 runs genuinely see failures.
Like the formal measurement, ours sits on the good side of the prediction,
consistent with the law's documented mild conservatism.

The §2 structure repeats at full N: every one of the 5 bad graphs has
exactly one condensation sink with min-publisher-coverage 0.0 (a single
muted publisher), and the sampled-publisher drain achieved full coverage
in all 150 runs. Cost sanity holds too: honest→honest sends mean 204 810
vs the pre-split-horizon expectation H·RF·(H−1)/(N−1) ≈ 204 797.

## 4. Uncertainty methodology — to raise with the formal-methods team

The formal folder reports Monte-Carlo uncertainty as ±1σ standard errors
(σ = √(p̂(1−p̂)/n)); this framework reports raw counts plus a Wilson score
interval at a fixed 95 % level.

**What the Wilson interval is.** Both conventions answer the same question
— given k successes in n trials, what values of the true probability p are
plausible? — but they plug different things into the binomial variance.
The familiar ±zσ band (the Wald interval) evaluates the variance at the
*estimate* p̂ = k/n and centres the interval on it:
p̂ ± z·√(p̂(1−p̂)/n). The Wilson interval instead asks directly which
*candidate* values of p are consistent with the observation — it solves
|p̂ − p| ≤ z·√(p(1−p)/n) for p, with the variance evaluated at the
candidate — and has the closed form

> centre = (p̂ + z²/2n) / (1 + z²/n),  half-width = (z / (1 + z²/n)) · √(p̂(1−p̂)/n + z²/4n²)

with z ≈ 1.96 at the 95 % level.

**Why the difference matters here.** At p̂ ∈ {0, 1} the Wald variance
p̂(1−p̂)/n is exactly zero, so ±zσ collapses to a point: 40 good graphs out
of 40 would read as "P(good) = 1, no uncertainty" — false certainty from a
finite sample. Wilson's variance is evaluated at the candidate p, which is
never exactly 0 or 1, so the interval keeps honest width: 40/40 says
P(good) ∈ [0.912, 1] — i.e. the data rule out P(bad) > 0.088 and nothing
stronger. The Wilson interval also never leaves [0, 1] (Wald bands overrun
the boundary at extreme p̂) and keeps close-to-nominal coverage at small n
and extreme p, which is exactly the corner these experiments live in: a
well-sized configuration is *supposed* to produce all-good samples, so the
boundary case is the common case, not the exception. The conventions map into each other through
the counts — e.g. the bulk point's 76/8000 gives ±1σ ≈ ±0.0011, and the
Wilson interval is ≈ the ±1.96σ band there — so nothing is lost either way
**except at the boundary**: at an all-good sample (this operating point's
40/40; any well-sized configuration's common case) the ±1σ convention
degenerates to zero width and reads as false certainty, while Wilson still
yields a meaningful bound (here P(good) ∈ [0.912, 1]). Since the regimes
these tools are sized for make all-good samples the norm, we suggest the
formal folder's validation tables also carry raw counts (they already do in
places, e.g. the tail ladder's "bad / trials" column) so any interval
convention stays derivable.

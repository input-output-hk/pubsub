# Reproducing the gated M3 downtime estimate

The CIP's M3 gated reference uses N = 20,000, RF = 13, s = 7,
B_relay = B_publisher = 769, an initial adversarial population S = 4,000
(μ = 0.2), and target δ = 10⁻⁴. Its quoted **1.58%** honest downtime is a
rounded baseline prediction, not a measured availability limit.

The calculation uses `m3_isolation` in
[m4_synthesis_predictions.py](m4_synthesis_predictions.py), whose two
terms are the relay hearing failure and the publication failure after
seeding rescue. For each integer number `d` of unavailable honest nodes:

```python
S_effective = 4000 + d
expected_isolated = sum(m3_isolation(20000, 13, 769, S_effective, 7, 769))
p_bad = -math.expm1(-expected_isolated)
honest_downtime_percent = 100 * d / 16000
```

This is the existing shifted-fraction convention: μ_eff = μ + p(1 − μ),
with the effective honest population also reduced inside the law. Both
link kinds use the same bucket count and independent directional gates.
Admission refusals, wholesale flooding and correlated downtime are absent
from this baseline calculation. A deployment with binding admission
budgets needs a separate estimate including those effects.

| Unavailable honest nodes d | Effective S | Honest downtime | Predicted P(bad) | Meets 10⁻⁴ |
|---|---|---|---|---|
| 0 | 4,000 | 0% | 5.84472950 × 10⁻⁵ | yes |
| 252 | 4,252 | 1.575% | 9.98402687 × 10⁻⁵ | yes |
| 253 | 4,253 | 1.58125% | 1.00052888 × 10⁻⁴ | no |

The largest passing integer dropout count is 252. Rounding its 1.575%
to two decimal places with decimal half-up rounding gives **1.58%**.
The displayed percentage must not be fed back into sizing as an exact
safe threshold: the unrounded count and failure estimate govern that.

Run from the repository root:

```sh
python3 -B pubsub-node/docs/experiments/check_cells_against_docs.py
```

The checker recomputes the threshold, reports the adjacent failing count,
and compares the rounded result with both CIP comparison tables. The
regression test pins the 252/253 boundary. This supplies calculation
provenance for the M3 value; it does not establish the correctness of the
coverage model or validate a deployment.

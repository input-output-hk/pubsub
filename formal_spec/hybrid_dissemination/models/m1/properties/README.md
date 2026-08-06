# M1 properties

Per-property analyses of [M1](../README.md).

**Scripts** (all in `../scripts/`): `m1_model.py` (sampler + closed
forms + honest BFS; run it for a self-test), `sim_m1_coverage.py`
(the P(bad) study), `sweep_m1_cost.py` (bandwidth/latency vs F),
`sim_m1_degrees.py` (degree distributions), `sweep_m1_mu_shift.py`
(μ-shift degradation at frozen F), `sim_m1_severity.py` (bad-graph
severity), `sim_m1_eclipse.py` (adaptive eclipse cost).

| Property | File | Verdict |
|---|---|---|
| Full coverage — P(bad graph) | [`full_coverage.md`](full_coverage.md) | HYBRID |
| Expected messages (bandwidth) | [`expected_number_of_messages.md`](expected_number_of_messages.md) | CLOSED FORM |
| Expected hops (latency) | [`expected_number_of_hops.md`](expected_number_of_hops.md) | SIMULATION ONLY |
| Node degrees (links held) | [`node_degrees.md`](node_degrees.md) | CLOSED FORM |
| μ-shift robustness (frozen params) | [`mu_shift_robustness.md`](mu_shift_robustness.md) | HYBRID |
| Adaptive eclipse cost (corruptions) | [`adaptive_eclipse_cost.md`](adaptive_eclipse_cost.md) | CLOSED FORM |

Candidate properties not yet analysed (churn tolerance, join service,
link repair, …): [`candidate_properties.md`](../../candidate_properties.md).

**Headline results** (N = 20 000, μ = 0.2): P(bad) ≈ 1 − e^{−E} with
E = H[e^{−F(1−μ)} + μ^F] (strong connectivity; the seed-proof in-degree-0
class dominates). The smallest fanout with P(bad) ≤ 10⁻⁴ is **F = 24**
(≈ 307 200 transmissions/message, 19.2 / honest node, 5.0 hops).

**Verdict legend** — CLOSED FORM: exact explicit formula; HYBRID: closed-form
law validated by simulation, exact finite-N values need simulation;
SIMULATION ONLY: no closed form.

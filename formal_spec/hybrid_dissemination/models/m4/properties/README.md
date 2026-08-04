# M4 properties

Per-property analyses of [M4](../README.md) (undirected/bidirectional RF-out
gossip).

**Scripts** (all in `../scripts/`): `m4_model.py` (undirected sampler +
honest-BFS + closed forms; run it for a self-test), `sim_m4_coverage.py`
(the P(bad) study), `sweep_m4_cost.py` (bandwidth/latency vs RF),
`sim_m4_degrees.py` (degree distributions), `sweep_m4_mu_shift.py`
(μ-shift degradation at frozen RF), `sim_m4_severity.py` (bad-graph
severity),
`compare_bandwidth.py` and `compare_hops.py` (cross-model runs, used by the
comparison report).

| Property | File | Verdict |
|---|---|---|
| Full coverage — P(bad graph) | [`full_coverage.md`](full_coverage.md) | HYBRID |
| Expected messages (bandwidth) | [`expected_number_of_messages.md`](expected_number_of_messages.md) | CLOSED FORM |
| Expected hops (latency) | [`expected_number_of_hops.md`](expected_number_of_hops.md) | SIMULATION ONLY |
| Node degrees (links held) | [`node_degrees.md`](node_degrees.md) | CLOSED FORM |
| μ-shift robustness (frozen params) | [`mu_shift_robustness.md`](mu_shift_robustness.md) | HYBRID |

Candidate properties not yet analysed (churn tolerance, join service,
link repair, …): [`candidate_properties.md`](../../candidate_properties.md).

**Headline results** (N = 20 000, μ = 0.2): P(bad) ≈ 1 − e^{−E_iso} with
E_iso = H·μ^RF·e^{−RF(1−μ)}; the smallest fanout with P(bad) ≤ 10⁻⁴ is
**RF = 8** (≈ 188 800 transmissions/message, 11.8 / honest node, 5.1 hops).

**Verdict legend** — CLOSED FORM: exact explicit formula; HYBRID: closed-form
law validated by simulation, exact finite-N values need simulation;
SIMULATION ONLY: no closed form.

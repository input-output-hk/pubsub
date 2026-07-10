# M5 properties

Per-property analyses of [M5](../README.md) (directed k_in/k_out gossip).

**Scripts** (all in `../scripts/`): `m5_model.py` (sampler + closed forms +
strong-connectivity check; run it for a self-test), `sim_m5_coverage.py`
(the P(bad) study), `sweep_m5_cost.py` (bandwidth/latency vs (k_in, k_out)),
`sim_m5_degrees.py` (degree distributions).

| Property | File | Verdict |
|---|---|---|
| Full coverage — P(bad graph) | [`full_coverage.md`](full_coverage.md) | HYBRID |
| Expected messages (bandwidth) | [`expected_number_of_messages.md`](expected_number_of_messages.md) | CLOSED FORM |
| Expected hops (latency) | [`expected_number_of_hops.md`](expected_number_of_hops.md) | SIMULATION ONLY |
| Node degrees (links held) | [`node_degrees.md`](node_degrees.md) | CLOSED FORM |
| Churn tolerance (no repair) | [`churn_tolerance.md`](../../churn_tolerance.md) (shared) | pending — expected CLOSED FORM |
| Join service (mid-epoch newcomers) | [`join_service.md`](../../join_service.md) (shared) | pending — structural |
| Link repair (mid-epoch redraws) | [`link_repair.md`](../../link_repair.md) (shared) | pending — expected HYBRID |

**Headline results** (N = 20 000, μ = 0.2): P(bad) ≈ 1 − e^{−E} with two
defect classes, E = H[μ^{k_in}e^{−k_out(1−μ)} + μ^{k_out}e^{−k_in(1−μ)}];
the balanced split k_in ≈ k_out is optimal at every total budget, and the
smallest budget with P(bad) ≤ 10⁻⁴ is **k_in + k_out = 17** ((9,8) or (8,9);
P(bad) ≈ 4.4×10⁻⁵), costing ≈ 217 600 transmissions per message
(13.6 / honest node).

**Verdict legend** — CLOSED FORM: exact explicit formula; HYBRID: closed-form
law validated by simulation, exact finite-N values need simulation;
SIMULATION ONLY: no closed form.

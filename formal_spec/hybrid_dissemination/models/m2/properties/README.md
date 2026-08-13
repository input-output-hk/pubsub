# M2 properties

Per-property analyses of [M2](../README.md) (pull relaying).

**Scripts** (this model's folder, all in `../scripts/`): the shared sampler
`m2_model.py` (run it for a self-test; reused by M3), `sweep_m2_cost.py`
(bandwidth/latency + good-graph-law check), `sim_m2_degrees.py` (degree
distributions), and the eclipse validators
`check_p01_per_target_eclipse.py` (per-target eclipse closed form
P_ecl = C(k,RF)/C(N−1,RF) ≈ μ^RF — the coverage floor) and
`check_p02_adversary_tolerance.py` (k_max(ε) ≈ N·ε^{1/RF}),
`sweep_m2_mu_shift.py` (μ-shift degradation at frozen RF),
`sim_m2_severity.py` (bad-graph severity),
`sweep_m2_reprovision.py` (cheapest RF vs design μ),
`sim_m2_eclipse.py` (adaptive eclipse cost — the same eclipse event
priced as an attacker budget), and `sweep_m2_pfail.py` (per-message
delivery under send loss + retry economics).

| Property | File | Verdict |
|---|---|---|
| Full coverage — P(bad graph) | [`full_coverage.md`](full_coverage.md) | HYBRID |
| Expected messages (bandwidth) | [`expected_number_of_messages.md`](expected_number_of_messages.md) | CLOSED FORM |
| Expected hops (latency) | [`expected_number_of_hops.md`](expected_number_of_hops.md) | SIMULATION ONLY |
| Node degrees (links held) | [`node_degrees.md`](node_degrees.md) | CLOSED FORM |
| μ-shift robustness (frozen params) | [`mu_shift_robustness.md`](mu_shift_robustness.md) | HYBRID |
| Re-provisioning (cheapest RF at design μ) | [`re_provisioning.md`](re_provisioning.md) | HYBRID |
| Adaptive eclipse cost (corruptions) | [`adaptive_eclipse_cost.md`](adaptive_eclipse_cost.md) | HYBRID |
| Transmission unreliability (per-message, p_fail) | [`transmission_unreliability.md`](transmission_unreliability.md) | HYBRID |

Candidate properties not yet analysed (churn tolerance, join service,
link repair, …): [`candidate_properties.md`](../../candidate_properties.md).

**Headline results** (N = 20 000, μ = 0.2): P(bad) ≈ 1 − e^{−H[(1−ρ_f)+u]},
dominated by muted publishers (e^{−RF(1−μ)}, present even at μ = 0); the
smallest fanout with P(bad) ≤ 10⁻⁴ is **RF = 24** (≈ 307 200
transmissions/message, 19.2 / honest node, 4.8 hops).

**Verdict legend** — CLOSED FORM: exact explicit formula; HYBRID: closed-form
law validated by simulation, exact finite-N values need simulation;
SIMULATION ONLY: no closed form.

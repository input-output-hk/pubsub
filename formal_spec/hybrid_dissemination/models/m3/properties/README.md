# M3 properties

Per-property analyses of [M3](../README.md) (pull relaying + standing
initiation links).

**Scripts** (all in `../scripts/`): `m3_model.py` (M3Params/M3Graph + coverage
mean-field + strict good-graph check; reuses the M2 sampler `m2_model.py`;
run it for a self-test), `sim_m3_coverage.py` (the P(bad) study),
`sweep_m3_cost.py` (bandwidth/latency vs RF), `sim_m3_degrees.py` (degree
distributions), `sweep_m3_mu_shift.py` (μ-shift degradation at frozen
(RF, s)), `sim_m3_severity.py` (bad-graph severity + initiation rescue),
`sweep_m3_reprovision.py` (cheapest (RF, s) vs design μ + split economics). Auxiliary:
`check_p03_end_to_end_coverage.py` (mean-field machinery checks),
`sim_p03_full_coverage.py` / `sim_p03_tail.py` (per-message success tables
and deep tail), `sim_p06_depth.py` (depth percentiles).

| Property | File | Verdict |
|---|---|---|
| Full coverage — P(bad graph) | [`full_coverage.md`](full_coverage.md) | HYBRID |
| Expected messages (bandwidth) | [`expected_number_of_messages.md`](expected_number_of_messages.md) | CLOSED FORM |
| Expected hops (latency) | [`expected_number_of_hops.md`](expected_number_of_hops.md) | SIMULATION ONLY |
| Node degrees (links held) | [`node_degrees.md`](node_degrees.md) | CLOSED FORM |
| μ-shift robustness (frozen params) | [`mu_shift_robustness.md`](mu_shift_robustness.md) | HYBRID |
| Re-provisioning (cheapest (RF, s) at design μ) | [`re_provisioning.md`](re_provisioning.md) | HYBRID |

Candidate properties not yet analysed (churn tolerance, join service,
link repair, …): [`candidate_properties.md`](../../candidate_properties.md).

**Headline results** (N = 20 000, μ = 0.2): P(bad) ≈ 1 − e^{−E} with two
defect classes, E = H[μ^RF + μ^{s−1}e^{−RF(1−μ)}] — initiation links attack
exactly the muted-publisher defect, at ~zero bandwidth. Smallest total budget
RF + (s−1) with P(bad) ≤ 10⁻⁴ is **19**; the bandwidth-minimal split is
**(RF = 12, s = 8)** (P(bad) ≈ 7.8×10⁻⁵, ≈ 153 600 transmissions/message,
9.6 / honest node, 5.9 hops).

**Verdict legend** — CLOSED FORM: exact explicit formula; HYBRID: closed-form
law validated by simulation, exact finite-N values need simulation;
SIMULATION ONLY: no closed form.

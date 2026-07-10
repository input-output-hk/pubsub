# Churn tolerance (degradation without repair) — M1–M5

**Status: DEFINED — analysis and validation pending.** Expected verdict:
CLOSED FORM (each model's validated coverage law read at a shifted
adversarial fraction), plus a confirming down-marking simulation.

## 1. Property

How much honest downtime a deployed operating point absorbs when dead links
are **not** repaired within the epoch. An offline honest counterpart is
indistinguishable from a silent adversary, so a per-epoch honest downtime
probability p shifts the effective adversarial fraction to

$$\mu_{\text{eff}} = \mu + p(1-\mu),$$

and each full-coverage law applies at μ_eff. The **churn budget** p_max is
the largest p keeping P(bad) ≤ δ at the deployed parameters. Downtime
relates to the departure rate via p = 1 − e^{−λ_d·T_epoch} — the epoch
length is the master knob.

## 2. Per model

Deployed operating points at N = 20 000, μ = 0.2, δ = 10⁻⁴. Sensitivity is
the log-derivative of the dominant defect term in μ — higher means more
churn-brittle; p_max estimates are first-order, unvalidated:

| model | operating point | dominant defect term | sensitivity | p_max (prelim.) |
|---|---|---|---|---|
| M1 | F = 24 | e^{−F(1−μ)} | ~F = 24 | ~1.8 % |
| M2 | RF = 24 | e^{−RF(1−μ)} | ~RF = 24 | ~1.8 % |
| M3 | (RF, s) = (12, 8) | μ^{RF} | RF/μ = 60 | ~0.5 % |
| M4 | RF = 8 | μ^{RF}·e^{−RF(1−μ)} | RF/μ + RF = 48 | ~1.0 % |
| M5 | (k_in, k_out) = (9, 8) | both, mixed | ≈ k/μ + k′ ≈ 50 | ~2.1 % |

μ-power-dominated points are the most churn-brittle: M3 at (12, 8) is the
worst in the family, but its equal-budget split (13, 7) reads ~2.1 % — churn
is expected to argue for (13, 7), the first property to separate the two
budget-19 Pareto points. M5's ~2.1 % rides on its extra coverage headroom
(4.4×10⁻⁵ vs the 10⁻⁴ target) despite high sensitivity.

## 3. Planned analysis

- exact churn budget p_max at each operating point;
- validation: mark a random p-fraction of honest nodes as down in sampled
  graphs and run each model's good-graph checker against its law at μ_eff.

Out of scope here: correlated churn (region outages, upgrade waves) —
simulation-only, deferred. Mid-epoch repair and newcomers are separate
properties: [`link_repair.md`](link_repair.md),
[`join_service.md`](join_service.md).

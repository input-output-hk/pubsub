# Dissemination models

Candidate designs for the dissemination layer, in the order they evolved.
**One folder per model**: the model description (`README.md`), per-property
analyses (`properties/`), and the executable model + simulators (`scripts/`).

| Model | Mechanism |
|---|---|
| [M1](m1/README.md) | push to F random targets |
| [M2](m2/README.md) | pull (RF forwarders per epoch) |
| [M3](m3/README.md) | pull (RF forwarders per epoch) + s−1 standing initiation links |
| [M4](m4/README.md) | RF uniform picks, bidirectional links, flooding |
| [M5](m5/README.md) | directed: k_in inbound + k_out outbound links, both own picks |

## Current focus

Six properties, analysed per model in each `properties/` folder:

1. **Full coverage** — a sampled graph is **good** iff **every message of
   every honest publisher reaches all other honest nodes**. The guarantee is
   a property of the standing (per-epoch) structure alone: per-message
   randomness cannot promise *every* message, so only links that exist for
   the whole epoch count toward it. For directed models this is strong
   connectivity of the honest propagation structure.
2. **Expected number of messages** — honest→honest transmissions per
   dissemination (bandwidth);
3. **Expected number of hops** — depth until the last / typical honest node
   receives (latency);
4. **Node degrees** — standing links per node (chosen picks + accepted links
   from others), actual in-/out-degree distributions;
5. **μ-shift robustness** — P(bad) degradation at frozen parameters as the
   effective adversarial fraction rises: budget (last μ_eff meeting the
   target) and collapse point (P(bad) = ½); churn reads the curve at
   μ_eff = μ + p(1−μ).
6. **Re-provisioning** — the coverage law inverted at elevated design
   fractions μ_design > 0.2: cheapest parameters, their cost, the
   robustness they carry, and the +1-notch operating points at μ = 0.2;
   cross-model synthesis in [`comparison.md`](comparison.md) §5 (the
   robustness-adjusted frontier).

Candidate properties — the analysis backlog (churn family, security,
economics, lifecycle), short descriptions:
[`candidate_properties.md`](candidate_properties.md).

Cross-model comparison at the standard operating point (N = 20 000, μ = 0.2,
P(bad) ≤ 10⁻⁴): [`comparison.md`](comparison.md).

Validation: every model module runs a self-test when executed directly
(law vs its own simulator); `validate.py` cross-validates the simulators
against each other and against independent algorithms (boundary identities
M5(RF,0) ≡ M2 and M5(0,F) ≡ M1, brute-force and Kosaraju/union-find
cross-checks, a reference dissemination simulator; `--tail {m3,m5}` for
deep-tail law runs).

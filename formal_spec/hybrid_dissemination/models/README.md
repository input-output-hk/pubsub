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

Four properties, analysed per model in each `properties/` folder:

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
   from others), actual in-/out-degree distributions.

Defined, analysis pending (the churn family — one shared file per property,
with per-model tables): [**churn tolerance**](churn_tolerance.md)
(degradation without repair, μ → μ + p(1−μ)),
[**join service**](join_service.md) (what a mid-epoch newcomer gets),
[**link repair**](link_repair.md) (mid-epoch verifiable redraws:
equivalence, exposure, traffic).

Cross-model comparison at the standard operating point (N = 20 000, μ = 0.2,
P(bad) ≤ 10⁻⁴): [`comparison.md`](comparison.md).

Validation: every model module runs a self-test when executed directly
(law vs its own simulator); `validate.py` cross-validates the simulators
against each other and against independent algorithms (boundary identities
M5(RF,0) ≡ M2 and M5(0,F) ≡ M1, brute-force and Kosaraju/union-find
cross-checks, a reference dissemination simulator; `--tail {m3,m5}` for
deep-tail law runs).

# Experiment readiness — sweepable parameters & blockers

**Status:** meeting notes, 2026-07-09. Evidence: `tests/experiment_spike.rs` (branch
`spike-experiment-run`) — N-node publish→measure runs with the default strategies reach 100%
coverage at N = 5/20/50/200 (~0.6 s at 200), driven entirely by the integration-test harness.

## Sweepable today

| Parameter | Meaning | How |
|---|---|---|
| N, topics, membership | network size, topic count, who subscribes where | test code (or config + registry files for the binary) |
| Connection strategy | `connect-to-all` \| `hash-gated` | per-node injection |
| Acceptance strategy | `accept-from-all` \| `bounded` \| `hash-gated` \| `hash-gated-bounded` | per-node injection; mixed pairs legal |
| `target_degree` | RF, the pull fan-out | required by all but the two defaults |
| `genesis` | seed (initial epoch nonce); same seed ⇒ same topology | `Node::new` / `--genesis` |
| `bucket_count` | pinned bucket count B (overrides the derived value) | both seams |
| `cap_buffer` | c in accept cap ⌈RF + c·√RF⌉ (default 3) | bounded acceptance |

**Not configurable yet:** adversary count/placement (k), message rate, per-node heterogeneity
(per-node RF), rotation cadence, golden-tier G/F_g, deposits.

## Blockers (mapped to the experiment-program stages, PR #74)

1. **Experiment framework** — the big one; blocks nothing *conceptually* but everything
   *practically*: topology builder at scale, seed sweeps, metrics (coverage distribution, hop
   depth, per-node failure cause), CSV/aggregation. Stages 1–3 need nothing else.
2. **Determinism** — the harness is tokio/wall-clock; the deterministic sim over the pure
   `NodeState`/`apply` core is not started. Topology is seed-reproducible, runs are not.
3. **Adversary strategies (Stage 2+)** — no hostile bundles exist; even the silent relay
   (forward-to-none) for E3–E5 is missing. Cheap: each is just a strategy instance.
4. **D2 discrepancy** — E7 assumes rejected dialers back-fill; the implementation has **no
   retry/back-fill** (over-capacity rejection just removes the pending edge, ADR 0031). The doc
   or the code must move.
5. **Stage 4+ features** — connection rotation (v1 never fires `Epoch`; heartbeats re-dial the
   same set), golden push tier, provable-misbehaviour/slashing machinery.
6. **Hop-depth metric (E2)** — deliveries are recorded without hop counts; needs a message
   annotation or delivery-time reconstruction in the framework.

**Bottom line:** Stage 1–2 experiments are days away, not weeks — missing pieces are the
sweep/metrics wrapper, one silent-adversary strategy, and a decision on D2.

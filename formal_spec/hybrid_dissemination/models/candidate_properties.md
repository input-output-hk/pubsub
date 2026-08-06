# Candidate properties — backlog

Properties considered but not yet analysed (analysed ones live in each
model's `properties/` folder). Short descriptions only; each becomes a
per-model analysis when picked up.

## Churn family

- **Churn tolerance (no repair)** — the churn reading of the analysed
  μ-shift curves: per-epoch honest downtime p shifts μ_eff = μ + p(1−μ),
  with p = 1 − e^{−λ_d·T_epoch} (epoch length is the master knob).
  Remaining work: down-marking validation and the H_eff = H(1−p)
  correction (offline nodes also leave the coverage requirement).
- **Join service (mid-epoch newcomers)** — a joiner draws its chosen
  links immediately but appears in nobody's draws until the next epoch.
  Expected profile: an M1 joiner publishes but is deaf, M2 receives but
  is mute, M3/M4/M5 fully functional. To quantify: time to full service,
  degraded-newcomer fraction at churn equilibrium.
- **Link repair (mid-epoch redraws)** — chooser-side redraws from the
  verifiable draw sequence; fresh-sample equivalence (the coverage law
  holds at N(t), μ(t)), residual exposure during repair latency, repair
  traffic = churn rate × chosen degree. Blocked on two design inputs:
  visible vs silent departures; mid-epoch redraw budget semantics.

## Security / robustness

- **Local defect detectability** — which δ-events the victim can
  self-diagnose: accepted-side-zero is visible at epoch start (an M4
  isolated node, an M2 requester-less publisher), chosen-side-dead is
  not (silent adversaries accept connections and say nothing —
  detectable only from traffic silence). Drives link-repair triggers.
- **Rational silence (free-riding)** — a node that holds links but does
  not relay ≡ silent adversary, so the guarantee is incentive-robust
  with μ read as attackers + free-riders + unrepaired churn sharing one
  budget. One-paragraph reinterpretation, folds into μ-shift.
- **Attribution surface** — which deviations are provable (opening more
  links than the verifiable draw budget) vs unprovable (not relaying);
  a taxonomy of punishable vs merely tolerated misbehaviour.
- **Transmission unreliability (p_fail)** — per-transmission loss breaks
  the standing-structure argument; effective branching ×(1−p_fail),
  per-message delivery probability, retransmission policy. M4's doubled
  edge redundancy should win this axis.
- **Adversarial flooding / emergency delivery under load** — the loud
  dual of the silent adversary: coverage implicitly assumes offered
  load ≤ capacity, and adversaries control offered load. Valid messages
  amplify H·c-fold — 4 000 adversaries injecting 1 KB/s each ≈ 300 Mb/s
  steady ingress at every honest node (M3) — and the clog hits the
  max-out-degree node first. Analyzable defense stack: per-origin rate
  caps at every relay (bound adversarial load to a μ-share), priority
  classes gated by scarce publishing rights (bound emergency delay to
  one head-of-line serialization per hop), receiver-driven fetch
  (announce-then-fetch hands scheduling to the victim). Queue drops
  would convert the latency attack into a coverage attack → stratify
  guarantees by class. Target theorem: with all three defenses,
  emergency delivery ≤ hops × (RTT + HOL) under arbitrary valid flood;
  absent any one, delay is unbounded at negligible attacker cost.
  Couples tightly to sustained load & hotspots and eager/lazy economics.
  *Status: deferred (July 2026 team sync) — the publisher set is expected
  to be small and trusted for now.*

## Performance / economics

- **Eager/lazy economics (announce-then-fetch)** — duplicates become
  ~100 B announcements + one body fetch per node; the bandwidth axis
  compresses for large messages and the state axis dominates — the one
  property that could flip the M3/M4 frontier. Output: per-model traffic
  and the message-size crossover.
- **Re-provisioning comparison** — cheapest parameters at μ_design > 0.2
  and the +1-notch operating points (M3 (13, 7), M4 RF = 9): what
  robustness costs; the robustness-adjusted frontier.
- **Sustained load & hotspots** — per-node egress under a publication
  rate, the busiest node's provisioning number (balls-in-bins tail ×
  traffic), spam amplification factor.
- **Latency percentiles / wall-clock** — per-node depth percentiles and
  first-passage times under a WAN RTT distribution → the effective
  synchrony bound Δ that consensus timing needs.
- **Bandwidth lower bound** — the eclipse floor forces
  ≥ ln(2H/δ)/ln(1/μ) honest in-edges per node (≈ 9.6 copies at the
  operating point); M3 sits exactly on it → an optimality theorem for
  the standing uniform-pick protocol class.
- **Per-target guarantee** — a single node's ε (marginal defect
  probability ≈ E/H) rather than the network-wide δ; the number a node
  operator quotes.
- **SLO arithmetic** — P(bad) × severity × epochs/year → expected
  node-epochs lost per year and per-node availability; the guarantee in
  operator units.

## Scale / lifecycle

- **Epoch transition** — messages in flight at rotation, overlap
  semantics (transient 2× state), whether a message can fall between
  consecutive graphs.
- **Epoch-length synthesis** — the T_epoch window jointly satisfying
  churn budget, join wait (T/2), repair traffic, setup amortisation and
  the grinding window; needs the churn family completed.
- **Admission caps** — a bounded accepted side turns rejected picks into
  dead links: another μ-shift source, correlated with popular targets.
- **Multi-topic scaling** — per-topic graphs multiply per-node state
  (M4's 16 vs M3's 38 per topic) vs shared-substrate designs.
- **Heterogeneity / correlated failure** — non-uniform stake, bandwidth,
  geography; a region outage is the μ-shift curve at large p (stress
  scenario, not a budget).
- **Setup overhead** — per-epoch handshakes + proof-of-selection bytes
  and verification CPU × degree; the state axis in per-epoch cost terms.

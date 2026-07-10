# Join service (mid-epoch newcomers) — M1–M5

**Status: DEFINED — analysis and validation pending.** Expected verdict:
CLOSED FORM / structural.

## 1. Property

What service a node joining mid-epoch receives, and its time to full
service. A joiner draws its own (**chosen**) links immediately — verifiable
selection needs no coordination — but it appears in nobody's existing
draws, so its **accepted** side stays empty until the next epoch's draws.

## 2. Per model

The chosen side sets the newcomer's immediate service:

| model | chosen links | immediate service |
|---|---|---|
| M1 | F push targets (out) | publishes; **deaf** for the residual epoch |
| M2 | RF forwarders (in) | receives; **muted publisher** for the residual epoch |
| M3 | RF forwarders (in) + s−1 initiation (out) | **fully functional** |
| M4 | RF bidirectional | **fully functional** |
| M5 | k_in inbound + k_out outbound | **fully functional** |

In every model the accepted side — redundancy, and relay usefulness — fills
in at the next epoch (mean residual wait T_epoch/2).

## 3. Planned analysis

To quantify: time-to-full-service distribution, and the fraction of the
network in the degraded-newcomer state at churn equilibrium (join rate ×
mean residual epoch); confirmation by a small epoch-boundary simulation.
Related properties: [`churn_tolerance.md`](churn_tolerance.md),
[`link_repair.md`](link_repair.md).

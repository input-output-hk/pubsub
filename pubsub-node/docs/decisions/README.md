# Architecture decision records

Immutable audit trail (Constitution, Principle III): decisions are never
rewritten — a change of course appends a new ADR and a forward-pointing status
banner on the ADR it amends. **Read the status line first**; the body records
what was decided *at the time*, superseded parts included.

| ADR | Decision | Status |
|---|---|---|
| [0001](0001-async-runtime-tokio.md)–[0007](0007-network-handle-actor-pattern.md) | Foundations: tokio, serde/TOML, tracing, clap, typed errors, receive task, network-handle actor | Active |
| [0008](0008-subscription-mutator-shape.md) | Subscription mutator shape | Superseded in part by 0015 |
| [0009](0009-crypto-trait-shape.md)–[0010](0010-protocol-message-type-hierarchy.md) | Crypto trait shape; message type hierarchy | Active |
| [0011](0011-pure-state-transition-core.md)–[0012](0012-node-state-sharing-and-lifecycle.md) | Pure state-transition core; state sharing & lifecycle | Active |
| [0013](0013-subscription-list-is-authoritative-for-node-interests.md)–[0016](0016-topic-registry-interface-and-node-integration.md) | Registries: authority, interfaces, node integration | Active (0014/0016 amended by 0020) |
| [0017](0017-key-backed-peer-identity-and-signed-connection-control.md) | Key-backed identity; signed connection control | Amended by 0032 (link store; role on the wire) |
| [0018](0018-connection-selection-strategy-seam.md) | Connection-selection seam | Amended by 0034 (one selection family) |
| [0019](0019-graceful-shutdown-lifecycle.md) | Graceful shutdown | Active |
| [0020](0020-cross-registry-consistency-and-readiness.md) | Cross-registry consistency; readiness | Active |
| [0021](0021-fanout-strategy-seam-dedup-and-message-origin.md) | Fan-out seam; dedup; `Origin` | Amended by 0033/0034/0036 (origin-aware kinds over the flow store) |
| [0022](0022-async-test-synchronization-strategy.md) | Async test synchronisation | Active |
| [0023](0023-connection-acceptance-strategy-seam.md) | Acceptance seam | Amended by 0033/0034 (role slots) |
| [0024](0024-verifiable-hash-gated-selection.md) | Verifiable hash-gated selection | Amended by 0033/0034/0035 (role domains; merged struct; symmetric mode) |
| [0025](0025-acceptance-seam-and-rejected-action.md) | `Admission`; explicit `Rejected` | Amended by 0032/0033 (role on the wire; role-scoped scan) |
| [0028](0028-strategy-self-construction.md) | Two-phase strategy construction | Active (extended by 0034) |
| [0029](0029-strategies-module-layout.md) | `strategies/` module layout | Amended by 0034 (`selection/`; fanout kinds) |
| [0030](0030-heartbeat-interval-and-edge-predicate.md) | Heartbeat interval; shared edge predicate | Amended by 0031; extended by 0033/0035 |
| [0031](0031-epoch-round-split-and-acceptance-decomposition.md) | Epoch/round split; readiness gate; acceptance baselines | Amended by 0033/0034 (publish pass; role instantiation) |
| [0032](0032-unified-link-store-and-role-handshake.md) | Unified link store; role-carrying handshake | Active; §1/§3 reshaped by 0036 (API stable) |
| [0033](0033-publishing-link-seams.md) | Publishing-link seams | §1–2 superseded by 0034; §5 default made configurable by 0035 |
| [0034](0034-model-family-seams.md) | Model-family seams (M3/M4/M5 as config) | Active; labelling amended in-review; §3 reshaped by 0036 |
| [0035](0035-symmetric-edges-and-publish-in-admission.md) | Symmetric edges; publish-in admission | Active |
| [0036](0036-flow-oriented-link-store.md) | Flow-oriented link store; test-stability rule | Active |

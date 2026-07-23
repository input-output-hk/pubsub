# 0033 — Remove the config bootstrap peer list (and the node-config file)

**Status**: Accepted (follow-up to feature 015's review; closes N-007)

**Context**: the TOML node-config file and its `peers` list date from the 001
scaffold, when nodes connected to a statically configured set. Feature 008
made the subscription registry the source of truth for membership and derived
per-topic `candidates`; 004-connections/005 made link establishment
strategy-driven over those candidates. Since then the config `peers` list has
fed **no behaviour**: it was parsed, stored on the shell, and exposed via
`Node::peers()`, whose only consumer was a test asserting the list round-trips.
N-007 kept it for a "future dialer" that was superseded by the registry-derived
candidate set. The project is pre-release with no deployed users, so no
compatibility shim is owed.

## Decision

Remove the subsystem outright:

- `NodeConfig` / `PeerEntry` / `load_node_config` (the whole `config` module)
  and the `--config` CLI flag — a node now takes only the registry files.
- The `peers` field and `peers()` getter on `Node`.
- `PeerDescriptor` / `BasicPeerDescriptor` (the descriptor abstraction existed
  only to carry the config entries).
- The config-loading test binary and the bootstrap-passthrough test.

`ConfigError` stays: the in-memory registries' `from_file` loaders share it.

## Consequences

- Every node invocation (and future experiment-harness sweep script) drops a
  mandatory-but-meaningless `--config` file.
- The test node builders lose their dead `peers` parameters; `NodeSpec` loses
  its `peers()` setter.
- If a bootstrap/dial-by-address list is ever needed (a real transport
  without a registry), it returns as part of that feature's design — not as a
  parsed-and-ignored config field.

## Alternatives rejected

- **Keep as deprecated** — pre-release; deprecation ceremony for zero
  consumers is noise.
- **Repurpose for the discovery layer** — the discovery/view-sampling feature
  (`H_v`) samples the registry-derived candidate set; a static address list is
  a different concern belonging to a real-transport feature.

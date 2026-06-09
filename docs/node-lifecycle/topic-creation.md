# Topic creation and registry management

Topic creation, role-based access control (owners, admins, publishers), and the full topic-registry contract lifecycle are formally specified in Quint at [`formal_spec/topic_registry/`](../../formal_spec/topic_registry/). The spec covers ten operations, an authorisation matrix, fifteen invariants, and one temporal liveness property.

The node-lifecycle procedures in this directory consume the topic registry as a read-only on-chain artifact (see [README](./README.md#on-chain-artifacts)). Operations that *modify* the topic registry — creating a topic, registering or rotating an authorised publisher key, granting or revoking roles — are the topic-registry contract's domain and are not duplicated here.

See [`formal_spec/topic_registry/README.md`](../../formal_spec/topic_registry/README.md) for running the spec under Quint or TLC.

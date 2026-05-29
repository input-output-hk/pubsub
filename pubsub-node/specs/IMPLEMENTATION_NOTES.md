# pubsub-node — implementation notes to revisit

**Purpose**: a running list of implementation questions that surfaced during pre-spec discussion of a feature but were deemed out of scope for that feature. Each entry records the question, the working answer (if any), and the trigger condition for revisiting.

Workstream-level (not feature-scoped). Sibling to `ROADMAP.md`. Migrated into a feature's spec when the trigger condition fires.

---

## N-001 — Local emission vs local receipt in `received_messages()`

**Surfaced during**: 002 (topic-subscription filtering) pre-spec discussion.

**Question**: when a Node's own send / emission path is invoked to publish a message `M`, does `M` appear in that Node's local `received_messages()` snapshot? I.e., is local emission also a local receipt?

**Working answer (002 scope)**: **No.** A Node does not see its own published messages in `received_messages()` unless a peer forwards them back. The snapshot is strictly inbound-from-the-network.

**Why deferred**: this question only becomes operationally interesting when there's an external admin / REST API driving the Node — an operator publishing through such an API would plausibly want a confirmation that "the message was accepted into the local view". 002 has no such API surface; the send path is invoked from within the same process, so a separate confirmation snapshot adds clutter without value.

**Trigger to revisit**: when a Node-facing REST / admin API is introduced. Until then the "strictly inbound" snapshot semantics hold.

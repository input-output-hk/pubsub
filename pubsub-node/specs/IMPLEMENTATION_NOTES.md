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

---

## N-002 — Self-addressing semantics under connection-based communication

**Surfaced during**: 002 (topic-subscription filtering) /speckit-clarify Round 2 review.

**Question**: when a Node emits a message addressed to its own peer id, does the message reach the Node's own receive path? In 002 (in-memory, registry-routed), the answer is yes — the InMemoryNetwork's loopback routes the message through, subscription filtering applies, and a subscribed Node observes the message in `received_messages()`. The spec records this as an Edge Case bullet for 002.

**Working answer (002 scope)**: Self-addressing is a legitimate inbound delivery via the network's loopback. FR-005 ("network unchanged"), FR-009's "deliveries arriving through the receive path are valid receipts" carve-out, and the absence of any `from == to` short-circuit in `src/network.rs:50–72` together imply this behavior.

**Why deferred**: in connection-based transports (TCP in feature 009; the connection-oriented model in feature 004), "a connection to self" is operationally a different beast. Some transports refuse the self-connect; others permit it but it loops through OS networking; some applications model it as a no-op. Whichever model emerges, the self-addressing semantics defined here for the in-memory pipe may not survive unchanged.

**Trigger to revisit**: when feature 004 (connection-oriented network model) lands. The connection-lifecycle ADR for that feature should explicitly address self-connections; the receive-path filter behavior may need to be re-examined alongside it.

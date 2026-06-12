# Feature Specification: Logical Connection Management with Autonomous Static Topology

**Feature Branch**: `004-connections`

**Created**: 2026-06-11

**Status**: Draft

**Input**: User description:

> Logical connection management with autonomous static topology (feature directory MUST be `specs/004-connections/` — the follow-on under the 004 umbrella per project convention; do not auto-number). Builds on the merged 004 event-loop refactor (pure `apply` → `Vec<Effect>`, single event queue, ADRs 0011/0012) and the merged 008 subscription registry (`Event::MembershipUpdate`, registry-derived `subscriptions`, `candidates` per topic with self excluded, ADRs 0013/0014).
>
> Nodes gain a notion of logical connections with peers, one connection per (peer, topic) pair, organized by role: upstream connections are those this node requested (its message sources, per the ROADMAP §1.2 direction inversion) and live in node state as a map from (peer, topic) to an explicit state, AwaitingAccept or Active; downstream connections are those this node accepted (its fan-out destinations) and live in node state as a set of (peer, topic) pairs — terminal outcomes are removals, not stored states. The candidates set remains as-is; upstream/downstream are new fields alongside it. The transport stays exactly as today — a dumb peer-id-routed pipe with the single network mailbox producer; connections are purely logical, established and torn down by application-level control messages (Request, Accepted, Terminated — no Rejected in this feature) carried as a new signed sibling of the existing signed/plain message split: the emitting node signs control messages (nodes therefore gain a signing identity at construction, mock crypto), and all validation including control-message signature checks happens inside the pure transition, which already owns the verifier.
>
> Establishment is autonomous and runs exactly once: at construction the node arms a one-shot connection-setup timer (configurable duration with default; it exists to let registry sync converge); on expiry an event enters the queue and the transition consults an injected, encapsulated connection-selection strategy (later instantiable from config) that returns the expected upstream set — the initial policy is connect-to-all-candidates across the node's topics; the transition diffs expected against current, updates state, and returns effects to send the requests. The strategy consumes the candidate set as of timer expiry: partial or empty convergence yields a partial or empty static topology (operator tunes the duration; consistent with 008's no-fail-fast posture). Incoming requests are accepted unconditionally (no deny path in this feature) and idempotently: a request matching an existing downstream entry re-confirms it and re-sends Accepted (this covers peers re-dialing after an abrupt restart; revisit when a richer accept policy lands). The resulting topology is a full bidirectional per-topic graph among nodes sharing a topic, static thereafter — static means requests are generated exactly once. Unsolicited or unknown control messages (an Accepted with no matching AwaitingAccept, a Terminated for a connection not held) are dropped cause-tagged with no state change — upstream entries originate only from the node's own strategy, downstream entries only from peers' requests. Control messages whose sender is the node itself are dropped (self-connections are unrepresentable: never dialed since candidates excludes self, never accepted); no cross-check between routing-frame identity and signed emitter is performed (the real transport's frame data is unknown).
>
> The receive path changes behavior: a signed message is admitted into received messages only if its sender is an Active upstream for its topic — otherwise it is dropped with a new cause under the existing message_dropped convention. An invalid-signature message arriving over an Active upstream is misbehavior: the node silently terminates that connection — removes the upstream entry and raises a semantic misbehavior effect that the shell only logs in this feature (no Terminated message is sent; silent termination is the non-cooperative path) — and subsequent messages from that peer are dropped as no-longer-connected. Misbehavior is signature-only in this feature: topic mismatches are plain drops because they are innocently reachable while own-topic-change reconciliation is deferred. Sending is otherwise unchanged (send resolves once the network accepts; enforcement is receiver-side only).
>
> Graceful teardown: a new consuming async shutdown method pushes a shutdown event; the transition clears both connection sets and returns one Terminated-notice effect per live counterpart; the event doubles as the event loop's terminal marker, so awaiting the loop's completion guarantees the notices were sent before drop runs; plain drop without shutdown remains the abrupt no-notice path. Peers receiving Terminated remove the matching entry. There is no manual connect/disconnect API — the node's connection behavior is fully autonomous; the operator/library verbs are construction, send, shutdown, and read-only snapshot getters for both connection sets.
>
> This feature is deliberately not parity-preserving: pre-connection delivery semantics are retired, and the existing integration suites are reworked (not preserved) with an establishment preamble through the real path — script the registry, trigger setup, await Active — using declarative test builders per constitution v1.2.0; all other receive-path behavior (subscription filter, signature check, drop-event convention, recording) is unchanged once a connection exists and is re-asserted as the regression boundary. Resolves deferred notes N-002 (self-addressing, as above) and N-006 (construction-failure integration test, extended to construction's new signer/timer parameters). Out of scope, each recorded as a spec line or deferred note: fan-out/forwarding over downstream (006), dynamic connection transitions including re-selection, reconnection, stale-AwaitingAccept garbage collection, and a rejection/deny path (deferred package; explicit rejection messages return when a deny path exists), blacklisting, alternative selection policies from config, golden-mode toggles (007), transport-level connections and multiplexing (009+, supersedes the event-loop contract §1.3 sketch), Active-connection liveness/heartbeat (009), handshake identity-binding hardening (real crypto), backpressure (unchanged, queue stays unbounded), DDoS resistance (architecture docs), hardcoded-connections config affordance (no consumer yet). Effect execution errors are logged only.

## Clarifications

### Session 2026-06-11

- Q: Where does the misbehavior trigger sit in the receive-path check order? → A: Connection check first, then the pre-existing order preserved unchanged (subscription filter, then signature verification); misbehavior severance fires only for a message that passed all earlier checks and fails signature verification. An invalid-signature message on a no-longer-subscribed topic over an Active connection is a plain topic-not-subscribed drop, never a severance.
- Q: Which connections receive a Terminated notice at graceful shutdown? → A: Every entry in both structures regardless of state — Active and AwaitingAccept upstreams plus all downstreams. The counterpart of a pending entry may already hold matching state, and a redundant notice is harmlessly absorbed by the unknown-Terminated drop rule.
- Q: Is the one-sided connection after an acceptor's abrupt restart accepted v1 state? → A: Yes — when an acceptor restarts abruptly, the survivor's Active upstream toward it goes permanently quiet (the restarted node lost its downstream entry and once-only establishment never recreates it). Accepted, documented stale state; healed only when dynamic transitions/liveness land. No healing at re-dial.
- Q: Is Request acceptance gated only on control-message validity, or also on topic membership? → A: Membership-validated — "accepted unconditionally" in the input description meant no acceptance *policy*, not no validation. After the control-message checks, the receiver accepts a Request iff the topic is among its own topics AND the requester is a member of that topic in the receiver's current view; a failing Request is silently dropped (cause-tagged, no state change, no reply — there is no Rejected message). This membership gate is what makes the converged topology a full bidirectional graph *per topic*. (Note: the verbatim Input section above retains the original "accepted unconditionally" wording as the historical record; this entry is the correction.)
- Convergence note: four clarify rounds were run before planning, with a strictly decreasing finding profile — 3 decisions, 1 correction (the membership-validation entry above), 0, 0. Rounds 3–4 produced only integration-completion and terminology alignments (duplicate-request edge case deferring to FR-012; accepter → acceptor). The spec is judged converged; remaining verification is cross-artifact and belongs to `/speckit-analyze` after plan and tasks exist.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A node autonomously builds its per-topic connection topology (Priority: P1)

A node is constructed against a network and a subscription registry. Without any operator action, it waits its configured setup delay (allowing its registry view to converge), then requests a connection to every known member of every topic it is registered for. Each contacted member accepts. Nodes that share a topic end up mutually connected: each holds the other as an upstream message source (it requested) and as a downstream fan-out destination (it accepted).

**Why this priority**: This is the feature's core deliverable — the connection topology that every later dissemination feature (fan-out, dialer policies, golden mode) operates on. Without it nothing else in the feature is observable.

**Independent Test**: Construct several nodes sharing a topic against one network and one registry; after their setup delays elapse, observe via the connection getters that every pair sharing the topic holds an Active upstream entry and a downstream entry for each other, with no further connection activity afterwards.

**Acceptance Scenarios**:

1. **Given** N nodes registered in the registry for a common topic, **When** every node's setup delay elapses and the resulting requests and acceptances are processed, **Then** every node holds an Active upstream entry for each of the other N−1 nodes for that topic and a downstream entry for each of the other N−1 nodes for that topic (a full bidirectional per-topic graph).
2. **Given** a node registered for two topics with different member sets, **When** its setup delay elapses, **Then** it issues one connection request per (member, topic) pair across both topics, and its connection entries are keyed per (peer, topic) — a peer sharing both topics yields two independent connections.
3. **Given** a node whose registry view at timer expiry knows only a subset of a topic's members (partial convergence), **When** the setup event is processed, **Then** it connects to exactly that subset and issues no further requests when later membership updates arrive (static, once-only establishment).
4. **Given** a node whose registry view is empty at timer expiry, **When** the setup event is processed, **Then** it issues no requests and remains a node with no upstream connections; it still accepts incoming requests from others.
5. **Given** any node, **When** another node's connection request arrives for a topic that is among the receiver's own topics and from a requester the receiver knows as a member of that topic, **Then** it is accepted: a downstream entry is recorded and an acceptance is sent back.
6. **Given** a node that issued a request, **When** the acceptance arrives, **Then** the matching upstream entry transitions from AwaitingAccept to Active; messages from that source are admissible from that point on.
7. **Given** any node, **When** a connection request arrives for a topic that is not among the receiver's own topics, or from a requester the receiver does not know as a member of that topic, **Then** it is dropped with a cause-tagged event: no downstream entry is created, no reply is sent, and the requester remains pending.

---

### User Story 2 - Message delivery is gated on an established connection (Priority: P1)

A subscriber only records messages that arrive from sources it deliberately connected to. A publisher-style message arriving from a peer that is not an Active upstream source for that topic is not recorded, and the drop is observable with a distinct cause. Once a connection is Active, all previously specified receive behavior (topic subscription filter, signature verification, recording of deliveries) applies unchanged.

**Why this priority**: This is the behavioral payoff of having connections at all — the receive path now enforces the topology. It is also the deliberate compatibility break this feature charters: delivery without a connection is retired.

**Independent Test**: With two connected nodes and one unconnected sender, send the same valid signed message from both; only the connected source's message enters the recipient's received messages, and the unconnected source's message surfaces as a cause-tagged drop.

**Acceptance Scenarios**:

1. **Given** an Active upstream connection from B for topic T, **When** B sends a validly signed message on T, **Then** the recipient records the delivery exactly as specified by the pre-existing receive behavior.
2. **Given** no connection (or a still-AwaitingAccept connection) from B for topic T, **When** B sends a validly signed message on T, **Then** the message is not recorded and a drop event with a distinct cause is emitted.
3. **Given** an Active upstream connection from B for topic T, **When** B sends a message for a different topic U with no Active connection, **Then** the message is not recorded (connections are per-topic; T's connection does not admit U's traffic).
4. **Given** an Active upstream connection and an admitted message, **When** the existing subscription filter or signature verification would have dropped it under prior features, **Then** it is still dropped with the same cause as before (post-connection behavior is the regression boundary).

---

### User Story 3 - Misbehavior over an established connection severs it silently (Priority: P2)

A node that receives an invalid-signature message over an Active upstream connection treats the sender as misbehaving: it removes that connection immediately and unilaterally — no notice is sent to the misbehaving peer — and surfaces the fact as an observable misbehavior signal. From that moment, all further messages from that peer on that topic are dropped as not-connected, even if validly signed.

**Why this priority**: This is the protocol's first containment behavior and the user's stated test flow for connection termination. It depends on US1/US2 being in place.

**Independent Test**: Establish a connection, deliver one invalid-signature message, observe the upstream entry disappear and a misbehavior signal raised; deliver a subsequent validly signed message from the same peer and observe it dropped as not-connected.

**Acceptance Scenarios**:

1. **Given** an Active upstream connection from B for topic T, **When** a message from B on T fails signature verification, **Then** the upstream entry for (B, T) is removed, a misbehavior signal is raised (logged by the node runtime in this feature), and no termination notice is sent to B.
2. **Given** the connection from B for T was severed for misbehavior, **When** B sends a subsequent validly signed message on T, **Then** it is dropped with the not-connected cause and is not recorded.
3. **Given** an invalid-signature message from a peer with no Active connection for its topic, **When** it arrives, **Then** it is dropped as not-connected (there is no connection to sever; a forged sender identity must not cost the genuine peer anything).
4. **Given** a message whose topic is not in the recipient's subscription set or has no connection, **When** it arrives, **Then** it is a plain cause-tagged drop, never a termination (misbehavior is signature-only in this feature; topic mismatches are innocently reachable while topic-change reconciliation is deferred).

---

### User Story 4 - Graceful shutdown notifies counterparts; abrupt loss is recoverable on restart (Priority: P2)

An operator (or owning program) shuts a node down gracefully: the node sends a termination notice for every connection entry in both roles — including requests still awaiting an answer — and only completes shutdown after those notices are on their way. Counterparts receiving a notice remove the matching entry. If a node instead disappears abruptly (no notices), its counterparts keep stale entries — and when the node restarts and re-requests the same connections, the counterparts re-confirm idempotently, so the restarted node converges back to Active without operator action.

**Why this priority**: Teardown semantics complete the lifecycle and make multi-node tests hermetic, but the topology is useful without them.

**Independent Test**: Connect two nodes; gracefully shut one down and observe the survivor's matching entries removed. Separately, abruptly drop one node, restart it under the same identity, and observe its re-requests re-confirmed and back to Active.

**Acceptance Scenarios**:

1. **Given** a node with upstream entries (Active or AwaitingAccept) and downstream entries, **When** its shutdown operation is invoked, **Then** one termination notice per entry in both structures is sent regardless of state, the connection state is cleared, and the operation completes only after the notices have been handed to the network.
2. **Given** a counterpart receiving a termination notice for a connection it holds (either role), **When** the notice is processed, **Then** the matching entry is removed and no reply is sent.
3. **Given** a node dropped abruptly (no shutdown call), **When** nothing else happens, **Then** counterparts retain their entries (stale, harmless: they admit no messages by themselves).
4. **Given** a counterpart holding a downstream entry for a peer that restarted, **When** the restarted peer's new connection request for the same (peer, topic) arrives, **Then** the entry is kept as-is and an acceptance is re-sent (idempotent re-accept), letting the restarted peer reach Active.

---

### User Story 5 - Lifecycle is observable and deterministically testable (Priority: P3)

A library consumer or test inspects a node's connection state through read-only snapshot getters for both roles, including diagnostic visibility of requests that were never answered (stuck at AwaitingAccept because the target is absent from the network). The whole connection state machine is exercisable synchronously by feeding events to the transition — no timing dependencies — with multi-step scenarios expressed as declarative event scripts per the project's test-construction standard.

**Why this priority**: Observability and sync-testability are what make the previous stories verifiable and keep the feature within the project's testing discipline; they add no node behavior of their own.

**Independent Test**: Drive a node's transition through a scripted lifecycle (setup → request → accept → misbehave → shutdown) in a synchronous test and assert each intermediate state via the same snapshots the public getters expose.

**Acceptance Scenarios**:

1. **Given** any point in a node's life, **When** the connection getters are read, **Then** they return consistent snapshots of the upstream map (with per-entry state) and the downstream set without disturbing the node.
2. **Given** a request sent to a registry member absent from the network, **When** no answer ever arrives, **Then** the upstream entry remains visible at AwaitingAccept indefinitely (accepted v1 state; never admits messages; documented diagnostic).
3. **Given** the connection state machine, **When** exercised in synchronous transition tests, **Then** every lifecycle transition in this specification is reachable by feeding events alone (timer expiry is itself representable as an event), with no real timers required.

---

### Edge Cases

- **Setup timer fires before registry convergence**: defined behavior — the strategy consumes the candidate set as of timer expiry; partial or empty views yield a partial or empty static topology. The operator tunes the delay; tests script convergence before triggering setup.
- **Acceptance with no matching pending request** (unsolicited): dropped with a cause-tagged event, no state change. An unsolicited acceptance must never create an upstream entry — upstream entries originate only from the node's own strategy.
- **Termination notice for a connection not held**: innocently reachable (e.g., crossing a removal); dropped with a cause-tagged event, no state change.
- **Duplicate connection request** (peer re-dialing after abrupt restart): idempotent re-accept, subject to the same membership validation as any Request (FR-012) — keep the existing downstream entry, re-send acceptance. If the re-dialing peer no longer passes validation (e.g., removed from the registry in the interim), the Request is dropped and the existing entry is left as-is (stale-but-harmless, like every other unmaintained entry). Revisit when a richer acceptance policy exists.
- **Control message whose sender is the node itself**: dropped, no state change. Combined with the strategy never selecting self (the candidate set excludes self), self-connections are unrepresentable end to end — this resolves deferred note N-002. No cross-check between transport-frame identity and signed emitter is performed (the real transport's frame data is not yet known).
- **Control message with an invalid signature**: dropped with a cause-tagged event, no state change (misbehavior termination is defined only for payload messages over an Active upstream).
- **Request targeting a peer present in the registry but absent from the network**: the network silently drops the send (existing behavior); the requester's entry stays at AwaitingAccept indefinitely — accepted, observable, harmless.
- **Request arriving before the receiver's view has converged** (the receiver does not yet know the requester as a member, or does not yet know its own topic): membership validation fails and the Request is silently dropped — the requester stays at AwaitingAccept indefinitely (no Rejected message, no retry under once-only establishment). In healthy deployments the setup delay makes this unreachable: every node's view converges before any timer fires, so requests arrive after both ends know each other. Tests script registry convergence on both ends before triggering setup.
- **Both directions between the same pair**: A↔B may simultaneously hold, for the same topic, an upstream and a downstream connection each (A requested from B, and B requested from A). These are independent connections; the role-split state makes the coexistence structural.
- **Shutdown with queued events ahead of it**: events already queued are processed before the shutdown event; the node quiesces in order.
- **Abrupt drop without shutdown**: no notices are sent; this remains the no-guarantees teardown path.
- **Acceptor's abrupt restart (one-sided connection)**: A holds Active upstream (B, T); B restarts abruptly, losing its downstream entry toward A. B's re-run setup heals B's own sources but nothing recreates B's downstream toward A — A's Active upstream goes permanently quiet. Accepted v1 state: stale entries only admit traffic, never create it; healed when dynamic transitions/liveness land. No healing at re-dial (it would add a connection-creation trigger beyond once-only establishment).

## Requirements *(mandatory)*

### Functional Requirements

**Connection model and state**

- **FR-001**: The node MUST maintain **upstream connections** — those it requested, serving as its message sources — keyed by (peer, topic), each in exactly one explicit state: **AwaitingAccept** (request issued, no answer yet) or **Active** (acceptance received).
- **FR-002**: The node MUST maintain **downstream connections** — those it accepted, serving as its fan-out destinations — as a set of (peer, topic) entries with no further per-entry state.
- **FR-003**: Terminal outcomes (termination, misbehavior severance, shutdown) MUST be expressed as removal of entries; no terminated or rejected state is retained.
- **FR-004**: The connection structures are additions: the existing per-topic candidates knowledge and the existing static bootstrap peer list MUST remain unchanged and distinct from them.
- **FR-005**: Every connection state change MUST occur through the node's single event-processing transition, and every externally visible connection action (sending a control message) MUST be produced as a transition output executed afterwards by the node runtime — preserving the established single-writer, decide-then-execute architecture. Errors during execution of these actions are logged only.

**Autonomous establishment**

- **FR-006**: At construction the node MUST arm a one-shot connection-setup timer with an operator-configurable duration (with a default). Its expiry MUST enter the node as an ordinary event; the timer MUST fire at most once per node lifetime.
- **FR-007**: On processing the setup event, the node MUST obtain the expected upstream set from an injected **connection-selection strategy**, compare it with current connection state, record an AwaitingAccept entry per selected (peer, topic), and issue one connection request per entry. The strategy MUST be encapsulated behind an interface so alternative policies can later be instantiated from configuration without changing the node.
- **FR-008**: The initial strategy MUST select **all known candidates across all the node's topics** as of the moment the setup event is processed. A partial or empty candidate view yields a partial or empty topology; the node MUST NOT re-run selection on later membership changes (static, once-only establishment).
- **FR-009**: The strategy's input (the candidates knowledge) excludes the node itself; the node MUST never request a connection to itself.

**Control messages and handshake**

- **FR-010**: Connections MUST be established and torn down by application-level control messages — **Request**, **Accepted**, **Terminated**, each carrying the topic — exchanged over the existing peer-addressed network without any transport change. No Rejected message exists in this feature.
- **FR-011**: Control messages MUST be signed by the emitting node; nodes therefore acquire a signing capability at construction. Verification of control-message signatures MUST happen inside the transition, alongside all other validation.
- **FR-012**: On receiving a Request that passed the control-message checks (FR-015), the node MUST validate membership against its current view: the request's topic is among the node's own topics AND the requester is a known member of that topic. A valid Request MUST be accepted: record the downstream entry and send Accepted; a Request matching an existing downstream entry MUST be re-confirmed idempotently (entry kept as-is, Accepted re-sent), subject to the same validation. A Request failing membership validation MUST be dropped with a cause-tagged event, no state change, and no reply (no Rejected message exists; the requester is left pending). There is no acceptance policy beyond this fixed membership validation.
- **FR-013**: On receiving an Accepted matching one of its AwaitingAccept entries, the node MUST transition that entry to Active. An Accepted with no matching pending entry MUST be dropped with a cause-tagged event and MUST NOT create or modify any entry.
- **FR-014**: On receiving a Terminated for a held connection (either role), the node MUST remove the matching entry. A Terminated for a connection not held MUST be dropped with a cause-tagged event and no state change.
- **FR-015**: A control message whose sender identity equals the node's own MUST be dropped with no state change. A control message whose signature fails verification MUST be dropped with a cause-tagged event and no state change. No cross-check between the transport frame's sender and the signed emitter identity is performed in this feature.

**Receive-path enforcement and misbehavior**

- **FR-016**: A signed payload message MUST be admitted toward recording only if its sender holds an **Active upstream** connection with the recipient for the message's topic; otherwise it MUST be dropped, emitting the established drop event with a new not-connected cause. Pre-connection delivery semantics are retired. The connection check is the **first** receive check; the pre-existing checks retain their current order after it — subscription filter, then signature verification.
- **FR-017**: A payload message that **passed the connection check and the subscription filter** and then fails signature verification is **misbehavior**: the node MUST remove that upstream entry immediately, raise a distinct misbehavior signal carrying the peer, topic, and cause (in this feature the node runtime only logs it), and MUST NOT send any notice to the offending peer (silent, non-cooperative severance). A message dropped by an earlier check never reaches the misbehavior verdict — in particular, an invalid-signature message on a no-longer-subscribed topic over an Active connection is a plain topic-not-subscribed drop, not a severance.
- **FR-018**: Misbehavior is **signature-only** in this feature. Messages dropped for any other reason (no connection, topic not subscribed) are plain cause-tagged drops and MUST NOT sever connections.
- **FR-019**: For messages admitted through an Active upstream connection, all previously specified receive behavior — subscription filtering, signature verification, drop-event conventions, recording of deliveries — MUST apply unchanged. This is the feature's regression boundary.

**Teardown**

- **FR-020**: The node MUST offer a consuming, awaitable **shutdown** operation that: processes any events already queued ahead of it, clears both connection structures, sends one Terminated notice **per entry in both structures regardless of state — Active and AwaitingAccept upstreams plus all downstreams** (a counterpart of a pending entry may already hold matching state; redundant notices are absorbed by FR-014), and completes only after those notices have been handed to the network; the node's background activity then stops and resources are released.
- **FR-021**: Discarding a node without invoking shutdown MUST remain the abrupt path: background activity stops, no notices are sent, and counterparts are left with stale entries (which admit no messages and are re-confirmed idempotently if the node returns).

**Public surface and observability**

- **FR-022**: The node MUST NOT expose manual connect or disconnect operations. The operator/library verbs are: construction, send, shutdown, and read-only snapshot getters for the upstream map (with per-entry state) and the downstream set.
- **FR-023**: Message sending MUST be preserved unchanged: send resolves once the network accepts the message; enforcement of the connection topology is receiver-side only.
- **FR-024**: Construction MUST be preserved in shape and failure behavior — extended by the signing capability and the setup-timer duration — and a failed construction (e.g., identifier already registered) MUST surface the existing typed error with no background activity left running. An integration test MUST cover this construction-failure path (resolving deferred note N-006).
- **FR-025**: All new drop and misbehavior events MUST follow the established operator-facing event conventions (the drop-event name with a distinct snake_case cause; no specification references in operator-facing strings).

**Scope boundaries (explicitly out of scope here)**

- **FR-026**: No fan-out or forwarding over downstream connections is implemented in this feature; downstream entries are recorded and maintained but carry no traffic yet (deferred to the fan-out feature, 006).
- **FR-027**: No dynamic evolution of the connection set is implemented: no re-selection on membership changes, no reconnection, no garbage collection of stale AwaitingAccept entries, no Rejected message, no acceptance policy beyond FR-012's fixed membership validation, no blacklisting. These form one deferred package revisited when the connection set becomes dynamic.
- **FR-028**: No transport changes: no transport-level connections or multiplexing (deferred to a real transport, 009+; the event-loop/registry contract's sketch of per-connection producers is superseded accordingly), no liveness probing of Active connections, no identity-binding hardening between transport frames and signed emitters, no backpressure changes (the event queue stays unbounded).

### Key Entities

- **Logical connection**: a bilateral, per-(peer, topic) relationship recorded independently by each end; it has no transport existence. Roles: the requester holds it as upstream (a message source); the acceptor holds it as downstream (a fan-out destination). The same pair of nodes may hold both roles for the same topic simultaneously (two independent connections).
- **Upstream connection state**: AwaitingAccept or Active; the only stored states. All exits are removals.
- **Control message**: signed, topic-carrying protocol message — Request, Accepted, or Terminated — a new kind alongside the existing signed payload and plain messages.
- **Connection-selection strategy**: an injected, encapsulated policy consulted exactly once (at setup) that maps the node's current knowledge (its topics and candidates) to the expected upstream set. Initial policy: all candidates across the node's topics.
- **Setup timer / setup event**: a one-shot, configurable-delay trigger entering the node as an ordinary event; the sole initiator of establishment.
- **Misbehavior signal**: the transition's output reporting a severed-for-cause connection (peer, topic, cause); consumed by the node runtime as a log entry in this feature; the future hook for blacklisting.
- **Connection snapshots**: read-only views of the upstream map and downstream set exposed by the node's getters.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: N nodes sharing a topic converge, within one setup delay plus message propagation, to the full bidirectional per-topic graph: every node holds exactly N−1 Active upstream entries and N−1 downstream entries for that topic, and no further connection activity occurs afterwards.
- **SC-002**: 100% of payload messages from senders without an Active upstream connection for the message's topic are excluded from received messages and surfaced as cause-tagged drops; 0 messages are admitted on the strength of a pending (AwaitingAccept) or absent connection.
- **SC-003**: After a single invalid-signature message over an Active connection, that connection is severed and 100% of the offender's subsequent messages on that topic are excluded — while the offender's connections for other topics, and other peers' connections, are untouched.
- **SC-004**: Graceful shutdown leaves zero dangling entries about the departing node on all connected counterparts; an abruptly restarted node returns to Active with every counterpart it re-requests, with zero operator intervention.
- **SC-005**: The reworked pre-existing integration suites pass with establishment preambles added and no other behavioral edits: post-connection receive behavior is byte-for-byte the previously specified behavior.
- **SC-006**: Every lifecycle transition defined in this specification is reachable in synchronous transition tests by feeding events alone (no real timers), with multi-step scenarios expressed as declarative event scripts.
- **SC-007**: No sequence of events — internal or adversarial — produces a connection entry whose peer is the node itself.

## Assumptions

- The in-memory network's existing delivery semantics (reliable, in-order per sender-recipient pair, silent drop toward unregistered recipients) carry over unchanged; control messages rely on them exactly as payload messages do.
- The mock cryptography of the existing signing/verification capability is sufficient for this feature's signature checks; it distinguishes valid from tampered content but does not truly bind identity — identity-binding hardening is explicitly deferred.
- The operator tunes the setup delay to cover registry synchronization for their deployment — the delay protects both roles: dialers select from a converged view, and acceptors validate incoming requests against a converged view. The default value (chosen at planning) is suitable for tests and local runs.
- A node's subscription set and candidate knowledge converge through the registry watch as established by the registry feature; this feature adds no registry interaction of its own.
- Topic-change reconciliation (a node's own registered topics changing after establishment) is deferred; in the meantime, stale traffic arising from it is innocently reachable and therefore never treated as misbehavior.
- Exact identifiers — event names, effect names, drop causes, configuration field name and default — are fixed at planning under the project's existing naming conventions.
- The declarative test-construction standard (constitution v1.2.0) governs the new lifecycle tests and the reworked integration suites; scripting helpers analogous to the registry feature's are expected to be introduced beside the types they exercise.

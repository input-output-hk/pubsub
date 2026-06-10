# Feature Specification: Node Event-Loop Refactor

**Feature Branch**: `004-node-event-loop`

**Created**: 2026-06-09

**Status**: Draft

**Input**: User description: "Node event-loop refactor (behavior-preserving; no new functionality). Reshape the existing node so its mutable state is an explicit state value mutated by a single pure, synchronous state-transition over a typed event stream, driven by one event queue with a single consumer and node-owned producers."

## Overview

This is a **behavior-preserving refactor**. It adds no new protocol behavior. It reshapes the node's internals so that:

- the node's mutable state is an **explicit state value**;
- that state changes **only** through a single **pure, synchronous transition** over a typed stream of **events**;
- events flow through **one queue** with a **single consumer** (the event loop) and **node-owned producers**.

The feature is **Feature A** of the two-feature parallel-work plan described in the shared seam document [`../event-loop-and-registry-contract.md`](../event-loop-and-registry-contract.md); that document defines the event-queue boundary that the mock topic registry (008) builds on, and this spec is one of the two that cite it. The connection model and the registry reader are **out of scope** here (see Scope Boundaries).

The value is internal but real: the message-handling logic becomes **synchronously testable in isolation**, the state becomes a single auditable value rather than scattered fields, and the event stream becomes the **one extension seam** that parallel and future features attach to without disturbing existing behavior.

## Clarifications

### Session 2026-06-09

- Q: Is the pure core (state type + transition function) exposed as public library API, or crate-internal with `Node` remaining the only public surface? → A: Crate-internal — `Node` stays the only public surface; the synchronous state-machine tests live as in-crate unit tests; no new public API commitment is made by this feature (no external consumer justifies one).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Identical messaging behavior after the refactor (Priority: P1)

A consumer of the node (library caller, operator, or existing acceptance test) observes **exactly** the messaging behavior they had before: subscribed topics are received, off-topic and badly-signed messages are dropped with the same drop events, and the snapshot getters and subscribe/unsubscribe calls behave identically.

**Why this priority**: Parity is the whole point of a refactor. If any observable behavior changes, the refactor has failed regardless of how clean the internals are. This story is the acceptance gate.

**Independent Test**: Run the unchanged 002 and 003 acceptance tests against the refactored node; every one passes without modification. New behavior need not be added for this story to deliver value.

**Acceptance Scenarios**:

1. **Given** a node subscribed to topic T, **When** a validly-signed message on T is received, **Then** it appears in the received-messages snapshot in receive order.
2. **Given** a node not subscribed to topic U, **When** a message on U is received, **Then** it is dropped, does not appear in the snapshot, and a `message_dropped` event with `cause = "topic_not_subscribed"` is emitted.
3. **Given** a node subscribed to topic T, **When** a message on T with an invalid signature is received, **Then** it is dropped, does not appear in the snapshot, and a `message_dropped` event with `cause = "invalid_signature"` is emitted.
4. **Given** a node, **When** `subscribe(T)` is called on a new topic and then again on the same topic, **Then** the first returns the "added" outcome and the second the "already present" outcome, matching pre-refactor semantics.
5. **Given** a node, **When** `unsubscribe(T)` is called for a present then an absent topic, **Then** the first returns "removed" and the second "not subscribed".
6. **Given** several messages received in a known order, **When** the received-messages snapshot is taken, **Then** it reflects every message processed before the snapshot, in receive order.

---

### User Story 2 - Message-handling logic testable as a pure synchronous state machine (Priority: P2)

A node developer exercises the message-handling logic by constructing a state value, feeding it a scripted sequence of events, and asserting on the resulting state after each event — **without** spawning any asynchronous runtime, task, channel, or performing any I/O.

**Why this priority**: This is the durable maintainability payoff. It makes the protocol logic fast and deterministic to test and reason about, independently of the concurrency plumbing, and it is what later features depend on when they add new transitions.

**Independent Test**: Write a synchronous (non-async) **in-crate unit test** that builds the state value, applies a `Vec` of events one at a time, and asserts on state after each step. The test compiles and runs with no async runtime present. (The pure core is crate-internal — see Clarifications — so these tests live inside the crate, not in the external integration-test suite.)

**Acceptance Scenarios**:

1. **Given** a freshly constructed state value subscribed to T, **When** a "message received on T" event is applied, **Then** the state's received list grows by exactly that delivery and no I/O or task spawning occurs.
2. **Given** a state value, **When** an "off-topic message received" event is applied, **Then** the received list is unchanged.
3. **Given** a scripted sequence of events, **When** they are applied in order, **Then** the final state is a deterministic function of the sequence (same input sequence ⇒ same state).

---

### User Story 3 - New event sources and event kinds attach at one seam (Priority: P3)

A parallel or future feature (the mock topic registry, 008; later the connection model) attaches a **new event source** and, where needed, a **new kind of event**, by registering a node-owned producer and adding one transition branch — **without editing existing transition branches** or the existing producers.

**Why this priority**: This is the forward-compatibility guarantee that lets 008 proceed in parallel and lets the connection model land later. It is lower priority than parity and testability because it is about extension points, not current behavior, but it must hold for the parallel-work plan to work.

**Independent Test**: Add a no-op producer through the node's producer-registration mechanism and confirm it receives the queue feed; confirm the existing message-handling branch is untouched and all P1 scenarios still pass.

**Acceptance Scenarios**:

1. **Given** the node, **When** an additional node-owned producer is registered, **Then** events it pushes are processed by the same single consumer alongside existing producers' events.
2. **Given** the typed event stream, **When** a new event kind is introduced, **Then** it is added without reshaping the transition's input/output contract and without modifying existing branches' logic.

---

### Edge Cases

- **Event pushed after the node is dropped / the loop has shut down**: the event is silently discarded; pushing never panics and never blocks the producer (preserves the existing fire-and-forget feed semantics).
- **Events still queued when the node is dropped**: discarded along with the queue; teardown does not drain or process remaining events. (Same fate as events pushed after shutdown.)
- **Empty subscription set**: a node subscribed to nothing drops every inbound message (each with `cause = "topic_not_subscribed"`), unchanged from current behavior.
- **Subscribe/unsubscribe while events are being processed concurrently**: the control-plane call and event processing are serialized so that a snapshot taken afterward is consistent (no torn or lost updates).
- **Node dropped while producers are running**: dropping the node stops event processing and terminates all node-owned producers; no producer outlives the node.

## Requirements *(mandatory)*

### Functional Requirements

**Behavioral parity (observable behavior preserved from 002 / 003):**

- **FR-001**: The node MUST receive and record validly-signed messages on topics it is subscribed to, observable in receive order through the received-messages snapshot.
- **FR-002**: The node MUST drop messages on topics it is not subscribed to and emit a `message_dropped` event with `cause = "topic_not_subscribed"`.
- **FR-003**: The node MUST drop messages whose signature does not verify and emit a `message_dropped` event with `cause = "invalid_signature"`, with topic filtering applied before signature verification.
- **FR-004**: `subscribe` and `unsubscribe` MUST remain synchronous calls returning the existing outcome distinctions (added vs. already-present; removed vs. not-subscribed) and emitting their existing subscription log events.
- **FR-005**: The node MUST expose a snapshot of its current subscription set and a snapshot of received deliveries, each a stable copy unaffected by subsequent activity.
- **FR-006**: State observations MUST be consistent: any snapshot reflects all events and control-plane changes ordered before it, with no torn or lost updates (linearizability preserved from 003).
- **FR-007**: All receive-path drops MUST follow the `message_dropped` event convention with a `snake_case` `cause` field; operator-facing strings MUST NOT carry requirement/spec citations.

**Structural exposure (the reshape, stated as observable/testable contract):**

- **FR-008**: The node's mutable state MUST be a single explicit state value, changed **only** by one transition function; that transition MUST be **pure** (no **protocol** I/O, no concurrency, no asynchrony) and exercisable **synchronously** in isolation — inline emission of operator log events is permitted ambient observability, per Assumptions. The state value and transition are **crate-internal** — they are a refactoring of the node's internals, not new public API; the node remains the only public surface.
- **FR-009**: A constructed node MUST process queued events concurrently while remaining usable through its synchronous getters and subscribe/unsubscribe methods.
- **FR-010**: Events MUST be deliverable from multiple node-owned producers and from an ad-hoc feed handle; the node MUST own its producers' lifecycles.
- **FR-011**: Dropping the node MUST stop event processing and terminate all of its producers; no producer may outlive the node.
- **FR-012**: The typed event stream MUST be open to new event kinds without reshaping the transition's input/output contract, so a new source (e.g., the registry reader, 008) attaches by adding a producer and a single transition branch without modifying existing branches.

**Scope boundaries (explicitly out of scope here):**

- **FR-013**: The connection model (dialing, accepting, per-connection send/receive, message fan-out) MUST NOT be implemented in this feature; the transition's outbound-command output type ships present-but-empty and the transition returns no outbound commands. This is deferred to a later `004-connections` feature.
- **FR-014**: Registry-driven subscription confirmation and updates MUST NOT be implemented in this feature; the registry-update event seam is reserved for 008 and consumed there.
- **FR-015**: Message **sending** MUST be preserved unchanged: `send` resolves once the network accepts the message; sending to an unregistered recipient is silently dropped with no synchronous error to the sender; sending is independent of the sender's own subscription set.
- **FR-016**: Node **construction** MUST be preserved unchanged: a successful construction returns a node already able to process events; a failed construction (e.g. an identifier already registered on the network) surfaces the existing typed error and leaves **no background activity running**.

### Key Entities

- **Node state**: the node's full mutable state as one explicit value — its own identity, its subscription set, and its recorded received deliveries (plus the verifier it consults). It is the sole thing the transition mutates and the sole thing the getters read.
- **Event**: a typed item that can change node state. The only event kind exercised in this feature is "message received from a peer." The stream is open to further kinds (a reserved registry-update kind for 008; connection-related kinds later).
- **Outbound command (effect)**: the typed output of the transition representing work the node must perform externally. **No kinds exist in this feature** (the node only ingests); the type is present to fix the transition's contract for the connection model.
- **Producer**: a node-owned source that pushes events onto the queue (the network mailbox in this feature; the registry reader in 008). Its lifecycle is owned by the node.
- **Event queue / feed handle**: the single channel onto which producers and ad-hoc callers push events, consumed by exactly one event loop.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the existing 002 and 003 acceptance tests pass against the refactored node **without modification** to those tests.
- **SC-002**: The message-handling logic can be tested with **zero** asynchronous runtime, task spawning, channels, or I/O — a purely synchronous "apply a sequence of events, assert state after each" test compiles and passes.
- **SC-003**: Introducing a new event kind and a new event source requires adding exactly one transition branch and one producer registration, with **no edits to existing transition branches or existing producers** (verifiable by the 008 branch adding its registry-update handling additively).
- **SC-004**: The node's public observable surface (all public methods and their outcomes — construction, send, getters, subscribe/unsubscribe, producer registration, drop behavior) is **unchanged** from 003, and **no new public API is added** (the state value and transition stay crate-internal); no consumer of the 003 API needs to change call sites.
- **SC-005**: Dropping a node terminates all its producers and stops event processing, observable by no further events being recorded after drop and no leaked background activity.

## Assumptions

- **Subscriptions are config-seeded and static in this feature.** The subscription set is populated at construction from already-parsed configuration and is otherwise modified only in tests. Topics read from configuration are **assumed already confirmed in the registry**; registry-confirmed subscription is a future concern (008).
- **Future direction**: once registry-driven subscription-update events flow through the queue (008 and beyond), the synchronous `subscribe`/`unsubscribe` methods will likely be **deprecated** in favor of event-driven updates. They are retained as an interim control-plane here.
- **Logging is operator UX, not a transition output.** Drop and subscription log events are emitted inline as ambient observability where the corresponding decision is made; they are not modeled as transition outputs and are not a test-assertion surface (per the constitution).
- **Parse at the edge.** The node is constructed from already-parsed in-memory values; file and wire decoding remain in the CLI / network-edge layers (unchanged from 001–003).
- **Shared seam dependency.** This feature defines and owns the event-queue boundary specified in [`../event-loop-and-registry-contract.md`](../event-loop-and-registry-contract.md). Feature 008 depends on that boundary; this feature does not depend on 008.
- **Concrete shapes are deferred to the plan.** Exact state/event/command type definitions, the state-sharing mechanism, the producer-registration signature, lifecycle management, and code-organization conventions are decided in `/speckit-plan` and recorded as ADR(s) (constitution Principle III), not in this spec.

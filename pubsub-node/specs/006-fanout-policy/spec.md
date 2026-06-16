# Feature Specification: Message Publishing and Fan-out Forwarding

**Feature Branch**: `006-fanout-policy`

**Created**: 2026-06-16

**Status**: Draft

**Input**: User description:

> Add message publishing and fan-out forwarding to the node. A node can originate a dissemination message via a new fire-and-forget `Node::publish(SignedMessage)` method, which enqueues an `Event::Publish` consumed by a dedicated `handle_publish` transition and returns immediately — validation happens in the handler, and rejections surface as `message_dropped` log events, mirroring the receive path (the caller observes success via `received_messages()`, not a return value). Publishing validates the message before accepting it: the topic must be one the node is subscribed to (its own membership) and registered (legitimate) in the topic registry, the publisher must be authorized for the topic, and the signature must verify — the same checks as the receive path except the connection gate, since a published message has no upstream source. `publish` does not require the message's publisher to be the node itself: any validly-signed, authorized message is accepted, enabling proxy/injection of an external publisher's pre-signed message. An accepted message is recorded and fanned out.
>
> Fan-out forwarding applies on both paths: when a node accepts a received dissemination message (over an Active upstream), and when it publishes one. The node forwards the message **verbatim** — the original publisher's signature intact, no re-signing — to its downstream connections on that topic, excluding the peer that delivered it (split-horizon — a peer may be both an upstream source and a downstream sink on the same topic; on the publish path there is no deliverer to exclude). Forwarding targets are chosen by a pure, synchronous `FanoutStrategy` trait — a seam mirroring `ConnectionStrategy` — whose sole v1 implementor `ForwardToAll` returns every downstream peer on the topic (target order unspecified). Each forward is an `Effect::Send`; no new effect variant is introduced. Because a node only holds downstream connections on topics it is a member of, this is subscriber-relay: a node never relays a topic it is not subscribed to.
>
> Duplicate suppression prevents forwarding loops: the node keeps a set of seen message hashes (`MessageHash::of` over the plain content), unbounded in the in-memory model (bounding deferred to a real implementation). A message already seen is dropped with no record and no fan-out; a first-seen message is recorded, marked seen, and fanned out. The dedup check sits at the record point, after signature verification, so a failed verification never poisons the set. Equivocation detection (two distinct messages sharing a publisher and sequence) is out of scope — distinct content yields distinct hashes, so both propagate.
>
> The recorded delivery's origin is modelled explicitly as `enum Origin { Local, Peer(PeerId) }` — `Local` for the node's own published messages, `Peer(id)` for the forwarding peer of a received message; the publisher identity remains inside the message envelope.
>
> Testing: the existing receive-path unit tests hold an empty downstream set, so `ForwardToAll` is a no-op for them and they need only the shared constructor's strategy argument added. The `FanoutStrategy` is injected like `ConnectionStrategy`; a test-only no-op strategy lives in `test_support` (never in the production surface) for connection-lifecycle tests where fan-out is irrelevant noise, while dissemination tests use `ForwardToAll` and assert the forwarding.
>
> Out of scope: the epochal/periodic re-dialer (a connections concern, deferred), pick-k fan-out subsets (which would break the deterministic `apply` and require a seeded RNG in state), and renaming `Message::Signed` to `Message::Dissemination` (a separate mechanical refactor).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A node publishes a message and it reaches its direct subscribers (Priority: P1)

An application holding a node calls `publish` with a fully-signed dissemination message on a topic the node is a member of. The node validates the message, records it as a locally-originated delivery, and forwards it to every downstream peer it holds on that topic. Each of those peers receives the message over the connection it established.

**Why this priority**: This is the feature's headline capability — until a node can originate a message and push it to its neighbours, the connection topology built in 004 carries no traffic. It is the smallest slice that turns the static graph into a working dissemination channel.

**Independent Test**: Establish one publisher node with one or more downstream peers on a shared topic (through the real connection path); call `publish` with a validly-signed, authorized message; observe that the publisher records the message with a `Local` origin and that every downstream peer records it.

**Acceptance Scenarios**:

1. **Given** a node subscribed to a registered topic with two downstream peers on it, **When** the application publishes a validly-signed, authorized message on that topic, **Then** the publisher records the message once (origin `Local`) and a forward is emitted to each of the two downstream peers.
2. **Given** a node with no downstream peers on the topic, **When** it publishes a valid message, **Then** the message is recorded (origin `Local`) and no forwards are emitted.
3. **Given** a node, **When** it publishes a message whose publisher key is a different, topic-authorized entity (not the node's own key), **Then** the message is accepted, recorded, and fanned out (proxy/injection).
4. **Given** a node, **When** it publishes a message on a topic it is not subscribed to, or whose publisher is not authorized, or whose signature does not verify, **Then** the message is dropped (a `message_dropped` event is logged), not recorded, and not fanned out.

---

### User Story 2 - A received message is relayed onward through the mesh (Priority: P2)

A node receiving a dissemination message over an Active upstream — after it passes every receive-path check and is recorded — forwards it to its own downstream peers on that topic, excluding the peer that delivered it. Across a connected mesh this lets a single published message reach members several hops from the publisher.

**Why this priority**: Multi-hop relay is what makes the system a dissemination *network* rather than a one-hop push. It builds directly on US1's fan-out machinery, applied to the receive path, and is independently demonstrable once US1 exists.

**Independent Test**: Build a three-node line A→B→C sharing a topic (A downstream of nobody upstream of B, B between A and C); publish at A; observe C records the message, having received it relayed by B, and that B does not forward it back to A.

**Acceptance Scenarios**:

1. **Given** a node holding an Active upstream toward peer X and a downstream toward peer Y on a topic, **When** a valid message on that topic is delivered by X, **Then** the node records it and emits a forward to Y.
2. **Given** a node where the delivering peer is also one of its downstream peers on the topic (a bidirectional connection), **When** that peer delivers a valid message, **Then** the node forwards to its other downstream peers but **not** back to the delivering peer (split-horizon).
3. **Given** a node whose only downstream peer on the topic is the peer that delivered the message, **When** that message is delivered, **Then** the node records it and emits no forwards.
4. **Given** a connected mesh of nodes all sharing a topic, **When** one node publishes a message, **Then** every other member records the message exactly once.

---

### User Story 3 - Forwarding loops are suppressed (Priority: P3)

In a mesh with cycles (the full bidirectional per-topic graph 004 builds is cyclic for three or more nodes), a node that has already seen a message drops any later copy without recording or forwarding it. This prevents a message from circulating forever.

**Why this priority**: Without duplicate suppression, US2's relay would loop indefinitely in any cyclic topology — so dedup is what makes relay safe at scale. It is separable: US1 and US2 are observable in acyclic topologies (a line/star) before dedup exists; dedup is required to extend them to general meshes.

**Independent Test**: Build a triangle of three mutually-connected nodes on a topic; publish at one; observe each node records the message exactly once and message propagation terminates (a bounded, finite number of forwards), with no node recording or re-forwarding a second copy.

**Acceptance Scenarios**:

1. **Given** a node that has already recorded a message, **When** an identical copy of that message is later delivered over an Active upstream, **Then** it is dropped — not recorded a second time and not forwarded again.
2. **Given** a node that published a message (and thereby recorded and marked it seen), **When** a downstream peer that is also an upstream relays the same message back, **Then** the node drops it as already-seen.
3. **Given** a triangle of three mutually-connected members on a topic, **When** one publishes a message, **Then** the total number of forwards across all nodes is finite and each node holds exactly one recorded copy.

---

### Edge Cases

- **Publish with no downstream**: the message is recorded (origin `Local`) but produces no forwards — publishing is still observable locally.
- **Split-horizon collapses to no-op**: when the delivering peer is not among the node's downstream peers, excluding it changes nothing; when it is the sole downstream, the exclusion yields an empty target set.
- **Invalid-signature publish**: dropped at signature verification; because publishing has no upstream connection, this is a plain drop — it never triggers the 004 misbehavior/severance path (which is gated on an Active upstream) and never marks the message seen.
- **Duplicate of a previously-dropped message**: a message that failed a check is never marked seen, so a later copy is re-evaluated from scratch (and dropped again if it still fails) — failed verification does not poison the seen-set.
- **Off-topic / unregistered / unauthorized received message**: dropped by the existing receive-path checks before the record/fan-out point — never relayed (a node never relays a topic it is not a member of).
- **Equivocation**: two distinct messages sharing a publisher and sequence have distinct content hashes, so both are recorded and both propagate; detecting the conflict is out of scope.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The node MUST expose a fire-and-forget publish operation taking a complete signed dissemination message, which enqueues a publish event onto the node's event queue and returns immediately without a validation verdict.
- **FR-002**: A published message MUST be validated in the transition before acceptance, applying the same checks as the receive path **except the connection gate**: the topic is among the node's own (membership-derived) subscriptions, the topic is registered (legitimate) in the topic registry, the message's publisher is authorized for the topic, and the signature verifies over the message's signing bytes.
- **FR-003**: Publishing MUST NOT require the message's publisher identity to equal the node's own identity — any validly-signed, topic-authorized message is accepted, enabling proxy/injection of an externally-signed message.
- **FR-004**: An accepted published message MUST be recorded as a delivery with a `Local` origin and then fanned out.
- **FR-005**: A published message that fails any validation check MUST be dropped under the existing `message_dropped` log convention (with a cause), not recorded, and not fanned out. A publish never severs a connection (there is no upstream to sever).
- **FR-006**: On the receive path, a message that passes every check and is recorded MUST then be fanned out (forwarding is applied at the same point as recording, on both the publish and receive paths).
- **FR-007**: Fan-out MUST forward the message **verbatim** — the original publisher's signature unchanged, with no re-signing by the forwarding node.
- **FR-008**: Fan-out targets MUST be the node's downstream peers on the message's topic, selected by an injected, pure, synchronous fan-out strategy. The v1 strategy `ForwardToAll` returns every downstream peer on the topic; target order is unspecified.
- **FR-009**: On the receive path, fan-out MUST exclude the peer that delivered the message (split-horizon). On the publish path there is no delivering peer, so no exclusion applies.
- **FR-010**: The fan-out strategy MUST be injected at node construction in the same manner as the connection-selection strategy, and MUST be a single encapsulated trait so later strategies can replace it without reshaping the transition.
- **FR-011**: Each forward MUST be expressed as the existing send effect; the feature introduces no new effect variant.
- **FR-012**: The node MUST track the set of message hashes it has already accepted (the content hash over the plain message). A message whose hash is already present MUST be dropped — not recorded and not forwarded; a first-seen message MUST be recorded, added to the set, and forwarded.
- **FR-013**: The duplicate check MUST occur at the record point, **after** signature verification, so a message that fails verification never enters the seen-set.
- **FR-014**: A recorded delivery MUST carry an explicit origin distinguishing a locally-published message (`Local`) from a message forwarded by a peer (`Peer(peer-id)`); the publisher identity remains carried inside the message itself.
- **FR-015**: Duplicate suppression MUST span both paths — a message the node published (and thereby recorded and marked seen) MUST be dropped if a peer later relays it back.
- **FR-016**: The existing receive-path behavior for a message with an empty downstream set MUST be unchanged — recording occurs and fan-out emits nothing.

### Key Entities *(include if feature involves data)*

- **Publish event / handler**: a new event-queue variant carrying a signed dissemination message, dispatched to a dedicated transition handler that performs the publish-path validation, recording, dedup, and fan-out.
- **Fan-out strategy**: an encapsulated, pure, synchronous decision of which downstream peers receive a forward of a message on a given topic, given the node's downstream set and the peer (if any) to exclude. The v1 implementor forwards to all downstream peers on the topic.
- **Seen-message set**: the node-held set of accepted message content hashes used for duplicate suppression; unbounded in the in-memory model.
- **Delivery origin**: the explicit distinction, on a recorded delivery, between a locally-published message and one forwarded by a named peer.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: When a node publishes a valid message, 100% of its downstream peers on that topic record the message, and the publisher records exactly one copy.
- **SC-002**: In a connected mesh of N nodes all sharing a topic, a single publish results in all N members (publisher included) recording the message — full coverage.
- **SC-003**: No node records or forwards the same message more than once, in any topology including cyclic meshes (zero duplicate records per node).
- **SC-004**: A node never forwards a message back to the peer that delivered it (zero split-horizon echoes observed).
- **SC-005**: Message propagation in any finite topology terminates in a bounded number of forwards (no unbounded circulation).
- **SC-006**: A published message whose topic, authorization, or signature check fails is never recorded by any node and never forwarded.

## Assumptions

- Builds on the merged **004-connections** model: per-`(peer, topic)` upstream (`AwaitingAccept`/`Active`) and downstream connection sets in node state, the pure `apply` → `Vec<Effect>` transition with `Effect::Send`, the injected `ConnectionStrategy` seam, and signed messages — and on the merged **008** subscription registry (membership-derived `subscriptions` and per-topic `candidates`) and **013** topic registry (registered topics + authorized publishers). The publish path reuses the receive-path checks minus the connection gate.
- The caller constructs and signs the `SignedMessage` (including its publisher identity, sequence, and timestamp) before calling `publish`; the node does not mint or sign dissemination messages and consults no clock.
- The duplicate-suppression key is the content hash over the plain message (`MessageHash::of`), consistent with the existing content-anchored hash. The seen-set is unbounded in the in-memory model; bounding (LRU/TTL) is deferred to a real implementation.
- The transition remains pure and deterministic in its state outcome; `ForwardToAll` is deterministic, and because fan-out target order is unspecified, tests sort targets before asserting (as the existing connection-effect helpers do).
- This feature is, like 004, **not parity-preserving** at the integration level: dissemination suites are reworked to assert forwarding. Receive-path unit tests with empty downstream sets are unaffected beyond the shared constructor gaining the fan-out-strategy argument; a test-only no-op fan-out strategy lives in `test_support` for connection-lifecycle suites where fan-out is irrelevant noise, and is never part of the production surface.

## Out of Scope

- The epochal / periodic re-dialer (a connections concern: re-firing the setup event on an interval). The existing one-shot setup machinery is unchanged; periodic re-selection is deferred to its own slice.
- Pick-k fan-out subsets (e.g. forward to a random k of the downstream set); these would require a seeded RNG in node state to preserve deterministic `apply` and are deferred. `ForwardToAll` is the sole v1 strategy.
- Renaming `Message::Signed` to `Message::Dissemination` (and the `SignedMessage`/`PlainMessage` types) — a separate mechanical refactor, recorded for a future pass.
- Equivocation / conflicting-message detection (publisher chain-integrity, parent-hash, sequence monotonicity, deposits) — unchanged and deferred to later features.
- Bounding the seen-set; backpressure; connection liveness/heartbeats.

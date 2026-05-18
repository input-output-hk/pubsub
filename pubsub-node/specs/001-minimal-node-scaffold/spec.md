# Feature Specification: Minimal PubSub Node Scaffold

**Feature Branch**: `001-minimal-node-scaffold`

**Created**: 2026-05-17

**Status**: Draft

**Input**: User description: "I would like to implement the most basic pubsub node draft we can imagine. In the context of this project, we are still researching the best peer discovery and message dissemination protocols. So, for the very first iteration, I want to build a node that reads the peers descriptors it will connect to from a config file. We assume that the list of descriptors is provided by the user, and that these nodes won't be down. At this stage, nodes trust each other, and will accept connection requests as they arrive without checks. The nodes will be able to send a basic message (like a Ping(<number>)) to test connectivity. No cryptography, nor special algorithms. Just basic scaffolding pieces. Network layer should also be simplified to allow for simple \"In Memory\" connections (e.g. the network object is initialized with another message box object, and each peer is initialised by sharing this common network object to keep them connected, the network is in the background just a hashmap of peers to messages)."

## Clarifications

### Session 2026-05-17

- Q: What format should a peer descriptor take in v1 of the scaffold? → A: Abstract/opaque descriptor type exposing an `id()` accessor that returns a UTF-8 string. In v1 the descriptor carries no other fields; future iterations may add fields (e.g., network address, public key) and the identity basis itself may be replaced (e.g., a key-derived id) without breaking callers that only need to address a peer via `id()`.
- Q: What file format should the peer-set config use in v1? → A: TOML.
- Q: What concurrency model should the v1 InMemory network use? → A: Async/await. The Node and Network APIs expose async send/receive entry points from v1 so the interface already matches future networked transports, and to let the test harness establish patterns for async integration testing. Runtime choice (e.g., tokio) is a planning-stage decision.
- Q: Which of FR-006's three alternatives should be the normative way a receiver exposes delivered messages in v1? → A: Per-node queryable record (e.g., `received_messages()` returning the list of deliveries with sender id and payload). Acceptance scenarios assert against this record. Implementations MAY additionally emit structured logs but the record is the normative observability mechanism.
- Q: When a send targets an identifier that isn't registered on the InMemory network, what should the v1 contract be? → A: MUST log + drop. The network silently drops the message (preserving fire-and-forget per FR-004) AND MUST emit a warn-level structured log entry naming the unknown id. The send caller does not observe a synchronous error.
- Q: Where should a node's own identifier come from at startup? → A: Supplied as an explicit argument to the Node constructor (and exposed via a CLI flag at the binary boundary) — identity and peer view are independent concerns; the peer-list TOML does NOT carry the node's own id. This leaves room for identity to become key-derived later without changing the peer-list schema. **Layering note (planning input):** the Node constructor takes the peer set as an already-parsed in-memory value, NOT a filesystem path. File reading + TOML parsing happen in a separate CLI/loader layer that yields the parsed value before the Node is constructed. This keeps the Node API testable without fixtures on disk and isolates I/O failure modes from domain logic.
- Q: When `send(...).await` resolves on the InMemory network in v1, what should be true about the recipient's record? → A: Enqueued but not yet observable. `send().await` resolves once the network has accepted the message for delivery; the recipient may process it into its observable `received_messages()` record subsequently. Tests/operators asserting delivery MUST use a delivery-observation primitive (poll-with-timeout, await-on-condition, etc.). This decoupling mirrors how real networked transports will behave in future iterations and forces the v1 test harness to develop an await-on-delivery affordance that will still be useful then.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Two-Node Ping Exchange via InMemory Network (Priority: P1) 🎯 MVP

A developer instantiates two node objects inside a single process, each configured with the other as its sole peer, both sharing one InMemory network instance. The developer triggers one node to send a Ping(N) addressed to its peer; the receiving node observes the Ping(N) it received.

**Why this priority**: This is the smallest demonstration that the node concept, peer addressing, and the InMemory network substrate all compose correctly. Without it, no later iteration has a working substrate, so it is the irreducible MVP for the scaffolding workstream.

**Independent Test**: Spawn two nodes A and B in a test harness, share one InMemory network, configure A's peer set to contain B and B's peer set to contain A, invoke a "send Ping(N)" operation from A targeting B, observe Ping(N) appearing in B's record of received messages.

**Acceptance Scenarios**:

1. **Given** two nodes A and B attached to a shared InMemory network with A's peer set containing B, **When** A sends Ping(42) addressed to B, **Then** B's record of received messages contains exactly one Ping(42) attributed to A.
2. **Given** two nodes A and B attached to a shared InMemory network with A's peer set containing B but B's peer set NOT containing A, **When** A sends Ping(7) addressed to B, **Then** B still receives Ping(7) from A (trust-on-arrival: reception does not require reciprocal listing).
3. **Given** a node A whose peer set is empty, **When** A attempts to send a Ping, **Then** no message is delivered and the node does not enter an undefined state.

---

### User Story 2 — N-Node Graph via Per-Node Configuration (Priority: P2)

A developer configures N nodes (2 ≤ N ≤ 10 for demonstration purposes), each with its own peer set, and the resulting graph of pairwise links allows Pings to flow along configured edges without crossing into non-peer connections.

**Why this priority**: Confirms the design generalizes beyond two nodes — the natural next step after MVP — and that the InMemory network correctly multiplexes messages among many concurrent participants. Lower than P1 because P1 already proves the core delivery path; P2 generalizes.

**Independent Test**: Define a 4-node graph (e.g., a star with A at the centre connected to B, C, D), construct each node with its peer set, send a Ping from A to each of its peers, verify each addressed peer receives its Ping and no non-addressed peer receives anything.

**Acceptance Scenarios**:

1. **Given** four nodes A, B, C, D with A's peer set = {B, C, D} and others' peer sets initially empty, **When** A sends Ping(N) addressed in turn to each of its peers, **Then** each addressed peer receives exactly the Ping addressed to it and no other peer receives anything.
2. **Given** the same graph, **When** any other node sends a Ping addressed to A, **Then** A receives it even though A's outbound peer set is unrelated to its inbound traffic.

---

### User Story 3 — Peer Descriptors Loaded from a Config File (Priority: P3)

A developer authors a node's peer set in an external, human-readable configuration file rather than in source code, and starts a node by pointing it at that file. The node's resolved peer set matches the file's contents.

**Why this priority**: This is the user-facing configuration boundary described in the input. P1 and P2 are testable via in-process constructors; P3 makes the system usable outside the test harness and is the bridge to operator-level use. Lower than P2 because P2's behaviour can be exercised without P3's file-loading machinery.

**Independent Test**: Place a config file on disk listing three peer descriptors, start a node passing it the config file's path, query the node's resolved peer set, verify it contains exactly the three descriptors from the file.

**Acceptance Scenarios**:

1. **Given** a config file listing three valid peer descriptors, **When** a node is started with a path to that file, **Then** the node's resolved peer set contains exactly those three descriptors.
2. **Given** a malformed config file (syntactically invalid or missing required fields), **When** the node is started, **Then** startup fails with a clear, actionable error identifying the config problem; the node does not start with a partial or default peer set.

---

### Edge Cases

- A node started with an empty peer set: starts successfully; cannot originate sends; may still receive incoming messages from senders that have it in their peer set (per trust-on-arrival).
- A send targets an identifier that is not registered on the network: the send is dropped silently (consistent with the trusted, assumed-up setup); an operator-visible log entry records the drop.
- Two distinct nodes attempt to register under the same identifier on the same network: out of scope at this stage — identifier uniqueness is the responsibility of the configuration author under the trust assumption.
- A node attempts to send to a peer before that peer has joined the network: out of scope — the peer set is established at startup before any send is initiated.
- The numeric value `N` carried by Ping is unusually large, zero, or negative: any value the chosen numeric type accepts is valid; semantics of `N` are out of scope.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST allow a node's peer set to be loaded from a TOML configuration file, where each entry is a peer descriptor sufficient for routing messages over the network in use. File reading and TOML parsing are performed by a CLI/loader layer that yields an in-memory peer-set value; the Node constructor itself consumes that already-parsed value (see FR-012). The exact TOML schema is a planning-stage decision and is recorded in the plan/quickstart artifacts.
- **FR-002**: The system MUST provide an InMemory network abstraction that allows multiple in-process node instances to register themselves and exchange messages by addressing each other through their registered identifier.
- **FR-003**: A node MUST accept incoming connection requests and incoming messages from any sender on the same network without performing any authentication, authorization, or admission check (trust-on-arrival under the PoC trust assumption). In the InMemory variant this rule applies trivially; the requirement is documented now so future networked variants inherit it explicitly.
- **FR-004**: The system MUST support a Ping message that carries an opaque numeric value `N` and MUST allow `N` to be inspected by the receiver. Ping is **one-way (fire-and-forget)**: the sender's send operation completes without waiting for acknowledgment, the receiver MUST NOT emit any response message, and successful receipt is verified by inspecting the receiver's observable state per FR-006.
- **FR-005**: A node MUST be able to originate a Ping(N) message addressed to a specific peer identified by its descriptor (one-to-one send). Broadcast or multi-cast semantics are out of scope at this stage.
- **FR-006**: A receiving node MUST expose a queryable record of every received message (carrying at least the sender's id and the message payload), accessible to the operator or test without inspecting internal state directly. This record is the normative observability surface against which acceptance scenarios assert. Implementations MAY additionally emit structured log output.
- **FR-007**: The system MUST NOT perform any cryptographic operations in this iteration (no signatures, no hashing for authentication, no key material, no encryption).
- **FR-008**: The system MUST treat the configured peer set as static for the lifetime of each node: no peer discovery, no health checks, no failure handling, no reconnection logic, no peer-set mutation after startup.
- **FR-009**: Peer descriptors MUST expose an `id()` accessor that uniquely identifies a peer within a single network instance; duplicate ids on the same network are not supported and need not be detected at this stage. The descriptor type is intentionally abstract so future iterations can add fields (e.g., network address, public key) — or replace the identity basis — without breaking callers that only need to address a peer.
- **FR-010**: The InMemory network MUST route a message addressed to a registered identifier to the corresponding peer. Messages addressed to an unregistered identifier MUST be dropped (consistent with fire-and-forget send per FR-004) AND MUST result in a warn-level structured log entry that names the unknown identifier; the send caller does NOT observe a synchronous error in this case.
- **FR-011**: The Node and Network public APIs for sending and receiving messages MUST be asynchronous (Future-returning / `async fn` in the chosen implementation language), so the abstraction is shape-compatible with future networked transports and the test harness exercises async integration patterns from v1. The specific async runtime is a planning-stage decision.
- **FR-012**: The Node public constructor MUST take (a) the node's own identifier as an explicit argument and (b) its peer set as an already-parsed in-memory value (NOT a filesystem path). Loading and TOML parsing of the peer-set config file are handled by a separate CLI/loader layer that yields the parsed value before the Node is constructed. This keeps identity and peer view independent, isolates I/O failure modes from domain logic, and lets tests build a Node without touching the filesystem. The binary's CLI MUST expose both a `--self-id` (node identifier) and a config-path flag.
- **FR-013**: On the InMemory network, `send(...).await` MUST resolve once the network has accepted the message for delivery to the addressed recipient (enqueue complete). It is NOT required that the recipient's `received_messages()` record contain the message at the moment `send().await` resolves; the recipient MAY process the message into its observable record subsequently. Acceptance assertions on the receiver's record MUST therefore be expressed via an await/poll-with-timeout primitive supplied by the test harness, NOT by inspecting the record immediately after `send().await`. This decoupling preserves contract compatibility with future networked transports.

### Key Entities

- **Node**: A participant in the network. At construction it is supplied with (1) its own identifier and (2) its peer set as an already-parsed in-memory value (FR-012). Holds a reference to a network handle. Originates and receives messages.
- **Peer Descriptor**: An abstract/opaque type identifying another node, exposing at least an `id()` accessor that returns a UTF-8 string used for routing and uniqueness (FR-009). In v1 the descriptor carries no other fields; future iterations will extend it with network-level information (e.g., addresses) and cryptographic material (e.g., public keys), and the identity basis itself may shift (e.g., a key-derived id) without changing the accessor contract.
- **Network (InMemory variant)**: A shared abstraction that routes messages between attached nodes. Conceptually a routing primitive mapping peer identifiers to delivery destinations; the concrete data layout is an implementation concern. Send and receive operations are asynchronous (FR-011) so the abstraction stays shape-compatible with future networked variants.
- **Message**: A discrete unit of communication. The only message kind defined at this stage is `Ping(N)`, where `N` is an opaque numeric value.
- **Config**: A TOML file authored by the user that lists the local node's peer descriptors.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A developer can stand up a two-node test, send a Ping between the nodes, and verify delivery in under 30 seconds of local execution time (excluding build/compile time).
- **SC-002**: A developer can construct a 4-node graph and verify that each Ping reaches exactly its addressed peer with zero deliveries to non-addressed peers across at least 100 sequential sends.
- **SC-003**: Changing the network topology requires only edits to the configuration file(s) — no source code changes — and the running system reflects the new topology on the next node startup.
- **SC-004**: A new contributor can read this spec, the resulting source, and the accompanying test harness, and reproduce the two-node Ping demonstration within one hour without consulting any other document.
- **SC-005**: The receiving side records every delivered Ping with the original `N` value intact, verified across at least 100 sends with varied values.

## Assumptions

- **Trust assumption**: All nodes trust each other at this stage; no admission, authorization, or rate-limit checks apply.
- **Liveness assumption**: All configured peers are reachable and remain up for the duration of the test or demonstration. No failure handling is required.
- **Single-process scope**: The system runs as multiple node instances inside a single process, sharing a single InMemory network instance. Multi-process operation and real network transports are out of scope for this iteration.
- **No cryptography**: No signatures, no key material, no message authentication, no encryption. Adversarial behaviour is out of scope.
- **No protocol semantics beyond connectivity**: The Ping message is a pure connectivity probe; it does not carry topic, sequence, or chain semantics. Those are future iterations.
- **Research context**: Peer discovery and message dissemination protocols are still under research in this workstream. This scaffold deliberately stubs both — peer sets are user-authored, and there is no dissemination algorithm — so engineering can proceed on substrate concerns while research converges on the protocol choices.
- **Configuration trust**: Peer descriptors are authored by a human operator who is trusted to produce a coherent topology (no duplicate identifiers, no references to absent peers beyond what FR-010 already covers).
- **InMemory shape hint (planning input, not a spec requirement)**: The user described the InMemory network conceptually as "a hashmap of peers to message boxes", shared by reference among attached nodes. This concrete shape is a planning-stage suggestion forwarded to `/speckit-plan`; the spec only requires that the abstraction allow in-process message exchange (FR-002).
- **Naming-collision note (informational)**: This `specs/` directory holds Spec Kit feature specifications. Protocol specifications (formal models, papers, design notes) live under `pubsub/docs/` and `pubsub/formal_spec/` and are governed by Constitution Principle V (specifications are read-only to the implementation agent). Feature specs under `specs/` are agent-authored and editable.

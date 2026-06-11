# pubsub-node — Feature Roadmap

**Status**: working document, not authoritative.
**Started**: 2026-05-22 (after 001-minimal-node-scaffold landed).
**Purpose**: capture the next ~10 features in dependency order, the architectural anchors that constrain them, and the per-feature open questions, so that work can proceed one feature at a time without losing thread between sessions.

This roadmap is a meta-spec, not a contract. Features may be re-shuffled, dropped, or restructured as architectural understanding evolves. Each numbered feature ultimately gets its own `/speckit-specify` → `/speckit-plan` → `/speckit-tasks` → `/speckit-implement` cycle.

**Feature numbers are identifiers, not an implementation order.** The list below is roughly dependency-ordered, but the numbers are stable IDs: a feature may be built out of sequence and its `specs/NNN-<short-name>/` directory keeps its ID even when taken early. **In progress as of 2026-06-08, in parallel and ahead of the intervening entries:** **004** (node event-loop refactor + connection model) and **008** (mock topic registry). Their shared boundary — the node event queue — is specified in [`event-loop-and-registry-contract.md`](./event-loop-and-registry-contract.md), which both features' specs cite.

---

## 1. Architectural anchors

These constraints inform the feature decomposition. Some emerged during 001's clarify/analyze walks; others were established during the May 2026 roadmap planning.

### 1.1 Edge vs Golden nodes — configuration, not type

Both edge and golden nodes share the same code. The asymmetry is operational:

- **Both** accept incoming connection requests and **fan out** received messages to their accepted-inbound connections per a configurable policy.
- **Edge nodes** additionally run an **epochal dialer**: at each epoch boundary, pick a configurable number of peers from a topic-scoped view and initiate connections to them.
- **Golden nodes** initially have **no dialer**. They are passive amplifiers: they accept long-lived inbound connections, receive messages from publishers directly (out of band — owner-attested relay pattern), and fan out to their accepted-inbound set.

The mode is a **configuration flag**, not a separate type. The substrate must support both from the start, so `--mode golden` flips the dialer subsystem off without changing any other code path.

Future direction: golden nodes may eventually dial too (e.g., to push messages into the edge overlay rather than waiting for edges to pull). Whatever shape that takes, it should be **indistinguishable on the wire** from an edge node dialing — i.e., it's the same dialer subsystem, just with a different policy.

### 1.2 Connection direction inversion (non-obvious!)

In the edge-node protocol, the connection's initiator is the **receiver**, not the sender:

- Edge A dials B → B accepts → **B forwards its received messages to A** over the connection.
- Edge X dials A → A accepts → **A forwards its received messages to X** over the connection.

So from any node's perspective:

- **Established-outbound connections** (where I dialed) = my **incoming message sources** for this epoch.
- **Accepted-inbound connections** (where someone dialed me) = my **fan-out destinations**.

This inverts the "client-server" intuition where the dialer sends and the acceptor receives. It comes from epochal peer-sampling protocols: at each tick, I pick whose messages I want to hear from, and others reciprocally pick me. Both directions of the relationship are receive-oriented; the edge is a switchboard, not a pipeline.

Implication for the code: the `Connection` abstraction must be symmetric (both sides can send and receive over the same connection — there's no "send-only" or "recv-only" connection), and the **role assignment** (am I forwarding or receiving over this connection?) is determined by **who dialed**, not by the connection itself.

### 1.3 Two distinct configurables: dialer policy + fan-out policy

These are different subsystems with different inputs:

| Subsystem | Input | Output | Edge default | Golden default |
|-----------|-------|--------|--------------|----------------|
| **Dialer** | (peer view, epoch tick) | new connection requests | epochal pick-N from view | none |
| **Fan-out** | (received message, accepted-inbound set) | which connections to forward over | forward to all (per topic) | forward to all (per topic) |

Edge nodes treat **known golden nodes** as a separate table — connections to goldens are persistent (not rotated each epoch), so the dialer policy distinguishes "epochal edge peers" from "persistent golden peers".

### 1.4 Cross-cutting design principles (from 001's convergence walks)

These carry forward into every feature:

- **Parse at the edge.** File I/O / wire decoding happens in CLI / network-edge layers. Core constructors take already-parsed in-memory values. (See `feedback_parse_at_the_edge.md` in saved memory.)
- **Lock in future interface shapes early.** Async I/O, opaque types with accessors, trait-fronted abstractions — pay the small upfront cost so future iterations slot in without rewriting public surfaces. (See `feedback_lock_in_future_interface_shapes.md`.)
- **Mock first, real later.** For crypto, peer source, registry, and transport, ship a trait + a mock impl in the first iteration that needs the concept; the real impl comes as its own feature when research / design has converged.
- **Snapshot observability.** Tests assert against a `received_messages()`-style snapshot, not by inspecting internal state. Established in FR-006.
- **Abstract over identifiers.** `PeerDescriptor` is a trait, not a concrete struct; identity may evolve from string ids to key-derived ids without breaking callers. Established in FR-009.

---

## 2. Feature list (dependency-ordered)

Each entry: ID, name, one-line description, dependencies, whether Constitution Principle II's TDD trigger fires, and the most material open questions for the eventual `/speckit-specify` run.

### Near-term — data shape + protocol bedrock

#### 002 — Topics + topic-subscription filtering

- **What it adds**: `Message` gains a `topic` field; `Node` tracks subscribed topics; the `Node` grows an **incoming-message validation phase** that inspects the topic and silently drops messages for topics it does not subscribe to. The `InMemoryNetwork` stays a dumb pipe — it routes by `peer_id` and never peeks at payload, matching how a real transport (TCP, 009) behaves. Future direction: once connection management lands (004+), receiving an off-topic message may also be grounds for the node to close the connection with the sender (a misbehavior signal, not just a drop).
- **Dependencies**: 001.
- **TDD trigger**: not yet (routing is a data-shape concern, no protocol-behavior claim).
- **Open questions**:
  1. `TopicId` representation: parallel to `PeerId` (newtype around String) or richer (publisher-scoped, namespaced, …)?
  2. Subscription model: static at construction (TOML lists topics) or dynamic via a `Node::subscribe` method?

#### 003 — Message envelope + mock crypto

- **What it adds**: messages grow `(publisher_id, parent_hash, sequence, timestamp, signature)` per the synthesis §2.3 shape. Introduces a `Signer` / `Verifier` trait pair with a `TestSigner` impl that hashes data instead of signing (deterministic, fast).
- **Dependencies**: 002 (envelope wraps a topic-tagged payload).
- **TDD trigger**: **YES.** Chain integrity (parent-hash linkage, sequence monotonicity) and authenticity (signature binding) are protocol-behavior claims. Constitution Principle II → strict red-green TDD from this feature onward.
- **Open questions**:
  1. Where does the publisher's "current chain head" state live? On the Node (subscriber-side: track per `(topic, publisher)` the last-seen `parent_hash`)? In a separate `ChainState` type?
  2. How are equivocation events (two different messages with the same `(publisher, parent_hash)`) surfaced — as a returned `Result` variant, an event on a stream, a log entry?
  3. Mock-crypto contract: `TestSigner` produces a hash of the payload+key — sufficient for differentiating valid vs invalid signatures in tests, but not actually unforgeable. Document that limitation clearly.

### Mid-term — the architectural rewrite + behavior policies

#### 004 — Connection-oriented network model

> **In progress (2026-06-08), taken next in parallel with 008 — led by the node-state event-loop refactor.** The primary deliverable is the refactor: an explicit `NodeState`, a **pure** `apply` transition function returning `Vec<Effect>`, and a single event queue with one consumer and node-owned producers; the connection model below rides on top. The shared seam with 008 is specified in [`event-loop-and-registry-contract.md`](./event-loop-and-registry-contract.md). The structural decisions (pure `apply`/effects, event-queue model) are captured as ADR(s) during this feature's `/speckit-plan`.

- **What it adds**: the substantial architectural shift. Replaces 001's "send by id, network routes" with "open a connection, send over connection". Specifically:
  - `Network::dial(peer_id) -> Result<Connection>` — outbound initiation.
  - `network.incoming() -> impl Stream<Item = Connection>` — inbound acceptor.
  - `Connection` exposes `send`, `recv`, and `peer_id()` (the other end's id).
  - Node has two **separable** subsystems: an **acceptor** (drives `network.incoming()`, hands each new connection to a fan-out manager) and a **dialer** (drives `Network::dial`, hands each established outbound connection to a message-source loop).
  - Acceptor and dialer can be **independently enabled / disabled** — this is the seam that 007 (golden mode) flips.
- **Dependencies**: 003 (connections carry envelope-wrapped, topic-tagged, signed messages — the protocol payload).
- **TDD trigger**: yes (connection lifecycle is protocol behavior).
- **Open questions**:
  1. Connection state machine: explicit (`Requesting → Accepting → Established → Closing → Closed`) or implicit (just `Established / Closed`)?
  2. Where does the "incoming connection request" handshake live? Is acceptance unconditional in v1 (the 001 trust assumption still holds), or is there a deny path?
  3. Connection backpressure: bounded send buffer per connection, or unbounded (matching 001's mailbox decision)?
  4. Connection drops & reconnection — out of scope at this stage? Or part of 004?
  5. How does the receive task (currently a single per-Node loop) generalize to "many concurrent connections each with their own recv stream"?

#### 005 — Peer view + mock peer source

- **What it adds**: replaces 001's static "config-file peer list" with a richer `PeerView` (per-topic set of known peers) backed by a `PeerSource` trait. Initial impl reads the same TOML config. Future impls (registry, sampling service) plug in behind the trait.
- **Dependencies**: 004 (peer view feeds the dialer; without a dialer, there's no consumer of the view).
- **TDD trigger**: yes.
- **Open questions**:
  1. Is `PeerView` mutable from the outside (a runtime API) or only re-read on epoch tick?
  2. Per-topic isolation: one global view, or one view per topic?
  3. Initial mock impl's snapshot model: returns a fixed list, or refreshes from a file on each call?

#### 006 — Epochal dialer + fan-out policies (configurable)

- **What it adds**: the two policy subsystems wired up:
  - **`DialerPolicy` trait** with default impl `EpochalPickN { n, every: Duration }`. The dialer ticks at the configured period, picks `n` peers from the topic view, dials each. Connections established this tick stay open until the next tick replaces them (or until they drop on their own).
  - **`FanoutPolicy` trait** with default impl `ForwardToAcceptedSubset { k }`. On receiving a message, pick `k` of the accepted-inbound connections and forward.
  - Message-id dedup (to prevent infinite forwarding loops) — likely a small `LruCache<MessageId, ()>` per Node.
- **Dependencies**: 004 + 005.
- **TDD trigger**: yes.
- **Open questions**:
  1. Epoch synchronization: each node runs its own local epoch clock (independent), or some global notion (e.g., aligned to wall-clock minutes)?
  2. `MessageId` definition: hash of envelope? `(publisher_id, sequence)`? Affects dedup correctness under equivocation.
  3. Fan-out policy: forward-to-all-accepted vs pick-k. The synthesis suggests both have merit at different scales; the trait makes the choice swappable.

#### 007 — Golden-node mode

- **What it adds**: `--mode {edge,golden}` configuration flag. Wires up:
  - `mode=edge` → `DialerPolicy = EpochalPickN`, `FanoutPolicy = ForwardToAcceptedSubset`.
  - `mode=golden` → `DialerPolicy = None` (no outbound), `FanoutPolicy = ForwardToAccepted` (typically with a wider `k` or `to-all`).
  - **Special-peers table** on edge nodes: a separate table of known golden-node descriptors. The dialer treats these differently from the topic peer view — long-lived connections, not rotated each epoch.
  - Publisher-to-golden message-injection path: golden nodes need a way to receive messages from publishers directly (out of the gossip overlay). Could be the same `Network::dial` (a publisher dials its goldens persistently) — keeps the substrate uniform.
- **Dependencies**: 006.
- **TDD trigger**: yes.
- **Open questions**:
  1. Special-peers table: in the TOML config, in the registry, or in a separate dedicated config block?
  2. Persistent-connection lifecycle: how does an edge node detect a dead golden and reconnect? Heartbeat? Connection-drop signal?
  3. Should publisher-injection use the same `Connection` abstraction, or is it a separate one-way push API?

### Onward — swap mocks for real impls (mostly independent)

#### 008 — Registry abstraction (mock)

> **In progress (2026-06-08), taken next in parallel with 004.** Rescoped for parallel development as a standalone `Registry` trait + `MockRegistry` (write API for topics / authorized publishers / per-topic registered `PeerId`s) plus a **node-owned reader task** that pushes `Event::RegistryUpdate` onto 004's event queue — **decoupled from 007** (golden discovery), which it formerly depended on. The node consumes the registry only via that one event variant. Shared seam with 004: [`event-loop-and-registry-contract.md`](./event-loop-and-registry-contract.md).

- **What it adds**: `Registry` trait + `MockRegistry { topics, authorized_publishers, owner_attested_relays }` impl. Replaces "I know my goldens from CLI config" with "I look up topic T's owner-attested relays from the registry". Mock for now; real on-chain feed is feature 012.
- **Dependencies**: 007 (registry is consumed for golden-node discovery; until then, the static config suffices).
- **TDD trigger**: yes.
- **Open questions**:
  1. Registry refresh cadence: poll on epoch tick, on each new topic, on demand?
  2. Cache semantics: how long is a registry response considered fresh?
  3. Identity binding: does the registry record cryptographic keys, just descriptors, or both?

#### 009 — TCP transport

- **What it adds**: `TcpNetwork` impl behind the same `Network` trait. Unblocks multi-process testing and external deployment. Probably uses `tokio_util::codec::Framed` + a custom codec for the envelope.
- **Dependencies**: 004 (the trait shape).
- **TDD trigger**: yes — wire-format claims are protocol behavior.
- **Open questions**:
  1. Wire format: bincode, protobuf, custom length-prefixed CBOR?
  2. Connection multiplexing: one TCP socket per `Connection`, or multiplex many `Connection`s over one TCP socket per peer pair?
  3. TLS / Noise / plain: scope for this feature, or follow-on?

#### 010 — Real peer-sampling impl

- **What it adds**: replaces `MockPeerSource` with a SecureCyclon-style / Cyclon-style protocol that yields uniformly-random peer samples from the topic-participant set. Research-paced.
- **Dependencies**: 005 (the `PeerSource` trait); 009 (real samples need real reachability).
- **TDD trigger**: yes — this is *deeply* protocol-behavior (Sybil resistance, ring-position invariants).
- **Open questions**: many — see synthesis S-04, S-10, S-11, S-12.

#### 011 — Real crypto (Ed25519)

- **What it adds**: replaces `TestSigner` / `TestVerifier` with `Ed25519Signer` / `Ed25519Verifier`. Public-key handling, key generation, key file format.
- **Dependencies**: 003 (the trait shape).
- **TDD trigger**: yes.
- **Open questions**: key storage on disk, key rotation policy (synthesis §8.7 hash-chain recovery question).

#### 012 — Real on-chain registry feed

- **What it adds**: replaces `MockRegistry` with a real Cardano chain reader (Ogmios / kupo / similar). Reads the on-chain topic registry contract (Plutus/Aiken, currently Quint-modelled).
- **Dependencies**: 008 + the contract being deployed somewhere readable.
- **TDD trigger**: yes.
- **Open questions**: which Cardano client library, mainnet vs preview vs preprod for testing, latency tolerances.

#### 013 — Topic registry (mock)

> **Added 2026-06-11.** The topic-governance counterpart to 008's subscription list — the portion 008 explicitly deferred (008 FR-019: "topic governance is the separate `TopicRegistry`"). A standalone `TopicRegistry` trait + `InMemoryTopicRegistry`, parallel in shape to 008 (push `watch`, `Control`-trait write split, `from_file`) but sharing **no trait** with it (distinct on-chain artifacts per `docs/node-lifecycle/README.md`). Spec dir `specs/013-topic-registry/`.

- **What it adds**: `TopicRegistry` trait + `InMemoryTopicRegistry` recording **which topics legitimately exist** and each topic's **authorized publisher keys** (empty set ⇒ open topic), grounded in `formal_spec/topic_registry/`. A node-owned reader pushes `Event::TopicRegistryUpdate` onto 004's event queue; the node folds a registered-topics + authorized-publishers projection into `NodeState`. Two integration points: (a) a node's **effective subscriptions** become the intersection of its subscription-list topics (008) and the registered topics — subscription-list topics absent from the registry are ignored + logged; (b) inbound messages are dropped if their publisher is not authorized for the topic (open topics accept any). Mock for now; the real on-chain feed is feature 012.
- **Dependencies**: 008 (the subscription-list source it validates against) + 004 (the pure core / event queue it folds into). Both merged.
- **TDD trigger**: yes.
- **Open questions**: is publisher-authorization *enforcement* in this feature or deferred (defaulted in)? a global vs node-scoped topic watch; whether a cross-stream "registries warm" signal is needed before a node accepts traffic. Full governance (RBAC, replication, retention, soft-delete) stays with 012.

---

## 3. Deferred / further future

These have homes in the synthesis but are explicitly out of scope for the near-to-mid roadmap:

- **Replication / catch-up.** Late joiners pulling missed history. Synthesis §6.1. Gap surface S-07, S-08, S-14, I-15, I-19.
- **Anti-spam / rate limiting.** Topic-owner-driven; relay-enforced. Synthesis §3.3.
- **Persistence / archival.** Beyond a small dedup cache.
- **DID-based identity.** Synthesis §2.2 — possible upgrade path if richer identity semantics become valuable.
- **Real network failure handling.** Connection drops, retries, ack-based delivery semantics. Synthesis FR-013 disclaimer (the v1 exactly-once guarantee is explicitly conditional).
- **Decentralized trust-table sharing.** Synthesis §2.2 future direction — topics share off-chain trust-table data over the network with sidecar plugin verification.

---

## 4. Process notes

Lessons from 001's lifecycle (worth preserving):

- **One feature at a time.** Each is its own `/speckit-specify` → `/speckit-clarify` → `/speckit-plan` → `/speckit-tasks` → `/speckit-analyze` → `/speckit-implement` cycle.
- **One session through the design phase; fresh session for `/speckit-implement`.** The pre-spec discussion (if any) flows directly into `/speckit-specify` → `/speckit-clarify` → `/speckit-plan` → `/speckit-tasks` → `/speckit-analyze` in the same Claude session — decisions made during pre-spec stay warm and feed into the spec without artifact re-loading. Restart only for `/speckit-implement`, where the mechanical code-generation work benefits from a clean context focused on the produced artifacts. The 002 lifecycle (commits on 2026-05-29) followed this pattern; the original 001-era "fresh per `/speckit-specify`" guidance came from a feature with no meaningful pre-spec round and doesn't generalize to features that follow a roadmap-driven design discussion.
- **Stay in one session for cross-feature meta-work** (roadmap updates, prioritization decisions across features, workstream-level `IMPLEMENTATION_NOTES.md` edits) — context continuity helps there.
- **Convergence rule.** Each pass of `/speckit-analyze` typically surfaces cascade drifts from the previous pass's edits. After 3 passes the artifact set tends to converge (severity trends 9 → 6 → 5 → 0 on 001). Pass-1 fixes structural issues; pass-2 cleans up cascades; pass-3 is polish; pass-4 is confirmation.
- **Sweep downstream artifacts.** When an FR or task changes, the artifacts that quote or restate the same wording (data-model.md, contracts/, quickstart.md) should be swept in the same commit. SC-004 mandates this for quickstart.md; the implicit rule extends to the others.
- **Saved feedback memories**: `parse-at-the-edge`, `lock-in-future-interface-shapes`, `apply-skill-defaults-silently`. These carry across sessions automatically.

---

## 5. Open meta-questions (not feature-specific)

These influence multiple features and may want their own resolution before the affected feature starts:

1. **Dissemination policy as a separate trait, or threaded through the connection layer?** Affects 006. (My lean: separate trait — cleanest split for golden-vs-edge in 007.)
2. **Epoch-clock source.** Each Node ticks independently, or some synchronized global notion? Affects 006 onward.
3. **Identity model evolution.** Currently `PeerId(String)` opaque. When does it grow to carry a public-key fingerprint? Likely co-evolves with 011 (real crypto).
4. **Topic vs subscription vs "interested in".** Three near-synonyms; pick canonical terminology before 002.
5. **Should goldens publish?** Or are publishers always external entities that golden nodes proxy for? Affects 007 and registry shape.
6. **Self-host the registry-mock in-process for tests vs run a separate test-doubles binary?** Affects 008.

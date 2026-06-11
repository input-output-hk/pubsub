# Feature Specification: Topic Registry (Mock, In-Memory)

**Feature Branch**: `013-topic-registry` (spec directory `013-topic-registry` — feature ID per `specs/ROADMAP.md`)

**Created**: 2026-06-11

**Status**: Draft

**Input**: User description: "A new feature, branched off `main`, for the topic registry. We have the node (subscription) registry for which nodes are subscribed to which topics (008); the topic registry defines which topics are *legitimately registered* (in a real implementation this is an on-chain contract, formally specified in `formal_spec/topic_registry/`). In the prototype this is an in-memory registry similar to `SubscriptionRegistry` but for topics, which also defines authorized publishers. It implies that topics listed in the subscription-list file must reference valid topics registered in the topic registry; any topic not found in the topic registry is invalid and should be ignored (and logged)."

References read before specifying:
- `pubsub-node/specs/event-loop-and-registry-contract.md` — the push/subscribe read model + the node event-queue seam this feature plugs into (a node-owned reader pushing one `Event` variant), mirrored from feature 008. MUST be cited per CLAUDE.md.
- `pubsub-node/specs/008-node-registry/spec.md` — the sibling **subscription list** registry whose shape this feature parallels (distinct trait; same seam idiom). FR-019 of 008 explicitly defers topic governance to a separate `TopicRegistry`; this is that feature.
- `pubsub-node/specs/ROADMAP.md` — the originating "Registry abstraction (mock)" entry (`topics`, `authorized_publishers`) and the on-chain-feed deferral to feature 012.
- `formal_spec/topic_registry/` (READ-ONLY, Principle V) — the authoritative Quint model of the on-chain topic-registry contract: `Topic { name, owners, admins, publishers (empty = open), replicationFactor, retentionPeriod, alive, … }`, ten governance operations, an authorization matrix, fifteen invariants.
- `docs/node-lifecycle/README.md#on-chain-artifacts` + `docs/node-lifecycle/topic-creation.md` (READ-ONLY) — the two distinct on-chain artifacts: **Topic registry** (topic id + authorised publisher keys; read so relayers verify message signatures) vs **Subscription list** (node pubkey + topics + deposit; read so subscribers compute candidate sets).
- `pubsub-node/src/state.rs` (the pure core `apply` + `handle_signed_message` accept path: topic-subscription filter, then signature verification — the two integration points this feature extends) and `pubsub-node/src/message.rs` (`PublisherId`/`PublicKey` — the publisher identity the registry authorizes).
- `pubsub-node/specs/IMPLEMENTATION_NOTES.md` — N-003 (chain-integrity / equivocation / publisher-authorization validation, anticipated under the registry features); this feature delivers the publisher-authorization slice and records what remains deferred to 012.

## Clarifications

### Session 2026-06-11

- Q: Is authorized-publisher enforcement on the inbound message path in scope for feature 013 (drop unauthorized), or should it be log-only / deferred? → A: **In scope — enforce by dropping.** A signed message on an effectively-subscribed topic whose publisher key is not in that topic's non-empty authorized set is dropped (open topics — empty set — accept any), checked before signature verification. This is the faithful realisation of the topic registry's node-facing purpose (relayers verify signatures against authorised publisher keys). Confirms US3 / FR-015 as written; the publisher-authorization slice of IMPLEMENTATION_NOTES N-003 is closed here, with equivocation / sequence / deposit still deferred to 012.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — A Topic Registry of Legitimate Topics and Their Publishers (Priority: P1) 🎯 MVP

A consumer opens a watch on the topic registry and is told, as a cold-start burst, every **currently-registered topic** together with that topic's **set of authorized publisher keys** (an empty set meaning the topic is *open* — any publisher may publish). It then receives every subsequent change: a new topic registered, a topic's authorized-publisher set changed, or a topic removed. The registry is the source of truth for which topics legitimately exist.

**Why this priority**: The irreducible deliverable — a live view of legitimate topics and their authorized publishers, derived from one stream, exercisable against the registry alone with no node event loop. The safe MVP slice and the foundation both node-integration stories build on.

**Independent Test**: Construct an `InMemoryTopicRegistry` (from a file or programmatically) with topics `weather → {pubK1}`, `sports → {pubK1, pubK2}`, `chat → {}` (open). Open a watch; assert the cold-start burst reports all three topics with their publisher sets (and that `chat` is present with an empty set — *open*, not *absent*). Then register `news → {pubK3}` → one live `Registered`; change `weather` publishers to `{pubK1, pubK4}` → one `PublishersChanged { added: {pubK4} }`; remove `sports` → one `Removed`.

**Acceptance Scenarios**:

1. **Given** a registry with `weather → {pubK1}`, `sports → {pubK1, pubK2}`, `chat → {}`, **When** a consumer opens the watch, **Then** the cold-start burst yields a `Registered` for each topic carrying its authorized-publisher set, with `chat` present and its publisher set empty (open topic).
2. **Given** an open watch whose burst is drained, **When** topic `news` is registered with `{pubK3}`, **Then** the watch yields exactly one `Registered { topic: news, publishers: {pubK3} }`.
3. **Given** the same open watch with `weather → {pubK1}` registered, **When** `weather`'s publishers are set to `{pubK1, pubK4}`, **Then** the watch yields exactly one `PublishersChanged { topic: weather, added: {pubK4}, removed: {} }`.
4. **Given** the same open watch, **When** `sports` is removed, **Then** the watch yields exactly one `Removed { topic: sports }`.
5. **Given** `weather → {pubK1}` registered, **When** the same set `{pubK1}` is re-applied, **Then** **no** event is emitted (idempotent).
6. **Given** a topic registered as open (`chat → {}`), **When** the same open set `{}` is re-applied, **Then** no event is emitted; `chat` stays registered-and-open, distinct from being removed.

---

### User Story 2 — A Node Only Subscribes to Registered Topics (Priority: P2)

A node derives its topics from its own subscription-list entry (feature 008). Those topics are now **validated against the topic registry**: a topic in the node's subscription-list entry that is **not** a registered topic is **invalid** — the node does not treat itself as subscribed to it and emits an operator log line recording the ignored topic. Only the intersection — the node's subscription-list topics that are also registered — becomes the node's effective subscription set (the message accept-filter). Because both the topic registry and the subscription list are live streams, the effective set is re-evaluated as either changes: a subscription-list topic that is not yet registered becomes effective once it *is* registered, and ceases to be effective if its topic is later removed.

**Why this priority**: This is the explicit cross-registry guarantee the feature exists to provide — a node cannot participate in topics that do not legitimately exist, even if its subscription-list entry (or a misconfigured mock file) names them. Builds on 008's membership stream and feature 004's pure core.

**Independent Test** (pure, synchronous): Construct `NodeState` for self-id `S`. Feed scripted events: a topic-registry burst registering only `weather`; a membership self-`Joined { S, {weather, ghosttopic} }`. Assert `S`'s effective subscriptions become `{weather}` only (`ghosttopic` ignored, not registered). Then feed `Registered { ghosttopic, {} }`; assert `ghosttopic` becomes effective. Then feed `Removed { weather }`; assert `weather` is no longer effective. Assert every `apply` returns an empty `Vec<Effect>`. Separately, assert a message on `ghosttopic` is dropped while `ghosttopic` is unregistered and accepted once it is registered (subscription + signature otherwise valid).

**Acceptance Scenarios**:

1. **Given** a topic registry where `weather` is registered and `ghosttopic` is not, and a node whose subscription-list entry is `{weather, ghosttopic}`, **When** the node converges, **Then** its effective subscription set is `{weather}`; `ghosttopic` is ignored and recorded in an operator log line (cause: topic not registered).
2. **Given** the node from AS-1 (`ghosttopic` ignored), **When** `ghosttopic` is later registered in the topic registry, **Then** the node's effective subscription set becomes `{weather, ghosttopic}` without restart.
3. **Given** a node effectively subscribed to `{weather}`, **When** `weather` is removed from the topic registry, **Then** `weather` leaves the effective subscription set and subsequent messages on `weather` are dropped.
4. **Given** a node whose subscription-list entry is `{weather}` and `weather` registered, **When** a validly-signed message on `weather` arrives from an authorized publisher, **Then** it is accepted (validation is additive — registered-and-subscribed topics behave exactly as in 008).

---

### User Story 3 — A Node Rejects Messages From Unauthorized Publishers (Priority: P3)

When a node receives a signed message on a topic it is effectively subscribed to, it checks the message's **publisher** against the topic registry's authorized-publisher set for that topic. If the topic is *open* (empty authorized-publisher set) any publisher is accepted; otherwise the publisher's key MUST be in the authorized set. A message from a publisher not authorized for its topic is dropped (operator log cause: publisher not authorized), before the cost of signature verification is paid. This realises the topic registry's stated purpose — relayers verify message signatures against the registry's authorised publisher keys.

**Why this priority**: This is what makes the authorized-publisher data the registry holds *meaningful* to the node (a registry of publishers with no consumer would be unjustified surface). It composes US1 (the publisher sets) with the existing accept path. Lower priority than topic validity because it narrows an already-narrowed (subscribed + registered) stream.

**Independent Test** (pure, synchronous): Construct `NodeState` for `S` effectively subscribed to `weather` with `weather`'s authorized publishers folded as `{pubK1}`. `apply` a validly-signed message on `weather` from `pubK1` → recorded. `apply` a validly-signed message on `weather` from `pubK2` (not authorized) → dropped, not recorded, no effects. Then fold the topic to *open* (`{}`); `apply` the `pubK2` message again → now recorded. Assert the unauthorized-publisher drop happens regardless of signature validity (a valid signature from an unauthorized publisher is still dropped).

**Acceptance Scenarios**:

1. **Given** a node effectively subscribed to `weather` whose authorized publishers are `{pubK1}`, **When** a validly-signed message on `weather` from `pubK1` arrives, **Then** it is recorded.
2. **Given** the same node, **When** a validly-signed message on `weather` from `pubK2` (not in the authorized set) arrives, **Then** it is dropped (publisher not authorized) and not recorded.
3. **Given** `weather` registered as **open** (empty authorized-publisher set), **When** a validly-signed message on `weather` from any publisher arrives, **Then** it is recorded (open topics accept any publisher).
4. **Given** a message whose claimed publisher is unauthorized for its topic, **When** it is processed, **Then** it is dropped on the authorization check **without** the signature being verified (the authorization check is cheap and precedes verification).

---

### User Story 4 — A Network of Nodes Shares One Topic Registry (Priority: P4)

Several in-memory nodes are brought up against the **same topic registry** (the mocked on-chain contract) alongside the shared subscription registry from 008. Single-process: they share one `InMemoryTopicRegistry` `Arc`. Each node validates its subscription-list topics against the shared topic registry and enforces authorized publishers uniformly. The network agrees on which topics are legitimate and who may publish, with no operator intervention beyond the shared registries.

**Why this priority**: The end-to-end demonstration that the two registries compose into a usable multi-node test fixture. Lowest priority; composes US1–US3 with 008's US4.

**Independent Test**: A topic-registry source registers `weather → {pubK1}` and `sports → {}` (open); a subscription-list source lists `node-a → {weather}`, `node-b → {weather, sports}`, `node-c → {weather, ghosttopic}`. Single-process: share both `Arc`s across three nodes configured by `node_id` only. Start all three; assert each node's effective subscriptions are the registered subset of its subscription-list entry (`node-c` drops `ghosttopic`), and that a message on `weather` from `pubK1` is accepted by all subscribers while a message on `weather` from a non-`pubK1` publisher is dropped by all.

**Acceptance Scenarios**:

1. **Given** three nodes sharing one topic registry and one subscription registry, configured by `node_id` only, **When** all start, **Then** each node's effective subscription set is the intersection of its subscription-list topics and the registered topics (unregistered topics dropped per node).
2. **Given** the started network with `weather → {pubK1}`, **When** a node publishes-equivalent (a message arrives) on `weather` from `pubK1`, **Then** every subscriber to `weather` accepts it; **When** the same arrives from a non-authorized publisher, **Then** every subscriber drops it.
3. **Given** the started network, **When** a new topic `news → {pubK1}` is registered, **Then** nodes whose subscription-list entries include `news` begin accepting `news` traffic without restart.

---

### Edge Cases

- **Open topic vs absent topic**: a topic registered with an **empty** authorized-publisher set is *open* (any publisher accepted) and is a **valid, subscribable** topic; a topic that is **not registered at all** is invalid (subscriptions to it are ignored, messages on it dropped). The two are distinct, mirroring 008's empty-topics-vs-unregister distinction.
- **Unchanged re-register**: re-applying a topic's current authorized-publisher set emits no event (idempotent), including re-applying an empty (open) set.
- **Subscription-list topic not registered**: ignored (not added to the effective subscription set) and logged once; not an error, not a construction failure (parallels 008's missing-entry posture — converge from the stream, no fail-fast).
- **Topic registered after the node subscribes to it**: the node begins treating it as effective when the `Registered` event arrives (dynamic; both registries are live streams). No restart required.
- **Topic removed while subscribed**: the topic leaves the effective subscription set; in-flight and subsequent messages on it are dropped.
- **Publisher authorization changes**: a publisher removed from a topic's authorized set stops being accepted on that topic from the `PublishersChanged` event onward; a publisher added begins being accepted.
- **Message on a registered, subscribed topic from an unauthorized publisher**: dropped (publisher not authorized) before signature verification.
- **Cross-stream ordering**: topic-registry events and membership events arrive on two independent streams folded into one `NodeState`; the effective subscription set and authorization decisions reflect whichever facts have been folded so far, and converge once both streams have delivered the relevant events. No explicit cross-stream barrier (no "registries warm" signal) in v1.
- **No cold-start boundary marker**: like 008, the topic-registry cold-start burst has no explicit end-of-snapshot signal; the `#[non_exhaustive]` event enum leaves room for one if a future consumer needs it.
- **Channel overflow / slow consumer**: v1 watch is unbounded (ADR 0007) — no backpressure; deferred.
- **Watch dropped**: ends that subscription cleanly, no effect on registry state or other watches (mirrors `NetworkHandle` / `MembershipWatch`).

## Requirements *(mandatory)*

### Functional Requirements

#### Domain boundary and types

- **FR-001**: A public trait `TopicRegistry` MUST be defined as the **read-only, node-facing** interface between the pubsub node and the topic-registry source, exposing a **single** method `watch` (FR-007) — no point-read. `Send + Sync + 'static`; `watch` MUST be RPITIT returning `impl Future<Output = Result<TopicRegistryWatch, TopicRegistryError>> + Send` (the `Send`-bounded shape ADR 0007 flags for `async fn` in traits, required because the node-owned reader awaits it in a spawned task). It MUST be the sole anti-corruption boundary — no impl's storage or wire encoding may leak through it. The `Node` MUST depend only on this read trait, consumed **generically** (`Node::new<…, T: TopicRegistry>(…, Arc<T>)`, not `Arc<dyn>`: `async fn` traits are not `dyn`-compatible, so it is taken generically as `Network` and `SubscriptionRegistry` are), so the node has **no write methods in scope**. It MUST be **distinct** from `SubscriptionRegistry` (008); the two MUST NOT share a trait (different keys, payloads, and readers — `docs/node-lifecycle/README.md`).
- **FR-002**: A public `TopicRegistryEvent` enum MUST express one topic-registry delta: `Registered { topic: TopicId, publishers: <set of PublicKey> }`, `PublishersChanged { topic: TopicId, added: <set of PublicKey>, removed: <set of PublicKey> }`, `Removed { topic: TopicId }`. An **empty** `publishers` set means the topic is **open** (any publisher authorized). Each carries identity + authorization only — no `replicationFactor`, `retentionPeriod`, owners, or admins (those are on-chain governance fields not consumed by the node here). MUST be `#[non_exhaustive]`. `TopicId`/`PublicKey` are reused from the crate, not redefined.
- **FR-003**: Types that decode/serialize on-chain topic-registry transactions or governance operations (create/delete topic, role grants, publisher-key rotation) MUST be **module-internal** (`pub(crate)`/private), not in the trait surface or `TopicRegistryEvent` (parse-at-the-edge). The in-memory impl has no wire format yet; the boundary is fixed for the on-chain reader (012).

#### Topic source — the topic-registry file (mocked chain)

- **FR-004**: `InMemoryTopicRegistry` MUST provide a `from_file(path)` constructor that loads a **topic-registry file** — read **only** by the topic-registry module, **separate** from 008's subscription-list file — mapping each topic id to its authorized-publisher key set. The format MUST follow the crate's existing TOML config convention (e.g. `[[topic]]` tables with `id` and an optional `publishers` list of public keys; an absent or empty `publishers` list means an open topic), with strict unknown-field rejection per the 001 config policy. Governance fields (`owners`, `admins`, `replication_factor`, `retention_period`), if present, MUST be ignored (out of scope). The loaded entries are the registry's initial set of legitimate topics.

#### Write side — a separate `TopicRegistryControl` trait (operator / test harness, NOT the node)

The write surface MUST live on a **separate** trait `TopicRegistryControl: TopicRegistry`, NOT on the node-facing `TopicRegistry`, mirroring 008's `SubscriptionRegistryControl` split: this keeps the node-facing domain interface free of write/test signatures, matches the read-only node, and reflects that the real on-chain reader (012) implements only the read trait (chain writes are transactions). `InMemoryTopicRegistry` implements both traits; test/operator-sim code holds the concrete `Arc<InMemoryTopicRegistry>` to drive it.

- **FR-005**: `TopicRegistryControl` MUST expose a declarative idempotent upsert of a topic's authorized publishers — `async fn set_topic(&self, topic: TopicId, publishers: <set of PublicKey>) -> Result<(), TopicRegistryError>` (first registration → `Registered`; changed publisher set → one `PublishersChanged { added, removed }`; unchanged → no-op). In production this models an operator/owner on-chain governance transaction; in this feature it is called only by the file loader's equivalent and by test harnesses — **never by the node daemon**.
- **FR-006**: `TopicRegistryControl` MUST expose `async fn remove_topic(&self, topic: TopicId) -> Result<(), TopicRegistryError>` — removes the topic from the registry entirely; observers see `Removed`. Distinct from `set_topic(topic, {})` (which registers/retains the topic as *open*). (The on-chain contract's `alive` soft-delete semantics — retaining the topic id forever to prevent reassignment — are deferred to 012; the mock removes the entry.)

#### Read side (push / watch)

- **FR-007**: `TopicRegistry` MUST expose `fn watch(&self) -> impl Future<Output = Result<TopicRegistryWatch, TopicRegistryError>> + Send` — the **single** read method. Unlike 008's node-keyed `watch(node)`, this watch is **global** (the node may need to validate any topic and authorize publishers on any subscribed topic); no scoping argument. `TopicRegistryWatch` MUST mirror `MembershipWatch`/`NetworkHandle`: single-consumer, owns the receive half, not `Clone`, ends on drop. On open it MUST replay a cold-start burst of `Registered` events — one per currently-registered topic, carrying its authorized-publisher set — and then stream live deltas. Event-driven (push), not poll/diff. The burst and live deltas MUST form one gap-free, duplicate-free sequence (snapshot capture + subscriber registration atomic under the lock).
- **FR-008**: The v1 watch channel MUST be **unbounded** (ADR 0007) — no backpressure, no lag/skip event; deferred. `#[non_exhaustive]` `TopicRegistryEvent` leaves room for a future warmth/lag signal.

#### Implementation and errors

- **FR-009**: A concrete `InMemoryTopicRegistry` MUST be the first impl: in-process, holding the topic→publishers state + live subscriber channels behind private interior synchronisation; public struct, private internals; `InMemorySubscriptionRegistry`-style naming and structure (`new()`, `from_file`, `Default`). It MUST be shareable across multiple `Node`s via `Arc` (single-process multi-node topology, US4), as `InMemorySubscriptionRegistry` is.
- **FR-010**: A `TopicRegistryError` typed enum MUST back the fallible methods (`std::error::Error` + `Debug` + `Display`; `#[non_exhaustive]`). In-memory methods do not fail under normal operation; the variant set is minimal now and grows with the on-chain backend (012). File-load failures (FR-004) surface through the existing config/IO error path (`ConfigError`), not necessarily this enum (mirrors 008).

#### Node integration — read-only, on feature 004's pure core + 008's membership fold

- **FR-011**: The `Event` enum (`src/event.rs`) MUST gain `Event::TopicRegistryUpdate(TopicRegistryEvent)`. The variant + payload are owned by this feature; the `apply` arm is one dispatch line in `pub(crate) fn apply` plus a named `handle_topic_registry_update(state, update) -> Vec<Effect>` handler (ADR 0011 convention), existing arms untouched.
- **FR-012**: A node-owned producer MUST open `watch()`, drain the `TopicRegistryWatch`, and push one `Event::TopicRegistryUpdate(ev)` per event, registered via `spawn_producer` (node owns the `JoinHandle`, aborts on drop; symmetric with `network_mailbox_loop` and 008's membership reader). The node MUST consume the topic registry only through this one stream and MUST issue no registry writes. If the watch cannot be opened, the reader MUST log at `error` (operator UX) and the node stays at empty topic-registry-derived state (no registered topics → no effective subscriptions).
- **FR-013**: `apply` (via `handle_topic_registry_update`) MUST fold topic-registry deltas into crate-internal `NodeState`: the set of registered topics and, per registered topic, its authorized-publisher key set (empty = open). `Registered` adds/replaces a topic's publisher set; `PublishersChanged` applies the add/remove diff; `Removed` drops the topic. Pure (no I/O, no `.await`), returns an empty `Vec<Effect>` (`Effect` uninhabited).
- **FR-014**: A node's **effective subscription set** (the message accept-filter) MUST be the **intersection** of (a) its subscription-list topics (folded from 008's membership stream — the node's own entry) and (b) the registered topics (folded from this feature's stream). A subscription-list topic that is not a registered topic MUST be excluded from the effective set and recorded with an operator log line (cause: topic not registered) — it is **ignored**, not an error, and does **not** fail construction. The effective set MUST be re-evaluated as either stream changes (a topic registered later becomes effective; a topic removed later stops being effective), without restart. (Whether this is represented as two folded sets ANDed at accept time or a separately-maintained derived set is a design decision for `/speckit-plan`; the observable behaviour is the intersection.)
- **FR-015**: On the inbound signed-message path (`handle_signed_message`), after the effective-subscription check (FR-014) and **before** signature verification, the node MUST enforce authorized publishers: if the message's topic has a non-empty authorized-publisher set, the message's publisher key MUST be in that set, else the message is dropped with an operator log line (cause: publisher not authorized). A topic with an **empty** authorized-publisher set (open) accepts any publisher. The existing topic-subscription and signature-verification checks are retained; this check is **additive** and ordered before verification (cheap set lookup precedes expensive verification, consistent with the existing "filter first" ordering).
- **FR-016**: The topic-registry integration MUST NOT change the existing accept behaviour for the in-scope-and-authorized case: a message on a registered, effectively-subscribed topic from an authorized (or any, if open) publisher with a valid signature is recorded exactly as in 008/003. Off-topic, unregistered-topic, unauthorized-publisher, and invalid-signature messages are each dropped with their distinct logged cause.

#### Scope boundaries

- **FR-017**: This feature MUST NOT implement topic-registry **governance**: role-based access control (owners/admins), authorization of *who may create/delete topics or grant roles*, publisher-key rotation, `replicationFactor`, `retentionPeriod`, epochs, or the `alive` soft-delete semantics. The mock write surface (FR-005/FR-006) is a minimal topic+publishers upsert/remove for the loader and tests; the full ten-operation contract is the on-chain feature 012. The node-consumed projection is registered-topics + authorized-publishers only.
- **FR-018**: This feature MUST NOT implement full chain-integrity validation beyond publisher authorization — message equivocation, parent-hash chaining, per-publisher sequence monotonicity, and deposit/anti-Sybil enforcement remain deferred (IMPLEMENTATION_NOTES N-003); this feature delivers only the publisher-authorization slice of N-003 and MUST update N-003 to record what it closed and what remains.

#### Testing discipline

- **FR-019**: The topic-registry module MUST be verifiable **in isolation** (construct/`from_file`, write via `TopicRegistryControl`, `watch`; assert on emitted `TopicRegistryEvent`s + the cold-start burst). Node integration MUST be testable as a **pure** state-machine exercise (scripted `Vec<Event>` mixing `TopicRegistryUpdate` and `MembershipUpdate` through `apply`; assert the effective subscription set, publisher-authorization accept/drop, and that every `apply` returns an empty `Vec<Effect>`). The topic-validity invariant (a node never effectively subscribes to an unregistered topic) and the authorized-publisher invariant MUST each be tested. The multi-node topology (US4) is testable over shared `Arc`s with getter polling to steady state. Tests MUST assert on events/state/snapshots — **never** on log content (Constitution).

### Key Entities

- **TopicRegistry** (read trait, node-facing): the published, read-only anti-corruption boundary the `Node` depends on. **Single** method: `watch()` (global; no scoping, no point-read). Distinct from `SubscriptionRegistry`. The 012 chain reader implements this trait.
- **TopicRegistryControl** (write trait, `: TopicRegistry`): the operator/test write surface — `set_topic`, `remove_topic`. The node never depends on it; `InMemoryTopicRegistry` implements it, and the file loader + test/operator-sim code drive the registry through it.
- **Topic-registry file**: the mocked on-chain topic registry — a TOML file (`topic id → authorized publishers`) read only by the topic-registry module, separate from the subscription-list file; loaded via `InMemoryTopicRegistry::from_file`. The source of truth for which topics legitimately exist.
- **TopicRegistryEvent** (`#[non_exhaustive]`): one topic-registry delta — `Registered`/`PublishersChanged`/`Removed`, topic id + authorized publishers (empty = open). Payload of `Event::TopicRegistryUpdate`.
- **TopicRegistryWatch**: single-consumer subscription handle (mirrors `MembershipWatch`) over an unbounded channel; not `Clone`; ends on drop. Global cold-start `Registered` burst, then live deltas.
- **InMemoryTopicRegistry**: v1 concrete impl; holds topic→publishers state + live subscriber channels; `from_file` + programmatic writes; shareable via `Arc`.
- **TopicRegistryError** (`#[non_exhaustive]`): typed error for fallible methods; minimal now.
- **Authorized-publisher set** (per topic, `PublicKey`s): empty = open topic. Folded into `NodeState`; consulted on the inbound message path.
- **Registered-topics projection** (in `NodeState`): the set of legitimate topics + their authorized publishers, folded from the topic-registry stream; intersected with the membership-derived subscription topics to form the effective subscription set.
- **Effective subscription set**: the intersection of subscription-list topics (008) and registered topics (this feature); the actual message accept-filter.
- **Event::TopicRegistryUpdate**: the single seam the node consumes the topic registry through.
- **PublicKey / TopicId / PublisherId** (reused): publisher and topic primitives the registry keys on; `PublisherId` wraps the `PublicKey` checked against a topic's authorized set.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A consumer opening the topic-registry watch with `N` pre-existing registered topics receives all `N` (with their authorized-publisher sets) in the cold-start burst before any later change (cold-start completeness).
- **SC-002**: A topic-registry change (register / publishers-changed / remove) on the registry is observed exactly once by every current watcher.
- **SC-003**: A node never effectively subscribes to a topic that is not registered in the topic registry, for any interleaving of subscription-list and topic-registry events (topic-validity invariant).
- **SC-004**: A subscription-list topic that is unregistered at startup but registered later becomes effective without a node restart; a topic removed later ceases to be effective.
- **SC-005**: A message on a registered, effectively-subscribed topic from a publisher **not** in that topic's non-empty authorized-publisher set is dropped and never recorded (authorized-publisher invariant); a message on an **open** topic from any publisher with a valid signature is recorded.
- **SC-006**: Re-applying a topic's unchanged authorized-publisher set (including an unchanged empty/open set) produces zero topic-registry events.
- **SC-007**: The topic registry's write / `from_file` / `watch` behaviour is fully verified without the node event loop.
- **SC-008**: In a multi-node network sharing one topic registry, every node enforces the same legitimacy and authorization decisions (same effective-subscription intersection per its own subscription-list entry; same accept/drop per publisher).
- **SC-009**: A running node performs zero topic-registry writes; the topic-registry projection never alters the config bootstrap `peers` field or the subscription-list-derived membership data.
- **SC-010**: For an in-scope, registered, subscribed topic with an authorized publisher and a valid signature, message-acceptance behaviour is byte-identical to the pre-013 (008/003) behaviour (validation is additive, no regression).

## Assumptions

- **Feature 008 has merged** (PR #51): `SubscriptionRegistry`/`InMemorySubscriptionRegistry`, the `MembershipWatch` push model, `Event::MembershipUpdate`, the node-owned registry reader (`spawn_producer`), and the registry-derived subscription set + candidate sets in `NodeState` all exist on `main` (ADR 0013/0014/0015). This feature parallels that shape for topics and **extends** the same `NodeState`/`apply` core (feature 004, ADR 0011/0012).
- **Two distinct registries, no shared trait.** Per `docs/node-lifecycle/README.md`, the topic registry (topic id + authorised publisher keys; read so relayers verify signatures) and the subscription list (node pubkey + topics + deposit; read to compute candidate sets) are separate on-chain artifacts with different keys, payloads, and readers. `TopicRegistry` and `SubscriptionRegistry` therefore share no trait — only the event-queue seam idiom (a node-owned reader pushing one `Event` variant).
- **The topic registry is the source of truth for which topics legitimately exist**, both in production (on-chain) and in the mock (the topic-registry file). A node's subscription-list topics are validated against it; config supplies neither topics nor topic legitimacy.
- **Open topic = empty authorized-publisher set**, per the formal spec (`publishers` empty ⇒ open). A registered open topic is valid and subscribable; an unregistered topic is invalid.
- **Authorized publishers are keyed by `PublicKey`**, matching the message `publisher_id` (`PublisherId` wraps `PublicKey`) and the formal spec's `publishers: Set[PublicKey]`. Endpoint/identity binding beyond the public key is out of scope.
- **Publisher-authorization enforcement is in scope** (US3), confirmed via clarification (Clarifications 2026-06-11): the node enforces by **dropping** unauthorized-publisher messages. It is the topic registry's stated node-facing purpose and the justification for carrying publisher sets through the interface (forward-compatible-interface standard). Full chain-integrity / equivocation / sequence / deposit validation stays deferred (N-003).
- **In-memory, single source via file or shared `Arc`.** No network, chain, or persistence; unbounded channel per ADR 0007; runtime cross-process churn (file re-read / poll) deferred to 012.
- **Reused**: the 002/008 message-filter mechanism (now intersected with the registered-topics projection), the 003 signature-verification accept path (now preceded by the authorization check), the `Network`/008 actor-handle and node-owned-reader idioms, and the 001 strict-TOML config convention for the topic-registry file.
- **Deferred with owners**: topic governance / RBAC / replication / retention / soft-delete / epochs → topic-registry contract (012); real on-chain feed → 012; chain-integrity / equivocation / sequence / deposit → N-003 (012); bounded-channel backpressure → real transport.
- **A structural decision record (ADR) MUST be authored** during `/speckit-plan` for the topic-registry interface + node integration (the `TopicRegistry`/`TopicRegistryControl` split, the global vs node-keyed watch choice, the effective-subscription-intersection model, and the authorized-publisher accept-path ordering), per Constitution Principle III.

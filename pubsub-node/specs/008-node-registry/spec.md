# Feature Specification: Subscription Registry (Mock, In-Memory)

**Feature Branch**: `feat/node-registry` (spec directory `008-node-registry` — feature ID per `specs/ROADMAP.md`)

**Created**: 2026-06-09

**Status**: Draft

**Input**: User description: "A mock, in-process node-membership registry (feature 008) implementing the shared seam in `specs/event-loop-and-registry-contract.md` (§2, push/subscribe read model). Domain boundary: a public `Registry` trait + `RegistryEvent` (`Joined`/`TopicsChanged`/`Left`, carrying `PeerId` + `TopicId`s only — no network address, no deposit) form the published language the node consumes; any on-chain serialization/decoding for join/leave/change is internal to the module (parse-at-the-edge), not built in this feature. Write API: `set_interest(node, topics)` (declarative idempotent upsert; registry diffs to emit Joined/TopicsChanged) and `unregister(node)` (emits Left). Read API: `watch_members(topics) → RegistryWatch` — replays current members as a `Joined` cold-start burst, then streams live deltas; mirrors the `Network::register` actor-handle idiom; unbounded channel (no backpressure/Lagged yet). First impl: `InMemoryRegistry`. Node integration: a node-owned reader (`spawn_producer`) drains the watch and pushes `Event::RegistryUpdate(RegistryEvent)`; `apply` folds into `NodeState`, filtering the node's own `PeerId` locally. Coupling with 002: the local subscription set is the source of truth and drives `set_interest`, making `subscribe`/`unsubscribe` async + fallible. Startup: the node announces interest unconditionally via `set_interest` (idempotent); no recovery/restore of prior registration state (deferred to on-chain, 012). Non-goals: topic-governance writes (create topic, authorized publishers — deferred to ~007), address resolution (gossip), deposit/anti-Sybil, peer sampling (010), restart recovery, backpressure.

References to read before specifying:
- pubsub-node/specs/event-loop-and-registry-contract.md §2 (the push/subscribe read model + the §3 seam this feature plugs into) — MUST be cited per CLAUDE.md.
- pubsub-node/specs/ROADMAP.md feature 008 (originating roadmap entry; the node-membership reframing and the topic-governance deferral to ~007).
- pubsub-node/specs/IMPLEMENTATION_NOTES.md (deferred-revisit ledger; this feature adds an entry for restart-time registration-state recovery under a persistent/on-chain registry).
- pubsub-node/src/network.rs (the `Network` trait + `NetworkHandle` actor-handle idiom + ADR 0006/0007 unbounded-channel choice the read side mirrors).
- pubsub-node/src/peer.rs (`PeerId`, `PeerDescriptor`/`BasicPeerDescriptor` — the identity primitive the registry keys on; the documented extension point for address/pubkey at a later layer).
- pubsub-node/specs/002-topic-subscription-filtering/ (the local subscription set this feature couples `set_interest` to)."

## Clarifications

### Session 2026-06-10 (post-merge review + source-of-truth design)

The draft `Input` above was reviewed against three things that merged to `main` after it was written (PR #44 node-lifecycle docs, #49 event-queue seam, #50 feature 004), and then refined to fix a source-of-truth flaw. The original `Input` is retained verbatim for provenance; the body below reflects the final design. Decisions:

- **Naming → `SubscriptionRegistry` (not `Registry`).** The protocol (`../docs/node-lifecycle/README.md`) defines two distinct on-chain artifacts: a **Topic registry** (topic id + authorised publisher keys; read by relayers to verify signatures) and a **Subscription list** (node pubkey + topic-interest set + deposit; read by subscribers to compute candidate sets). This feature is the **subscription list**. They share neither key, payload, nor reader, so no shared trait: this feature defines `SubscriptionRegistry` (+ `InMemorySubscriptionRegistry`); the topic registry stays a separate future type (~007/012).
- **The subscription list is the single source of truth for a node's own interests — NOT config.** A node's authoritative topic-interest set is its entry in the subscription list, not a locally-editable config value. Otherwise an operator could make a node participate in topics beyond its registered (deposited) commitment, defeating the deposit's accountability. The node therefore **looks up its own entry** in the registry to determine its interests. (This resolves an ambiguity in `joining.md`, which lists the topic-interest set in *both* node config (step 4) and on-chain (steps 2–3) without stating which is authoritative at runtime — surfaced as an issue/ADR per Principle V.)
- **The mock's source of truth is a subscription-list file.** `InMemorySubscriptionRegistry::from_file(path)` loads a dedicated file — read **only** by the registry module — that maps `node_id → topic-interest set` (the mocked on-chain subscription list). This file is the membership truth.
- **The node is STRICTLY read-only; no self-seed.** Registration / interest-change / leave are operator actions: in production an on-chain transaction (`joining.md`: "the node does NOT initiate a registration transaction"); in this mock, entries in the subscription-list file and/or programmatic `set_interest` calls by the test harness. The node performs **zero** registry writes — it reads its own entry, watches its topics, folds candidate sets.
- **002's `subscribed_topics` config field is removed.** The node no longer self-declares topics. Its interest set is sourced from its registry entry and seeds the (retained) 002 subscription-set/message-filter mechanism. Node config now carries **identity (`node_id`) + bootstrap peers** only.
- **Seam variant → `Event::MembershipUpdate(MembershipEvent)`** (renamed from the `RegistryUpdate` placeholder anticipated by ADR 0011/CLAUDE.md — needs a heads-up to the 004 author + a one-line update to that comment and the CLAUDE.md SpecKit block when this lands).
- **Node integration targets feature 004's merged pure core.** Crate-internal `pub(crate) fn apply(&mut NodeState, Event) -> Vec<Effect>` (ADR 0011) gains one dispatch line + a named `handle_membership_update` handler; candidate sets live in crate-internal `NodeState`, exposed via a public `Node` getter; `Effect` is uninhabited so the handler returns an empty `Vec`.
- **Candidate set vs config `peers` → coexist (resolves N-007).** Config `[[peers]]` is the **bootstrap** set for the future dialer; the per-topic candidate set is the **interest-derived** set folded from the registry. Distinct sources, distinct roles. This feature adds the candidate set to `NodeState`, does not touch `peers`, and does not connect/dial (deferred to the dialer, ~006 / `004-connections`).

Resolved via `/speckit-clarify` (same session):

- Q: Should the cold-start burst carry an explicit end-of-snapshot boundary marker? → A: No — implicit "burst then deltas"; keep the 3-variant `#[non_exhaustive]` enum and add `SnapshotComplete` later only when feature 010's sampler needs a "warm" signal.
- Q: What does a node do if its `node_id` has no subscription-list entry at startup? → A: Fail fast (hard startup/config error); the protocol's wait-and-retry-with-backoff is deferred to 012.
- Q: Does the node react to runtime changes in its own subscription-list entry? → A: No — interest set is fixed at startup (read once via `entry`); changes to the node's own entry take effect only on restart; dynamic re-derivation deferred to 012.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Subscribe and Build a Candidate Set (Priority: P1) 🎯 MVP

A consumer opens a subscription to a set of topics and is told **who is already in those topics** (a cold-start replay), then **every subsequent change**. It folds the stream into a per-topic set of member node ids — the candidate set a future sampler/dialer draws from.

**Why this priority**: The irreducible deliverable — a live, per-topic candidate set from the subscription list. Exercisable against the registry alone, no node event loop. The safe MVP slice.

**Independent Test**: Construct an `InMemorySubscriptionRegistry` (from a file or programmatically) with nodes across `{T1}`, `{T1, T2}`, `{T2}`. Open `watch_members({T1})`; assert the cold-start burst reports exactly the two `T1` members (each `Joined` carrying `T1`), and neither the `T2`-only node nor `T2`. Then register a fourth node into `{T1}` → one live `Joined`; unregister an original `T1` node → one `Left`.

**Acceptance Scenarios**:

1. **Given** a registry with `A`(`{T1}`), `B`(`{T1,T2}`), `C`(`{T2}`), **When** a consumer opens `watch_members({T1})`, **Then** the watch yields `Joined` for exactly `A` and `B` (each reporting `T1`) as the cold-start burst, and nothing for `C` or `T2`.
2. **Given** an open `watch_members({T1})` whose burst is drained, **When** `D` is registered with `{T1}`, **Then** the watch yields exactly one `Joined { node: D, topics: {T1} }`.
3. **Given** the same open watch, **When** `A` is unregistered, **Then** the watch yields exactly one `Left { node: A }`.
4. **Given** the same open watch on `{T1}`, **When** `C` (`{T2}`) changes to `{T2, T3}`, **Then** the watch yields **no** event (change confined to unwatched topics).
5. **Given** `watch_members({T4})` for a topic with no members, **When** the burst completes, **Then** no `Joined` is yielded, and a later registration into `{T4}` yields exactly one live `Joined`.

---

### User Story 2 — Membership Is Defined by the Subscription List (Priority: P2)

Membership comes from the subscription list — in production an on-chain artifact written by operator transactions; in this mock, a **subscription-list file** (`node_id → topics`) loaded at construction, plus programmatic `set_interest`/`unregister` the test harness uses to simulate runtime churn (operator stand-in). The caller declares a node's desired interest set; the registry works out whether that is a first registration (`Joined`) or a change (`TopicsChanged`) and emits the delta. The node never writes.

**Why this priority**: This is how the candidate-set stream becomes non-empty. It is **not** a node-daemon behaviour — the node is read-only — but it is how the file-loader and tests create and mutate membership.

**Independent Test**: Load `InMemorySubscriptionRegistry::from_file` with `A→{T1}`, `B→{T1,T2}`; open `watch_members({T1,T2})` and assert the cold-start burst matches the file. Then `set_interest(A, {T1, T2})` → one `TopicsChanged { A, added: {T2} }`; `set_interest(A, {T2})` → one `TopicsChanged { A, removed: {T1} }`; `set_interest(A, {T2})` again → **no** event; `unregister(A)` → one `Left { A }`.

**Acceptance Scenarios**:

1. **Given** a subscription-list file with entries `A→{T1}`, `B→{T1,T2}`, **When** `InMemorySubscriptionRegistry::from_file` loads it and a consumer subscribes to `{T1,T2}`, **Then** the cold-start burst reports `A` (on `T1`) and `B` (on `T1` and `T2`).
2. **Given** `A` registered with `{T1}`, **When** `set_interest(A, {T1, T2})`, **Then** observers see exactly one `TopicsChanged { node: A, added: {T2}, removed: {} }`.
3. **Given** `A` registered with `{T1, T2}`, **When** `set_interest(A, {T2})`, **Then** observers see exactly one `TopicsChanged { node: A, added: {}, removed: {T1} }`.
4. **Given** `A` registered with set `S`, **When** `set_interest(A, S)` with the identical set, **Then** **no** event is emitted.
5. **Given** `A` registered with `{T2}`, **When** `unregister(A)`, **Then** observers see exactly one `Left { node: A }`; a later `set_interest(A, {T2})` is a fresh `Joined`.
6. **Given** `A` registered with `{T1}`, **When** `set_interest(A, {})`, **Then** observers watching `T1` see `TopicsChanged { removed: {T1} }`; `A` stays registered with an empty set, **not** `Left`.

---

### User Story 3 — The Node Reads Its Own Interests and Folds Candidates (Priority: P3)

On startup the node looks up **its own entry** in the registry to learn its authoritative topic-interest set, seeds its 002 subscription/filter from that, and `watch_members` on those topics. A node-owned reader drains the `MembershipWatch` and pushes `Event::MembershipUpdate(MembershipEvent)`; `apply` folds `Joined`/`TopicsChanged`/`Left` into per-topic candidate sets in the crate-internal `NodeState`, excluding the node's own id. The node issues **no** registry writes.

**Why this priority**: This is where subscription-list truth becomes the node's interests *and* its candidate set — and where the source-of-truth invariant is enforced (the node acts on its registered topics, not on config). Builds on feature 004's merged pure core.

**Independent Test** (pure, synchronous, contract §5): Construct `NodeState` for self-id `S`. Feed a scripted `Vec<Event>` of `MembershipUpdate` variants — `Joined { A, {T1} }`, `Joined { B, {T1, T2} }`, `Joined { S, {T1} }` (self), `TopicsChanged { A, added: {T2} }`, `Left { B }` — calling `apply` after each. Assert candidate sets: `T1 → {A}`, `T2 → {A}` (self excluded; `B` removed). Assert every `apply` returns an empty `Vec<Effect>`. Separately, assert `entry(S)` against a registry seeded `S→{T1}` returns `{T1}`, and that a node configured as `S` but with a registry entry `S→{T1}` acts only on `T1`.

**Acceptance Scenarios**:

1. **Given** a registry entry `S→{T1, T2}`, **When** node `S` starts, **Then** its interest set (and 002 message filter) is `{T1, T2}` — sourced from the registry, regardless of any other value — and it opens `watch_members({T1, T2})`.
2. **Given** a `NodeState` for self-id `S`, **When** `apply` processes `MembershipUpdate(Joined { node: S, topics: {T1} })` (own id), **Then** `S` is **not** added to any candidate set (self-exclusion, local).
3. **Given** a state where `A ∈ T1`, **When** `apply` processes `MembershipUpdate(TopicsChanged { node: A, added: {T2}, removed: {T1} })`, **Then** `A` is in `T2` and absent from `T1`; the call returns no effects.
4. **Given** a state where `A ∈ T1, T2`, **When** `apply` processes `MembershipUpdate(Left { node: A })`, **Then** `A` is absent from every candidate set.
5. **Given** a running `Node` with its registry reader producer wired via `spawn_producer`, **When** a node is registered into a watched topic, **Then** it appears in the `Node`'s candidate-set getter; **When** the `Node` is dropped, **Then** the reader producer is aborted with the others.
6. **Given** a `Node` with a config `[[peers]]` bootstrap list, **When** candidate sets are built, **Then** they remain **distinct** from `Node::peers()` — the candidate set never overwrites or merges into the bootstrap list.

---

### User Story 4 — A Network of In-Memory Nodes Discovers Itself From the File (Priority: P4)

Several nodes are brought up against the **same subscription-list file** (the mocked chain). Single-process: they share one `InMemorySubscriptionRegistry` `Arc`. Multi-process: each loads the same file into its own instance — identical membership, no IPC. Each node is configured only with its own `node_id` (+ bootstrap); it looks up its own entry for interests and observes the others to build candidate sets. The network discovers itself with no operator and no chain.

**Why this priority**: The end-to-end demonstration that the mock is usable for multi-node experiments — and that identity + membership come from the shared file, not per-node topic config. Lowest priority; composes US2 + US3.

**Independent Test**: A subscription-list file lists `node-a→{T1}`, `node-b→{T1,T2}`, `node-c→{T2}`. Single-process: load once, share the `Arc` across three `Node`s configured with `node_id` `node-a`/`node-b`/`node-c`. Start all three; assert (via each node's candidate-set getter, polled to steady state) that `node-a`'s `T1` candidates are `{node-b}`, `node-b`'s are `T1:{node-a}` + `T2:{node-c}`, `node-c`'s `T2` are `{node-b}` — each self-excluded, each scoped to its watched topics.

**Acceptance Scenarios**:

1. **Given** three `Node`s sharing one `InMemorySubscriptionRegistry` loaded from a file listing their entries, configured by `node_id` only, **When** all start, **Then** each node's candidate-set getter converges to the interest-scoped, self-excluded view of the others — with each node's interests taken from its **file entry**, not from any topic config.
2. **Given** the started network, **When** a fourth entry `node-d→{T1}` is added (file reload or `set_interest`) and `node-d` started, **Then** `node-a`/`node-b` (watching `T1`) observe `node-d`; `node-c` (watching only `T2`) does not.
3. **Given** a node `node_id` that has **no entry** in the subscription-list file at startup, **Then** the node does not fabricate interests from config — see the startup edge case for the chosen behaviour.
4. **Given** any started node, **When** its activity is inspected, **Then** it has issued **zero** registry writes — all membership originates from the file and/or the test harness.

---

### Edge Cases

- **Unchanged re-announce**: `set_interest(node, S)` with the current set emits no event (US2 AS-4).
- **Empty interest vs withdrawal**: `set_interest(node, {})` keeps the node registered with an empty set; `unregister(node)` removes it (`Left`). Distinct (US2 AS-6/AS-5).
- **Partial topic intersection on change**: a `TopicsChanged` touching watched + unwatched topics is delivered with `added`/`removed` intersected with the watched set (US1 AS-4).
- **Late subscriber**: cold-start burst replays current members before live deltas — never blind (US1 AS-1).
- **Self in the stream**: the stream may include the subscriber's own id (the registry is not told who is asking); the node filters its own id locally (US3 AS-2).
- **No cold-start boundary marker**: the cold-start burst has no explicit end-of-snapshot signal — a consumer folds the burst and subsequent live deltas uniformly into its candidate set. A `SnapshotComplete`-style variant can be added later (the enum is `#[non_exhaustive]`) if/when feature 010's sampler needs an explicit "warm" signal before sampling.
- **Node id absent from the subscription list at startup**: a node whose `node_id` has no entry cannot fabricate interests from config (source-of-truth invariant). When `entry(self_id)` returns `None` at startup, the node MUST **fail fast** — treat the missing entry as a hard startup/configuration error rather than running with empty interests. The protocol's faithful wait-and-retry-with-backoff (`joining.md` step 3, where chain lag is real) is deferred to feature 012.
- **Channel overflow / slow consumer**: v1 watch is unbounded (ADR 0007) — no backpressure, no lag/skip event; deferred.
- **Watch dropped**: ends that subscription cleanly, no effect on registry state or other watches (mirrors `NetworkHandle`).
- **Shared file, many nodes**: single-process shares one `Arc`; multi-process loads the same file independently (identical view); runtime churn across processes = file re-read (poll), deferred.
- **Ordering**: within one watch, events follow the order writes were applied (gap-free in-memory). Cross-watch ordering unspecified.

## Requirements *(mandatory)*

### Functional Requirements

#### Domain boundary and types

- **FR-001**: A public trait `SubscriptionRegistry` MUST be defined as the published interface between the pubsub node and the subscription-list source. `Send + Sync + 'static`, `async` methods, mirroring the `Network` trait (`#[allow(async_fn_in_trait)]` + its tracked `Send`-bound follow-up). It MUST be the sole anti-corruption boundary — no impl's storage or wire encoding may leak through it. It is **distinct** from the future `TopicRegistry`; the two MUST NOT share a trait.
- **FR-002**: A public `MembershipEvent` enum MUST express one membership delta: `Joined { node: PeerId, topics: BTreeSet<TopicId> }`, `TopicsChanged { node: PeerId, added: BTreeSet<TopicId>, removed: BTreeSet<TopicId> }`, `Left { node: PeerId }`. Each carries **identity + interest only** — no network address (endpoints are off-chain, resolved later by gossip/IP-discovery) and no deposit/stake (anti-Sybil deferred). MUST be `#[non_exhaustive]`. `PeerId`/`TopicId` reused from the crate, not redefined.
- **FR-003**: Types that decode/serialize on-chain subscription-list transactions (register/change/leave datum/redeemer) MUST be **module-internal** (`pub(crate)`/private), not in the trait surface or `MembershipEvent` (parse-at-the-edge). The in-memory impl has no wire format yet; the boundary is fixed for the on-chain reader (012).

#### Membership source — the subscription-list file (mocked chain)

- **FR-004**: `InMemorySubscriptionRegistry` MUST provide a `from_file(path)` constructor that loads a **subscription-list file** — a file read **only** by the registry module — mapping each `node_id` to its topic-interest set (the mocked on-chain subscription list). The format MUST follow the crate's existing TOML config convention (e.g. `[[entry]]` tables with `node_id` and `topics`), with strict unknown-field rejection per the 001 config policy. Deposit/stake fields, if present, MUST be ignored (out of scope). The loaded entries are the registry's initial membership and the authoritative source of each node's interests.

#### Write side (operator / test harness — NOT a node runtime behaviour)

- **FR-005**: `SubscriptionRegistry` MUST expose `async fn set_interest(&self, node: PeerId, topics: BTreeSet<TopicId>) -> Result<(), SubscriptionRegistryError>` — a declarative idempotent upsert (first call → `Joined`; changed set → one `TopicsChanged { added, removed }`; unchanged → no-op). In production this models an operator on-chain transaction; in this feature it is called only by the file loader's equivalent and by test harnesses simulating churn — **never by the node daemon**.
- **FR-006**: `SubscriptionRegistry` MUST expose `async fn unregister(&self, node: PeerId) -> Result<(), SubscriptionRegistryError>` — removes the node entirely; observers of its topics see `Left`. Distinct from `set_interest(node, {})`.

#### Read side (push / watch_members + self-lookup)

- **FR-007**: `SubscriptionRegistry` MUST expose `async fn watch_members(&self, topics: BTreeSet<TopicId>) -> Result<MembershipWatch, SubscriptionRegistryError>`. `MembershipWatch` MUST mirror `NetworkHandle`: single-consumer, owns the receive half, not `Clone`, ends on drop. On open it MUST replay current members of the watched topics as a `Joined` burst (cold start), then stream live deltas. Event-driven (push), not poll/diff.
- **FR-008**: `SubscriptionRegistry` MUST expose a self-lookup read — `async fn entry(&self, node: PeerId) -> Result<Option<SubscriptionEntry>, SubscriptionRegistryError>` — returning the node's **subscription-list entry** (`None` if not registered). A `SubscriptionEntry` MUST be a `#[non_exhaustive]` struct carrying at least `node: PeerId` and `topics: BTreeSet<TopicId>`; `#[non_exhaustive]` reserves room for the protocol's further per-entry fields (deposit, identity keys) that feature 012 will carry, without a breaking change — only `node` + `topics` are populated now. The node uses `entry(self_id)?.topics` at startup to learn its **own** authoritative interests before it knows which topics to `watch_members` on. This is the enforcement point of the source-of-truth invariant.
- **FR-009**: Events on a single `MembershipWatch` MUST be **scoped to the watched topics** (`topics`/`added`/`removed` intersected with the watched set; no events for unwatched topics) and delivered in write-application order, gap-free in-memory. The cold-start burst and the subsequent live deltas MUST form a single gap-free, duplicate-free sequence: the snapshot capture and the subscriber registration MUST be atomic, so no write is missed or double-delivered at the burst/live boundary. Cross-watch ordering unspecified.
- **FR-010**: The v1 watch channel MUST be **unbounded** (ADR 0007) — no backpressure, no lag/skip event; deferred. `#[non_exhaustive]` `MembershipEvent` leaves room for the signal.

#### Implementation and errors

- **FR-011**: A concrete `InMemorySubscriptionRegistry` MUST be the first impl: in-process, holding registration state + live subscriber channels behind private interior synchronisation; public struct, private internals; `InMemoryNetwork`-style naming. It MUST be shareable across multiple `Node`s via `Arc` (single-process multi-node topology, US4), as `InMemoryNetwork` is. The private `type Registry` alias in `src/network.rs` (the peer-sender map) MUST be renamed (e.g. `PeerSenders`).
- **FR-012**: A `SubscriptionRegistryError` typed enum MUST back the fallible methods (`std::error::Error` + `Debug` + `Display`; `#[non_exhaustive]`). In-memory methods do not fail normally; the variant set is minimal now and grows with the on-chain backend (012) — the `Result` shape + `#[non_exhaustive]` are the forward-compatible interface justified by that consumer, mirroring `NetworkError`. File-load failures (FR-004) surface through the existing config/IO error path, not necessarily this enum.

#### Node integration — read-only, on feature 004's merged pure core

- **FR-013**: The `Event` enum (feature 004, `src/event.rs`) MUST gain `Event::MembershipUpdate(MembershipEvent)`. The variant + payload are owned by this feature (contract §3); the `apply` arm is one dispatch line in `pub(crate) fn apply` plus a named `handle_membership_update(state, update) -> Vec<Effect>` handler (ADR 0011 convention), existing arms untouched. (Renamed from the `RegistryUpdate` placeholder — see Clarifications.)
- **FR-014**: A node-owned producer MUST drain a `MembershipWatch` and push one `Event::MembershipUpdate(ev)` per `MembershipEvent`, registered via `spawn_producer` (node owns the `JoinHandle`, aborts on drop; symmetric with `network_mailbox_loop`). The node MUST consume the registry only through this variant + the FR-008 startup self-lookup; it MUST issue no registry writes.
- **FR-015**: `apply` (via `handle_membership_update`) MUST fold deltas into a per-topic candidate set of member `PeerId`s in the crate-internal `NodeState`: `Joined` adds, `TopicsChanged` adds/removes, `Left` removes from every set. Pure (no I/O, no `.await`), returns an empty `Vec<Effect>` (`Effect` uninhabited). A public `Node` getter MUST expose a snapshot of the per-topic candidate set (mirroring `received_messages()`); `NodeState` stays `pub(crate)`.
- **FR-016**: The node MUST exclude its own `PeerId` when folding — its id never appears in any candidate set — filtering **locally**; `watch_members` MUST NOT take the subscriber's id.
- **FR-017**: The registry-derived candidate set MUST be **distinct** from the config `[[peers]]` bootstrap list (the `Node` shell field). This feature MUST NOT move, merge, or replace `peers`; `Node::peers()` MUST keep returning the configured bootstrap peers. (Resolves IMPLEMENTATION_NOTES N-007 for the 008 side: the candidate set is the peer data entering `NodeState`; bootstrap `peers` stays on the shell.)

#### Interest sourcing, read-only, and config

- **FR-018**: A node's authoritative topic-interest set MUST be sourced from **its own subscription-list entry**, obtained via `entry(self_id)` (FR-008) at startup — NOT from a node-config topic list. This sourced set MUST seed the retained 002 subscription/message-filter mechanism (which continues to gate message acceptance) and MUST determine which topics the node opens a `watch_members` watch on for candidate building. The node MUST be **strictly read-only** toward the registry (no `set_interest`, no `unregister`, no self-seed). The 002 `subscribed_topics` config field MUST be **removed**; node config carries **`node_id` (identity) + bootstrap peers** only. The node MUST NOT recover or reconcile pre-restart state; restart-time recovery under a persistent/on-chain registry MUST be recorded as a new deferred entry in `specs/IMPLEMENTATION_NOTES.md` (revisit at 012). The node's interest set is **fixed at startup**: it is read once via `entry(self_id)` and determines the watched-topic set (and the 002 message filter) for the node's lifetime; runtime changes to the node's *own* entry take effect only on restart (dynamic re-derivation per `changing-topic-subscription.md` is deferred to 012). When `entry(self_id)` returns `None` at startup, the node MUST **fail fast** with a hard startup/configuration error — it MUST NOT run with an empty interest set.

#### Scope boundaries

- **FR-019**: MUST NOT add topic-governance (create/delete topic, authorized publishers, owner-attested relays). `SubscriptionRegistry` stays subscription-membership only; topic governance is the separate `TopicRegistry` (~007).
- **FR-020**: MUST NOT implement address resolution (node id → endpoint; gossip/IP-discovery), connecting/dialing to candidates (dialer, ~006 / `004-connections`), deposit/anti-Sybil enforcement, peer sampling/selection (010 consumes the candidate set), or persistence.

#### Testing discipline

- **FR-021**: The registry module MUST be verifiable **in isolation** (construct/`from_file`, write, `watch_members`, `entry`; assert on emitted `MembershipEvent`s + cold-start burst). Node integration MUST be testable as a **pure** state-machine exercise (scripted `Vec<Event>` of `MembershipUpdate` through `apply`; assert candidate sets + `Vec<Effect>`). The source-of-truth invariant MUST be tested: a node configured as `S` against a registry entry `S→T` acts on `T` regardless of any other configured value. The multi-node topology (US4) is testable over a shared `Arc` (or a shared file) with getter polling to steady state. Tests MUST assert on events/state/snapshots — never on log content (Constitution).

### Key Entities

- **SubscriptionRegistry** (trait): published interface / anti-corruption boundary over the subscription-list source. Methods: `set_interest`, `unregister`, `watch_members`, `entry`. Distinct from the future `TopicRegistry`.
- **Subscription-list file**: the mocked on-chain subscription list — a TOML file (`node_id → topics`) read only by the registry module; loaded via `InMemorySubscriptionRegistry::from_file`. The membership source of truth.
- **SubscriptionEntry** (`#[non_exhaustive]`): a node's entry in the subscription list — `node` + `topics` (+ future deposit/keys). The materialized record returned by `entry`, distinct from the `MembershipEvent` delta.
- **MembershipEvent** (`#[non_exhaustive]`): one membership delta — `Joined`/`TopicsChanged`/`Left`, identity + interest only. Payload of `Event::MembershipUpdate`.
- **MembershipWatch**: single-consumer subscription handle (mirrors `NetworkHandle`) over an unbounded channel; not `Clone`; ends on drop. Cold-start `Joined` burst, then live deltas.
- **InMemorySubscriptionRegistry**: v1 concrete impl; holds node→interest state + live subscriber channels; `from_file` + programmatic writes; shareable via `Arc`.
- **SubscriptionRegistryError** (`#[non_exhaustive]`): typed error for fallible methods; minimal now.
- **Per-topic candidate set** (registry-derived, in `NodeState`): per-topic `PeerId` sets folded from events, self-excluded; snapshot via a public `Node` getter. **Distinct** from config `[[peers]]`.
- **Event::MembershipUpdate**: the single seam the node consumes the registry through.
- **Node identity / config**: `node_id` (the node's own `PeerId`) + bootstrap peers. No topic list (sourced from the registry).
- **PeerId / TopicId** (reused): identity + topic primitives the registry keys on.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A consumer subscribing to a topic with `N` pre-existing members receives all `N` in the cold-start burst before any later change (cold-start completeness).
- **SC-002**: A membership change on a watched topic is observed exactly once by every current subscriber to that topic.
- **SC-003**: A node never appears in its own candidate set, for any event sequence including its own id.
- **SC-004**: Re-announcing an unchanged interest set produces zero membership events.
- **SC-005**: A subscriber watching topic `A` receives no events for changes confined to a topic `B` it does not watch.
- **SC-006**: The registry's write/`from_file`/watch_members/`entry` behaviour is fully verified without the node event loop.
- **SC-007**: A node's effective interest set equals its subscription-list entry — a node configured as `S` against entry `S→T` acts only on `T`, independent of any node-config topic value (source-of-truth invariant).
- **SC-008**: In a multi-node network sharing one subscription-list source, each node's candidate set converges to the interest-scoped, self-excluded view of the others (self-discovery), with interests taken from the file.
- **SC-009**: A running node performs zero registry writes; the candidate set never alters the config bootstrap `peers` field.

## Assumptions

- **Feature 004 has merged** (PR #50): the pure core (`apply`/`NodeState`/uninhabited `Effect`), `Arc<Mutex<NodeState>>` shell with sync getters, sync `subscribe`/`unsubscribe`, and the `Event`/`EventQueue`/`spawn_producer` seam exist on `main` (ADR 0011/0012). Registry-module stories (US1/US2) are independent of it; node-integration stories (US3/US4) build on it.
- **Source of truth is the subscription list**, not config — both in production (on-chain) and in the mock (the subscription-list file). The node looks up its own entry; config supplies identity + bootstrap only. The `joining.md` config-vs-chain authority ambiguity is surfaced as an issue/ADR per Principle V.
- **The seam-variant rename** (`RegistryUpdate` → `MembershipUpdate`) needs a heads-up to the 004 author + a one-line update to ADR 0011's illustrative comment and the CLAUDE.md SpecKit block.
- **In-memory, single source via file or shared `Arc`.** No network, chain, or persistence; unbounded channel per ADR 0007.
- **Identity is `PeerId`** (opaque string today; pubkey-derived at 011). Endpoints/key material out of scope.
- **Reused**: the 002 message-filter mechanism (now seeded from the registry, not the removed `subscribed_topics`), the 003 snapshot-read pattern, the `Network` actor-handle idiom, the config `[[peers]]` bootstrap field, and the 001 strict-TOML config convention for the subscription-list file.
- **Deferred with owners**: topic governance → `TopicRegistry` (~007); address resolution → gossip/IP-discovery; connecting/dialing → dialer (~006 / `004-connections`); deposit/anti-Sybil → 012; peer sampling → 010; bounded-channel backpressure → real transport; restart-state recovery + multi-process file re-read → 012.

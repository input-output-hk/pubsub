# Research: Topic Registry (013)

**Date**: 2026-06-11 | **Plan**: [plan.md](./plan.md)

No `NEEDS CLARIFICATION` remained after `/speckit-specify` + `/speckit-clarify`; this consolidates the design decisions (each: Decision / Rationale / Alternatives considered). Structural rationale is in [ADR 0016](../../docs/decisions/0016-topic-registry-interface-and-node-integration.md); the shape deliberately parallels 008's [ADR 0014](../../docs/decisions/0014-subscription-registry-interface-and-node-integration.md).

## D1 — Distinct registry, no shared trait with 008

- **Decision**: A standalone `TopicRegistry` read trait + `TopicRegistryControl` write trait + `InMemoryTopicRegistry`, sharing **no** trait with `SubscriptionRegistry` (008). Only the event-queue seam idiom (a node-owned reader pushing one `Event` variant) is shared.
- **Rationale**: `docs/node-lifecycle/README.md` defines two distinct on-chain artifacts with different keys (topic id vs node pubkey), payloads (authorised publisher keys vs topic set + deposit), and readers (relayers verifying signatures vs subscribers computing candidate sets). 008's spec (FR-019) explicitly deferred topic governance to "the separate `TopicRegistry`". A shared trait would force an artificial common abstraction over two unrelated lookups.
- **Alternatives**: a unified `Registry<K, V>` generic trait (rejected — couples unrelated artifacts, no shared reader, premature abstraction); folding topics into the subscription registry (rejected — conflates membership with governance, contradicts the protocol's artifact split).

## D2 — Global watch, not node-keyed

- **Decision**: `fn watch(&self) -> impl Future<Output = Result<TopicRegistryWatch, _>> + Send` — **no** scoping argument. The cold-start burst replays every registered topic; live deltas cover all topics.
- **Rationale**: a node must validate *any* topic named in its subscription-list entry and must authorize publishers on *any* topic it is effectively subscribed to. Topic legitimacy is a global fact, not naturally scoped to one node. 008's `watch(node)` is node-keyed because subscription-list membership *is* naturally per-node-topic-scoped; topics are not. The registered-topic count is small, so folding all of them is cheap.
- **Alternatives**: a topic-scoped `watch(topics)` keyed on the node's subscription-list topics (rejected — would couple the topic-registry watch to the membership stream's output, creating a cross-stream ordering dependency at watch-open time; a premature optimization with no ROADMAP consumer needing it); a per-topic `watch(topic)` (rejected — N subscriptions per node, more churn, same coupling).

## D3 — Event shape: `Registered` / `PublishersChanged` / `Removed`, `BTreeSet<PublicKey>`

- **Decision**: `#[non_exhaustive] enum TopicRegistryEvent { Registered { topic, publishers }, PublishersChanged { topic, added, removed }, Removed { topic } }`, publishers as `BTreeSet<PublicKey>` (empty = open). Mirrors `MembershipEvent`'s three-variant join/change/leave shape.
- **Rationale**: structural symmetry with 008 keeps the fold + tests familiar; `added`/`removed` diffs (registry-computed) match `TopicsChanged`. `BTreeSet` (over `HashSet`) gives deterministic iteration/Debug and matches `MembershipEvent`'s `BTreeSet<TopicId>`. `#[non_exhaustive]` leaves room for a future warmth/lag signal (the named 012/010 consumers).
- **Alternatives**: a single `Snapshot(HashMap<…>)` event (rejected — loses delta semantics, forces full-state diffing in the fold); carrying the full new publisher set on every change instead of add/remove diffs (rejected — the diff is what scoped fan-out and minimal node updates want, matching 008); `HashSet<PublicKey>` (rejected — see D9).

## D4 — Effective subscriptions = two folded sets ANDed at accept time

- **Decision**: `NodeState` keeps 008's `subscriptions: HashSet<TopicId>` (written only by `handle_membership_update`) **unchanged** and adds `registered_topics: HashMap<TopicId, BTreeSet<PublicKey>>` (written only by `handle_topic_registry_update`). The effective subscription set — the message accept-filter — is the intersection, computed at accept time (`subscriptions.contains(t) && registered_topics.contains_key(t)`). The `subscriptions()` getter returns this intersection (the effective accept-filter) for observability/tests; post-implementation it became the single subscriptions getter (no separate declared getter).
- **Rationale**: each registry's handler owns exactly one field — no cross-handler mutation, no shared-write invariant. The AND-at-read handles *any* arrival ordering of the two streams for free: a topic registered after the node subscribes becomes effective the instant its `Registered` event folds, and a removed topic stops being effective immediately — with zero extra wiring (SC-004). It is also O(1) per inbound message.
- **Alternatives**: a single stored `effective_subscriptions` set recomputed on every event from either stream (rejected — adds a third field whose invariant both handlers must maintain, re-derivation logic, and a window where it is stale relative to its inputs); making 008's `subscriptions` field itself the intersection (rejected — would force `handle_topic_registry_update` to write a field 008 owns, and `handle_membership_update` to read topic-registry state, coupling the two features' handlers and violating the clean separation ADR 0014 established).

## D5 — Authorized-publisher enforcement, ordered before signature verification

- **Decision**: `handle_signed_message` check order becomes: (1) subscribed? → drop `topic_not_subscribed`; (2) registered? → drop `topic_not_registered`; (3) publisher authorized (topic open ⇒ any; else publisher key ∈ authorized set)? → drop `publisher_not_authorized`; (4) signature verifies? → drop `invalid_signature`; (5) record. The two new checks are additive; existing causes/behavior unchanged for the valid path.
- **Rationale**: the three new/existing membership+authorization checks are cheap set lookups; signature verification is the expensive step. Ordering the cheap filters first (and authorization before verification, per FR-015) means unauthorized/off-topic/unregistered traffic never pays verification cost — extending the existing "topic filter first, then verify" comment in `handle_signed_message`. Distinct drop causes give operators an actionable signal per failure mode.
- **Alternatives**: verify signature first, then authorize (rejected — pays verification on spam from unauthorized publishers; FR-015 mandates authorization first); a single merged "not acceptable" drop cause (rejected — loses operator diagnosability; the protocol distinguishes off-topic, illegitimate-topic, and unauthorized-publisher).

## D6 — Strictly read-only node; write surface on a separate `Control` trait

- **Decision**: `set_topic` / `remove_topic` live on `TopicRegistryControl: TopicRegistry`; `Node` depends only on `TopicRegistry`, taken generically as `Arc<T>`. The file loader and test/operator-sim code hold the concrete `Arc<InMemoryTopicRegistry>` to drive writes.
- **Rationale**: identical reasoning to 008's F3/ADR 0014 split — the node-facing domain interface stays free of write/test signatures, matches the read-only node and the read-only 012 chain reader (on-chain writes are governance transactions, not a reader method).
- **Alternatives**: write methods on the single trait (rejected — puts operator/test surface in the node's dependency); a `dyn TopicRegistry` object (rejected — an `async fn`/RPITIT trait is not `dyn`-compatible; consumed generically as `Network`/`SubscriptionRegistry` are, ADR 0007).

## D7 — `set_topic` declarative upsert; `remove_topic` hard delete

- **Decision**: `set_topic(topic, publishers)` is a declarative idempotent upsert (first → `Registered`; changed publisher set → one `PublishersChanged { added, removed }`; unchanged → no-op). `remove_topic(topic)` deletes the entry → `Removed`. `set_topic(topic, {})` registers/retains the topic *open*, distinct from removal.
- **Rationale**: mirrors 008's `set_topics`/`unregister` exactly, so the in-memory diff/fan-out logic and tests carry over. A declarative upsert is sufficient for the loader + tests; the node never writes.
- **Alternatives**: per-key `add_publisher`/`remove_publisher` operations matching the formal model's `AddPublisherRequest`/`RemovePublisherRequest` (rejected for the mock — the node doesn't write, and the loader/tests are simpler with a declarative set; the granular ops are 012's on-chain governance surface); soft-delete (`alive` flag) per the formal model (rejected for the mock — see D8).

## D8 — Mock scope: no governance, no soft-delete

- **Decision**: the mock models only the node-consumed projection — registered topics + authorized publisher keys. Owners, admins, role grants, `replicationFactor`, `retentionPeriod`, epochs, and the `alive` soft-delete are **out of scope** (FR-017); `remove_topic` is a hard delete.
- **Rationale**: the node reads none of the governance fields; carrying them would be unjustified surface (forward-compatible-interface standard wants a real consumer). The full ten-operation contract with its authorization matrix is feature 012 (the on-chain feed), already formally specified in Quint. The `alive` flag exists on-chain to prevent topic-id reassignment — a persistence concern with no analog in the in-memory mock. Surfaced (not silently dropped) in ADR 0016 + FR-017.
- **Alternatives**: model the full `Topic` record (rejected — speculative generality, Principle I); soft-delete in the mock (rejected — no reassignment risk in-memory; adds an `alive`-filtering branch to every read for no node-visible benefit).

## D9 — `BTreeSet<PublicKey>` requires `Ord` on `PublicKey` (additive derive)

- **Decision**: add `Ord, PartialOrd` to `PublicKey`'s derive list in `src/crypto/mod.rs` (it wraps `Vec<u8>`, which is `Ord` — lexicographic byte order). Use `BTreeSet<PublicKey>` for publisher sets in events and `NodeState`.
- **Rationale**: consistency with `MembershipEvent`'s `BTreeSet<TopicId>` (deterministic Debug + iteration), and a stable order for fan-out and test assertions. The change is purely additive and local (tactical, not structural — reversing it is a local rewrite confined to this feature's `BTreeSet` usage), so no separate ADR; noted in ADR 0016's consequences.
- **Alternatives**: `HashSet<PublicKey>` (works — `PublicKey: Hash + Eq` — and set-equality assertions are order-independent, so it avoids touching `crypto`; rejected for inconsistency with 008's `BTreeSet` and nondeterministic Debug/iteration, but recorded as the fallback if the `crypto`-type owner objects to the derive); a newtype wrapper with a local `Ord` impl (rejected — needless indirection over a one-line additive derive).

## D10 — Topic-registry file format + hex publisher keys at the edge

- **Decision**: a separate TOML file, `[[topic]]` tables with `id` (string) and an optional `publishers` array of lowercase-hex public-key strings (absent/empty ⇒ open) — **only** registered topics + authorized publishers, the facts the node consumes. Strict `deny_unknown_fields` applies **uniformly**: governance fields (`owners`/`admins`/`replication_factor`/`retention_period`) are **not** part of the mock format (012's on-chain domain), so any field outside `id`/`publishers` is a load error — no accepted-but-ignored fields (resolves analyze **F1** by **simplification** — the mock file is our own minimal config, not a faithful on-chain dump; the "ignore governance fields" clause is dropped, not reconciled). Duplicate `id` and malformed hex are load errors (`ConfigError::DuplicateTopicEntry` / `InvalidPublisherKey`). Hex is decoded by a small module-internal helper (the crate hand-rolls hex *encoding* in `crypto::Display`; decoding is the symmetric few lines).
- **Rationale**: mirrors 008's subscription-list `from_file` (parse-at-the-edge, strict unknown-field rejection, duplicate-key error). Hex matches `PublicKey`'s `Display`, so a key emitted by the mock round-trips. No `hex` crate is introduced (no-new-deps; the encoding precedent is already hand-rolled).
- **Alternatives**: base64 or raw byte arrays in TOML (rejected — hex matches the existing `Display`/operator-readable convention); reusing the subscription-list file (rejected — distinct artifacts, FR-004; a combined file would conflate the two registries).

## D11 — Validation lives in the node, not the subscription registry

- **Decision**: topic-validity (a node only effectively subscribes to registered topics) is enforced in the **node's** fold + accept path (D4), **not** by having the subscription registry validate against the topic registry and sanitize invalid topics out of the `MembershipEvent`s it emits.
- **Rationale**: the node already consumes the topic registry **directly** for publisher authorization (D5/FR-015) — that data exists only in the topic registry — so the node↔topic-registry stream is non-optional; topic-validity is then a free intersection on the same `registered_topics` projection. Sanitizing in the subscription registry would (a) not remove the node's topic-registry dependency (authorization still needs it) — it would consume the topic registry in two places; (b) couple two **independent** on-chain artifacts (the subscription-list contract does not reference the topic registry — there is no subscription-list formal model and the topic-registry contract never references one); (c) break the clean per-artifact 012 reader swap (the real subscription-list reader would have a topic registry injected); (d) bypass the event-queue/pure-`apply` fold — the *less* event-native path. The most native design is the established 004 pattern: independent producers → one queue → pure fold → accept path reads combined state. The node never *acts* on a phantom topic regardless (excluded from the effective filter, traffic dropped + logged).
- **Alternatives**: validate + sanitize in `InMemorySubscriptionRegistry` (rejected — above); a topic-scoped node watch so the node "doesn't know all topics" (D2 — rejected for the mock: re-couples the two streams for negligible benefit at the mock's topic count).

## D12 — Identity model: publishers are `PublicKey`, unifying with node id at 011

- **Decision**: key authorized publishers by `PublicKey` (events + `registered_topics`), matching the formal model's `publishers: Set[PublicKey]` and the message `publisher_id` (`PublisherId` wraps `PublicKey`, consulted on verification). The node-facing projection carries publishers only; owners/admins/R/T are deferred (D8).
- **Rationale**: in the protocol a publisher key *is* a node identity — the same identity the subscription list keys on (node pubkey). The current mock's split (`PeerId` opaque string vs `PublicKey` bytes) is a pre-011 artifact; 011 unifies identity to pubkey-derived. Keying by `PublicKey` now matches both the model and the verification path and avoids a translation layer. The unification is surfaced (not silently assumed) as IMPLEMENTATION_NOTES **N-009** (Principle IV).
- **Alternatives**: key publishers by `PeerId` to match the subscription list's node id (rejected — `PeerId` ≠ `PublicKey` today, and the message's publisher is a `PublicKey`; would force a translation the protocol doesn't have); wait for 011 to unify first (rejected — 013 needs publisher keying now; N-009 records the convergence).

## Cross-cutting

- **Reuses**: the 004 pure core (`apply`/`NodeState`/uninhabited `Effect`, ADR 0011/0012); the 008 node-owned-reader + `spawn_producer` seam (ADR 0014) and `MembershipWatch` handle shape (ADR 0007); the 003 signature-verification accept path (ADR 0009/0010), now preceded by the authorization check; the 001 strict-TOML config convention.
- **Deferred (with owners)**: governance / RBAC / replication / retention / soft-delete / epochs → topic-registry contract (012); real on-chain feed → 012; chain-integrity / equivocation / sequence / deposit → IMPLEMENTATION_NOTES N-003 (012); a "registries warm" cross-stream readiness signal → reviewed and deferred (converge from streams; tests poll to steady state), revisit if 010's sampler needs it; bounded-channel backpressure → real transport.

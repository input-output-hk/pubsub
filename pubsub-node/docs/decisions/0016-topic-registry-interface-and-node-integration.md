# ADR 0016: Topic registry interface and node integration

**Status**: Accepted
**Date**: 2026-06-11
**Feature**: 013-topic-registry
**Source**: `specs/013-topic-registry/{spec,plan,research,data-model}.md` + `contracts/topic-registry.md`; the sibling subscription-registry decision ADR 0014; ADR 0007 (Network actor-handle), ADR 0011/0012 (004 pure core + lifecycle), ADR 0009/0010 (crypto + message hierarchy); `../formal_spec/topic_registry/` and `../docs/node-lifecycle/{README,topic-creation}.md` (READ-ONLY); `IMPLEMENTATION_NOTES.md` N-003.

## Context

Feature 013 is the in-memory **topic registry** — the source of truth for which topics legitimately exist and which keys may publish to each. It is the topic-governance counterpart to 008's subscription list; 008 (FR-019) explicitly deferred topic governance to "the separate `TopicRegistry`". Per `../docs/node-lifecycle/README.md`, the two are distinct on-chain artifacts (different keys, payloads, readers). Like ADR 0014, this is structural per Principle III: the trait surface is what feature 012 (on-chain reader) builds against; the seam variant and the `Node::new` shape touch already-merged code (004/008) and every existing caller; reversing any of it is not a local rewrite. The data model (empty-publishers ⇒ open; topic legitimacy; authorised keys) is taken from the Quint model `../formal_spec/topic_registry/types.qnt`. The `/speckit-clarify` pass (2026-06-11) confirmed publisher-authorization **enforcement** (drop) is in scope; this ADR fixes the interface and wiring.

## Decision

### 1. Two traits — read (node-facing) and control (operator/test) — parallel to ADR 0014, sharing no trait with 008

The node-facing trait is **read-only**; the write surface is a separate trait extending it. The node depends only on the read trait; the 012 chain reader implements only the read trait (on-chain governance writes are transactions, not a reader call); the domain interface stays free of write/test signatures. `TopicRegistry` and `SubscriptionRegistry` **share no trait** — different key (topic id vs node id), payload (authorised keys vs topic set + deposit), and reader.

```rust
pub trait TopicRegistry: Send + Sync + 'static {        // read-only; Node depends on this; 012 implements it
    // The SINGLE method, and a GLOBAL stream (no scoping argument — cf.
    // SubscriptionRegistry::watch(node)). Send future (RPITIT) because the
    // node-owned reader awaits it inside a spawned task.
    fn watch(&self)
        -> impl std::future::Future<Output = Result<TopicRegistryWatch, TopicRegistryError>> + Send;
}

pub trait TopicRegistryControl: TopicRegistry {          // operator/test write surface; node never depends on it
    async fn set_topic(&self, topic: TopicId, publishers: BTreeSet<PublicKey>) -> Result<(), TopicRegistryError>;
    async fn remove_topic(&self, topic: TopicId) -> Result<(), TopicRegistryError>;
}

#[non_exhaustive]
pub enum TopicRegistryEvent {
    Registered { topic: TopicId, publishers: BTreeSet<PublicKey> },   // empty publishers ⇒ open
    PublishersChanged { topic: TopicId, added: BTreeSet<PublicKey>, removed: BTreeSet<PublicKey> },
    Removed { topic: TopicId },
}
```

`TopicRegistryWatch` is single-consumer (not `Clone`, owns an unbounded `mpsc` receiver, ends on drop) — the `NetworkHandle`/`MembershipWatch` shape (ADR 0007). `set_topic` is a declarative idempotent upsert (first → `Registered`; changed → one `PublishersChanged { added, removed }`; unchanged → no-op); `remove_topic` is a hard delete → `Removed`, distinct from `set_topic(t, {})` (registers/retains `t` *open*). Events carry topic id + authorised keys only — no replication/retention/owners/admins. `#[non_exhaustive]` leaves room for a future warmth/lag signal.

**Node-facing projection = publishers-only; identity = `PublicKey`** (design review 2026-06-11). The Quint `Topic` record carries `{ name, owners, admins, publishers (empty ⇒ open), replicationFactor, retentionPeriod, alive }` with an owner/admin authorization matrix; the **node consumes none of** owners/admins/R/T, so the node-facing projection is registered-topics + authorized-publishers only (forward-compatible-interface standard: no consumer ⇒ don't carry them), governance is deferred to 012 (FR-017), and the mock's write surface is **permissionless** (no owner/admin gating). The **mock file** likewise carries only `id` + optional `publishers` — strict `deny_unknown_fields` applies uniformly, governance fields are not part of the mock format (resolves analyze F1 by simplification: our own minimal config, not a faithful on-chain dump). Authorized publishers are keyed by `PublicKey` — the same identity space the subscription list uses (node pubkey) in the protocol; the mock's `PeerId`(string)/`PublicKey`(bytes) split unifies at 011, recorded as IMPLEMENTATION_NOTES **N-009** (Principle IV). The model's numeric `TopicID` + `name` collapse to the crate's string `TopicId` (the 012 reader maps).

### 2. Global watch, not node-keyed (the deliberate divergence from ADR 0014)

`watch()` takes **no argument**. On open it replays a cold-start burst of `Registered` events — one per currently-registered topic — then streams live deltas; the burst + deltas are one gap-free, duplicate-free sequence (snapshot + subscriber registration atomic under the lock). This differs from 008's node-keyed `watch(node)` on purpose: subscription-list membership is naturally scoped to *a node's* topics, but topic legitimacy is a **global** fact — the node must validate any topic in its subscription-list entry and authorize publishers on any topic it accepts. Topic-scoping the watch would couple it to the membership stream's output at watch-open time; the registered-topic count is small, so folding all of it is cheap. (Research D2.)

### 3. Seam variant + handler; registered-topics projection in `NodeState`

The node consumes the registry through one new `Event` variant: `Event::TopicRegistryUpdate(TopicRegistryEvent)`, with a named `handle_topic_registry_update(&mut NodeState, TopicRegistryEvent) -> Vec<Effect>` dispatched by one line in `apply` (the ADR 0011 named-handler convention; sibling to 008's `MembershipUpdate`/`handle_membership_update`). `NodeState` gains `registered_topics: HashMap<TopicId, BTreeSet<PublicKey>>`, written **only** by this handler (`Registered` inserts/replaces, `PublishersChanged` applies the diff, `Removed` drops). It does not touch `subscriptions` or `candidates` (008's fields) — each registry's handler owns exactly one field.

### 4. Effective subscriptions = two folded sets ANDed at accept time

The message accept-filter is the **intersection** of 008's membership-derived `subscriptions` and 013's `registered_topics` keys, computed at accept time — **not** a stored derived set. `handle_signed_message` gains two checks before the existing signature verification:

```
1. subscribed?   subscriptions.contains(topic)                         else drop: topic_not_subscribed   (existing)
2. registered?   registered_topics.contains_key(topic)                 else drop: topic_not_registered    (NEW)
3. authorized?   registered_topics[topic].is_empty()                   else drop: publisher_not_authorized (NEW)
                 || registered_topics[topic].contains(publisher_key)
4. signature?    verifier.verify(...)                                  else drop: invalid_signature       (existing)
5. record
```

The two new checks are cheap O(1) lookups ordered **before** the expensive signature verification (FR-015), extending the existing "filter first, then verify" ordering. A single `Node::subscriptions()` getter (and the `NodeState` accessor) expose the intersection — the effective accept-filter — for observability/tests; the declared set and `registered_topics` stay internal. (Post-implementation, 2026-06-12: an earlier separate `effective_subscriptions()` getter was collapsed into `subscriptions()` — a single clear concept for consumers, changing 008's declared-set semantics.) Keeping two independently-folded sets (rather than one stored `effective_subscriptions`) decouples the handlers and makes any stream arrival order correct for free: a topic registered after the node subscribes becomes effective the instant its `Registered` event folds; a removed topic stops being effective immediately (SC-003/SC-004). (Research D4.)

**Validation lives in the node, not the subscription registry** (design review 2026-06-11). An alternative considered was to have the subscription registry validate its entries against the topic registry and sanitize unregistered topics out of the `MembershipEvent`s it emits, so the node never sees them. Rejected: the node already consumes the topic registry **directly** for publisher authorization (§4 step 3 — that data lives only here), so the node↔topic-registry stream is non-optional and topic-validity is a free intersection on the same projection. Sanitizing in the subscription registry would not remove the node's topic-registry dependency (it would consume the registry twice), would couple two **independent** on-chain artifacts (the subscription-list contract does not reference the topic registry — there is no subscription-list formal model), would break the clean per-artifact 012 reader swap, and would bypass the event-queue/pure-`apply` fold (the less event-native path). The node never *acts* on a phantom topic regardless — it is excluded from the effective filter and its traffic dropped + logged. (Research D11.)

### 5. `Node::new` gains a third registry generic; node is read-only; publisher keys orderable

`Node::new` **adds the topic registry generically** — `Node::new<N: Network, R: SubscriptionRegistry, T: TopicRegistry>(…, topic_registry: Arc<T>)`, *not* `Arc<dyn>` (an `async fn`/RPITIT trait is not `dyn`-compatible; consumed generically as `Network`/`SubscriptionRegistry` are). It spawns a node-owned reader producer calling `watch()` (symmetric with the 008 membership reader and `network_mailbox_loop`); `registered_topics` converges as the burst drains, no startup point-read, no fail-fast on an empty registry. The node issues **no** registry writes — `set_topic`/`remove_topic` are for the `from_file` loader and test harnesses. Because the topic registry is mandatory and always enforced, every existing delivery test must register the topics it sends on (the atomic call-site change, like 008's `subscribed_topics` removal). `BTreeSet<PublicKey>` requires `Ord` on `PublicKey`; it wraps `Vec<u8>`, so `Ord, PartialOrd` are added as a purely-additive derive (tactical, local — not a separate ADR).

## Consequences

- The registry module is independently testable without the node loop; the fold + accept-path changes are testable as a pure state machine.
- **Public API change**: `Node::new`'s signature gains the third registry generic; `main.rs` and every `tests/` caller that delivers messages are updated in the same feature (register topics, await convergence). `Node::subscriptions` changes semantics to return the effective accept-filter (declared ∩ registered) — a single getter, no separate `effective_subscriptions` (supersedes 008's declared-set semantics); `candidates`/`peers` unchanged. `Event` gains `TopicRegistryUpdate`; `ConfigError` gains `DuplicateTopicEntry` + `InvalidPublisherKey`; `PublicKey` gains `Ord, PartialOrd`.
- **Message acceptance is now gated on the topic registry**: with no topic registered (empty registry or before the cold-start burst drains), a node has no effective subscriptions and drops all traffic. Send-then-observe tests must register topics and poll to steady state (the `await_subscriptions` harness helper), as 008 introduced for membership convergence. No cross-stream "registries warm" barrier in v1 (reviewed and deferred — converge from streams).
- Clean 012 swap: `from_file` → chain reader; `watch` → on-chain reads/subscriptions; the node, `apply`, the fold, and the accept path are untouched.
- The seam stays minimal (one variant + one handler + one producer), exhaustiveness-checked by the compiler.
- **N-003 partially closed**: publisher-authorization validation lands here; equivocation, parent-hash chaining, per-publisher sequence monotonicity, and deposit/anti-Sybil remain deferred to 012. `IMPLEMENTATION_NOTES.md` N-003 is updated to record the split.

## Alternatives considered

- **Share a trait / a generic `Registry<K,V>` with 008**: rejected — distinct on-chain artifacts (different key/payload/reader); a common abstraction would be premature and contradict the protocol's artifact split (Research D1).
- **Node-keyed or topic-scoped topic watch** (mirror 008's `watch(node)`): rejected — topic legitimacy is global; scoping couples the topic-registry watch to the membership stream and is a premature optimization with no ROADMAP consumer (Research D2).
- **A stored `effective_subscriptions` set recomputed on every event**: rejected — adds a third field whose invariant both handlers must maintain plus a staleness window; the AND-at-read is O(1) and order-independent (Research D4).
- **Make 008's `subscriptions` field the intersection**: rejected — would force `handle_topic_registry_update` to write a field 008 owns and `handle_membership_update` to read topic-registry state, coupling the two features' handlers.
- **Verify signature before authorizing the publisher**: rejected — pays verification cost on unauthorized-publisher spam; FR-015 mandates the cheap authorization check first.
- **Per-key `add_publisher`/`remove_publisher` write ops** (mirror the formal model's granular operations): rejected for the mock — the node never writes; a declarative `set_topic` upsert is simpler for the loader/tests; the granular governance ops are 012's on-chain surface (Research D7).
- **Model the full `Topic` record (owners/admins/replication/retention/alive)**: rejected — the node consumes none of them; carrying them is unjustified surface (Principle I). Governance + the `alive` soft-delete are 012 (FR-017; Research D8).
- **Per-watcher topic filter on the in-memory impl** (as 008's subscribers carry): rejected — the topic watch is global, so subscribers need no filter.
- **`HashSet<PublicKey>` to avoid touching `crypto`**: viable (set-equality is order-independent) and recorded as the fallback, but rejected for inconsistency with 008's `BTreeSet` and nondeterministic Debug/iteration (Research D9).

## Sources

- `specs/013-topic-registry/spec.md` — FR-001..019, SC-001..010, Clarifications 2026-06-11.
- `specs/013-topic-registry/{plan,research,data-model}.md` + `contracts/topic-registry.md`.
- ADR 0014 (the 008 split this parallels), ADR 0007 (handle pattern), ADR 0011/0012 (004 pure core), ADR 0009/0010 (crypto + message hierarchy + the accept path this extends).
- `../formal_spec/topic_registry/types.qnt` (`Topic.publishers` empty ⇒ open; topic legitimacy; authorised keys), `../docs/node-lifecycle/{README,topic-creation}.md` (topic registry vs subscription list; registry read so relayers verify signatures).
- `IMPLEMENTATION_NOTES.md` N-003 (chain-integrity / publisher-authorization, partially closed here).

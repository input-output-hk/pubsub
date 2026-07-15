# Data model — 015 unified link model & publishing links

## 1. Vocabulary (`connection_state.rs`)

```rust
/// Which dissemination duty a link serves (FR-001).
pub enum LinkRole {
    /// Full flood participation: carries published AND relayed messages.
    Relay,
    /// Publishing link (the M3 S-link): carries only the dialing publisher's
    /// own locally-originated messages.
    Publisher,
}

/// Who dialed (FR-001). Orientation per role is derived — research R2.
#[non_exhaustive]
pub enum LinkDirection {
    /// This node dialed.
    Out,
    /// The peer dialed.
    In,
}

/// Establishment lifecycle of an Out link (the former `UpstreamState`,
/// renamed; FR-003). In links are recorded `Active` at acceptance.
pub enum LinkState {
    AwaitingAccept,
    Active,
}
```

## 2. The store (`connection_state.rs`, replacing `upstream` + `downstream`)

```rust
pub struct LinkStore {                       // flow-oriented, ADR 0036
    sources: BTreeMap<TopicId, BTreeMap<PeerId, SourceEntry>>, // peers I receive from
    sinks:   BTreeMap<TopicId, BTreeMap<PeerId, SinkEntry>>,   // peers I send to
}   // nested topic → peer: borrowed-key lookups (no clones); per-topic ops walk one subtree
struct SourceEntry { pull: Option<LinkState>, push_accepted: bool }
struct SinkEntry   { relay_accepted: bool,    push: Option<LinkState> }
// role × direction ⇄ facet: Relay/Out = sources.pull · Publisher/In = sources.push_accepted
//                           Relay/In  = sinks.relay_accepted · Publisher/Out = sinks.push
```

- **Each seam reads one map** (ADR 0036): the receive gate looks up `sources`; every fan-out policy is one `sinks` pass selecting facets. One entry per peer ⇒ per-peer dedup is structural.
- The **role × direction vocabulary stays** as the mutation API, snapshots (`links()` emits the same tuples), and wire — tests bind to those views, never to the store shape (the test-stability rule; this refactor changed no test file).
- Coexisting roles per pair (Clarifications 2026-07-13) = two facets on one entry. Ordered maps: deterministic emission. Terminal outcomes are removals — unchanged.

**Migration mapping** (FR-002/003/004): `upstream[(p,t)] = s` → `links[(p,t,Relay,Out)] = s`; `downstream ∋ (p,t)` → `links[(p,t,Relay,In)] = Active`.

## 3. NodeView (`strategies/view.rs`)

`downstream: &HashSet<…>` is replaced by the borrowed link store plus role-scoped accessors the seams use:

```rust
pub struct NodeView<'a> {
    pub subscriptions: &'a BTreeSet<TopicId>,
    pub candidates: &'a BTreeMap<TopicId, BTreeSet<PeerId>>,
    pub links: &'a LinkStore,   // reads: sources() / sinks() / sinks_on(topic) / get() / inbound_scan()
    pub epoch_nonce: u64,
}

impl NodeView<'_> {
    /// (already_in, count) for In links of `role` on `topic` — the acceptance
    /// prelude's single scan, now role-scoped (FR-008a).
    pub fn inbound_scan(&self, role: LinkRole, emitter: &PeerId, topic: &TopicId) -> (bool, usize);
}
```

## 4. Wire (`message.rs`)

Every `ConnectionAction` variant gains `role: LinkRole`. `signed_bytes` layout (research R3): emitter key (len-prefixed) · action tag byte · topic (len-prefixed) · **role tag byte** (`0x00` Relay, `0x01` Publisher). The signature binds all four.

## 5. Strategy seams

| Seam | Trait method | Kinds | Slots |
|---|---|---|---|
| Link selection | `expected_links(&NodeView) -> BTreeSet<(PeerId, TopicId)>` | `none`, `connect-to-all`, `hash-gated` (`HashGatedSelection { role, … }`) | relay (default `connect-to-all`), publish (default `none`) |
| Acceptance | `admit(emitter, topic, &NodeView) -> Admission` | four baselines, role-instantiated (`AcceptanceParams { role, degree, … }`) | relay, publish (both default `accept-from-all`) |
| Fan-out | `targets(topic, &LinkStore, origin, exclude)` — the **model knob** | `forward-to-all` (default; **M3**), `role-scoped` (experiment variant), `role-agnostic` (**M5**, no role distinction) | one |

Params (`strategies/config.rs`): `SelectionParams { self_id, role, degree, bucket_count }`, `AcceptanceParams { self_id, role, degree, bucket_count, cap_buffer }` — `role` picks the hash domain (`edge/v1` vs `publish-edge/v1`) and the degree-flag names in errors. Standing initiation links select **unconditionally** (`m3/README.md`; the trigger of the earlier draft is superseded — ADR 0034). Both params carry `symmetric: bool` (the M4 mode, `--symmetric-edges` — relay seams only); the receive gate carries `PublishInAdmission` (`owner-only` | `any-verified`, the M5 gate — ADR 0035).

## 6. Transitions touched (`state.rs`)

| Transition | Change |
|---|---|
| `handle_heartbeat` | after the relay dial diff, the publish dial pass: `publish_selection.expected_links` (unconditional) → `(p,t,Publisher,Out) = AwaitingAccept` + `Request{role: Publisher}` |
| `handle_connection_request` | dispatch on carried role → relay vs publish acceptance slot; accept records `(emitter, topic, role, In) = Active`, replies `Accepted{role}` |
| `handle_connection_accepted/rejected` | match the `(emitter, topic, role, Out)` entry |
| `handle_connection_terminated` | remove the `(emitter, topic, role, ·)` entries (both directions of that role) |
| `handle_dissemination` | gate: `Active (from,t,Relay,Out)` OR (`(from,t,Publisher,In)` AND `publisher_id == from`) — else drop (`not_connected` / `relay_over_publish_link`, R5) |
| `record_and_fanout` / `fanout` | thread `Origin` into the seam (FR-005) |
| `handle_shutdown` | one `Terminated{role}` per held entry, ordered by the BTreeMap key |
| `handle_topic_registry_update(Removed)` | cascade: `links.retain(|(_, t, _, _), _| t != &topic)` |

## 7. Public surface (`node.rs`, `lib.rs`)

- `Node::upstream_connections()` / `Node::downstream_connections()` — **preserved semantics** as relay-scoped views (`Relay`/`Out` triples, `Relay`/`In` pairs) so existing tests and callers observe identical behaviour (US1).
- NEW `Node::links()` → full snapshot `Vec<(PeerId, TopicId, LinkRole, LinkDirection, LinkState)>` (the SC observation surface).
- `LinkRole`, `LinkDirection`, `LinkState` exported; `UpstreamState` name retired (call-site rename; behaviour identical).
- CLI: `--relay-degree` (renamed), `--relay-strategy`/`--publish-strategy` (one kind family; defaults `connect-to-all`/`none`), `--publish-degree`, `--publish-acceptance-strategy` (`accept-from-all` default), `--fanout-strategy` (`forward-to-all` | `role-scoped` | `role-agnostic`), `--symmetric-edges` (M4), `--publish-in-admission` (`owner-only` default | `any-verified`, M5). `--bucket-count`/`--cap-buffer` shared knobs.

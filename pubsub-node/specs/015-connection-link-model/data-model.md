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

## 2. The store (`state.rs`, replacing `upstream` + `downstream`)

```rust
links: BTreeMap<(PeerId, TopicId, LinkRole, LinkDirection), LinkState>,
```

- Keyed including role: a `Relay` and a `Publisher` link between the same pair coexist independently (Clarifications 2026-07-13).
- Keyed including direction: dial + accept between the same pair (today's upstream∩downstream case) stay two entries.
- `BTreeMap`: deterministic iteration (shutdown notices, snapshots). Requires `Ord` on the key components (`LinkRole`, `LinkDirection` derive it; `PeerId`, `TopicId` already have it).
- Terminal outcomes are removals (no closed variant) — unchanged rule.

**Migration mapping** (FR-002/003/004): `upstream[(p,t)] = s` → `links[(p,t,Relay,Out)] = s`; `downstream ∋ (p,t)` → `links[(p,t,Relay,In)] = Active`.

## 3. NodeView (`strategies/view.rs`)

`downstream: &HashSet<…>` is replaced by the borrowed link store plus role-scoped accessors the seams use:

```rust
pub struct NodeView<'a> {
    pub subscriptions: &'a BTreeSet<TopicId>,
    pub candidates: &'a BTreeMap<TopicId, BTreeSet<PeerId>>,
    pub links: &'a BTreeMap<(PeerId, TopicId, LinkRole, LinkDirection), LinkState>,
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

| Seam | Trait method (after) | v1 kinds |
|---|---|---|
| Relay selection | `expected_relay(&NodeView) -> BTreeSet<(PeerId, TopicId)>` (renamed from `expected_upstream` for role symmetry, analysis A7; `relay_degree` rename) | `connect-to-all`, `hash-gated` |
| **Publish selection (NEW)** | `expected_publish(&NodeView) -> BTreeSet<(PeerId, TopicId)>` — internally applies the M3 trigger per topic (R6) | `none` (default), `hash-gated` |
| Acceptance | `admit(emitter, topic, &NodeView) -> Admission` — one slot per role; role dispatch in the handler | four baselines × role instantiation |
| Fan-out | `targets(topic, links, origin, exclude) -> Vec<PeerId>` (origin-aware, FR-005) | `forward-to-all` |

Publish-side parameters (`strategies/config.rs`): `PublishParams { self_id, publish_degree: Option<usize>, bucket_count: Option<usize> }`, `PublishAcceptanceParams { self_id, publish_degree, bucket_count, cap_buffer }`. `relay_degree` replaces `target_degree` in the relay params. The publish predicate lives beside the relay one in `strategies::edge` under domain `pubsub/bucketed-pull/publish-edge/v1` (relay domain bytes unchanged).

## 6. Transitions touched (`state.rs`)

| Transition | Change |
|---|---|
| `handle_heartbeat` | after the relay dial diff, a publish dial pass: `expected_publish` → create `(p,t,Publisher,Out) = AwaitingAccept` + send `Request{role: Publisher}` |
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
- CLI: `--relay-degree` (renamed), `--publish-strategy` (`none` default), `--publish-degree`, `--publish-acceptance-strategy` (`accept-from-all` default). `--bucket-count` and `--cap-buffer` apply per seam as today (shared knobs).

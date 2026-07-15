# Data model: publisher links and dissemination-model configurations (015)

## 1. New shapes (the only two)

```rust
/// Which dissemination class a link belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum LinkKind {
    /// The pull-based relay mesh (existing behaviour).
    Relay,
    /// A standing link carrying, by default, only its owner's own publications.
    Publisher,
}

/// The key of one link: topic-first so derived `Ord` clusters a topic's links
/// contiguously in a `BTreeMap` (per-topic reads are range walks).
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct LinkKey {
    pub topic: TopicId,
    pub peer: PeerId,
    pub kind: LinkKind,
}
```

`UpstreamState` is renamed `LinkState` (same two variants, same semantics).
`PublisherAdmission { OwnerOnly, AnyVerified }` is a config enum (R8), not a
link shape.

## 2. NodeState changes

| Field | Before | After |
|---|---|---|
| `upstream` | `HashMap<(PeerId, TopicId), UpstreamState>` | `BTreeMap<LinkKey, LinkState>` — peers the node **receives from** |
| `downstream` | `HashSet<(PeerId, TopicId)>` | `BTreeMap<LinkKey, LinkState>` — peers the node **sends to** |
| `publisher_strategy` | — | `Option<Arc<dyn ConnectionStrategy>>` |
| `publisher_acceptance` | — | `Option<Arc<dyn ConnectionAcceptanceStrategy>>` |
| `publisher_admission` | — | `PublisherAdmission` (default `OwnerOnly`) |

### Field × kind invariants (doc-comment contracts, enforced by the handlers)

| Collection | Kind | Meaning | Lifecycle |
|---|---|---|---|
| `upstream` | Relay | my pull dials (message sources) | `AwaitingAccept` → `Active` |
| `upstream` | Publisher | accepted inbound publisher links | inserted `Active` |
| `downstream` | Relay | accepted relay peers (fan-out destinations) | inserted `Active` |
| `downstream` | Publisher | my publisher dials (own-publication targets) | `AwaitingAccept` → `Active` |

A peer may hold entries of both kinds in the same collection for one topic;
they coexist and are mutated independently (spec FR-001/FR-015).

## 3. Transitions (delta per handler)

- **`handle_heartbeat`** — unchanged relay diff over `upstream` × Relay; then,
  if `publisher_strategy` is `Some`, the same diff pattern over `downstream` ×
  Publisher (insert `AwaitingAccept`, send `Request` with kind Publisher).
  Both passes behind the one `synced` gate. The publisher pass never reads the
  relay entries (unconditional, FR-002).
- **`handle_connection_request`** — dispatch on the carried kind. Relay: as
  today (acceptance strategy → insert `downstream` × Relay `Active`, reply
  `Accepted`). Publisher: if `publisher_acceptance` is `None`, silent drop
  (`publisher_links_disabled`); else the same admission mechanics inserting
  `upstream` × Publisher `Active`.
- **`handle_connection_accepted`** — kind Relay activates the matching
  `upstream` × Relay `AwaitingAccept`; kind Publisher activates `downstream` ×
  Publisher. Unsolicited (absent/already-Active) drops as today.
- **`handle_connection_rejected`** — removes the matching `AwaitingAccept`
  from the *dialed* collection for the carried kind (Relay: `upstream`;
  Publisher: `downstream`).
- **`handle_connection_terminated`** — removes the `(peer, topic, kind)` entry
  from both collections (whichever hold it); unknown termination drops.
- **`handle_dissemination`** — the admission gate becomes:
  1. `upstream` × Relay `Active` for `(from, topic)` → admitted (as today), or
  2. `upstream` × Publisher present for `(from, topic)` **and**
     (`publisher_admission == AnyVerified` or the message's publisher key ==
     `from`'s key) → admitted;
  otherwise `not_connected` drop. On signature failure past all checks, sever
  the **admitting** `LinkKey` (FR-010) and emit `Misbehaved`.
- **`handle_publish` / `record_and_fanout`** — unchanged flow; `fanout()`
  passes the delivery's `Origin` through to the strategy.
- **`handle_topic_registry_update` (Removed cascade)** and
  **`handle_shutdown`** — retain/clear + `Terminated` notices now iterate both
  maps; the notice carries each entry's kind.

## 4. NodeView

```rust
pub struct NodeView<'a> {
    pub subscriptions: &'a BTreeSet<TopicId>,
    pub candidates: &'a BTreeMap<TopicId, BTreeSet<PeerId>>,
    pub upstream: &'a BTreeMap<LinkKey, LinkState>,   // NEW borrow
    pub downstream: &'a BTreeMap<LinkKey, LinkState>, // type change
    pub epoch_nonce: u64,
}
```

The acceptance helper `downstream_scan` generalises to
`link_scan(map, kind, emitter, topic) -> (already_present, count_on_topic)`
— the relay acceptance instance scans `downstream` × Relay (today's
semantics), the publisher instance scans `upstream` × Publisher. Caps stay
disjoint per kind (FR-004) because each instance only ever counts its own
kind.

## 5. Snapshots / getters (state and `Node`)

| Getter | Returns | Notes |
|---|---|---|
| `upstream_relays()` | `Vec<(PeerId, TopicId, LinkState)>` | rename of `upstream_snapshot` (kind-filtered) |
| `downstream_relays()` | `Vec<(PeerId, TopicId)>` | rename of `downstream_snapshot` (kind-filtered) |
| `upstream_publishers()` | `Vec<(PeerId, TopicId)>` | new; presence-only |
| `downstream_publishers()` | `Vec<(PeerId, TopicId, LinkState)>` | new; dial lifecycle |

On an M2-configured node the two relay getters return byte-identical results
to the old getters — the pre-existing suite's only edit is the call rename.

## 6. Wire

`PlainConnection` gains `kind: LinkKind`. `signed_bytes()` appends one tag
byte after the topic: `0x00` Relay, `0x01` Publisher (inside the signature).
Kind implies direction: a Relay `Request`'s dialer will receive; a Publisher
`Request`'s dialer will send. Layout-pin test updated to the new layout
(deliberate, R3).

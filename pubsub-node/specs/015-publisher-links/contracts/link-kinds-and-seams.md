# Contract: link kinds, seams, and configuration surface (015)

The externally observable surface this feature adds or changes. Everything
here is testable through public getters, wire bytes, or CLI behaviour — never
through logs.

## 1. Wire contract

The handshake kind is **message vocabulary** (ADR 0034): one connection
variant per handshake — `Message::RelayConnection` /
`Message::PublisherConnection` / `Message::SymmetricConnection`, each
carrying a `ConnectionMessage` over `PlainConnection { emitter, action }`.
`signed_bytes(kind)` layout (all multi-byte integers big-endian,
`push_len_prefixed` = u32 length + bytes):

1. emitter public key — length-prefixed
2. action tag — 1 byte: `0x00` Request, `0x01` Accepted, `0x02` Terminated, `0x03` Rejected
3. topic — length-prefixed UTF-8
4. **handshake-kind tag — 1 byte: `0x00` Relay, `0x01` Publisher, `0x02`
   Symmetric** (supplied from the enclosing variant; relay/publisher
   preimages byte-identical to the earlier kind-field encoding)

The signature binds emitter, action, topic, **and handshake kind**: a control
message cannot be replayed under another vocabulary. The handshake implies
data direction — Relay `Request`: dialer receives from acceptor; Publisher
`Request`: dialer sends to acceptor; Symmetric `Request`: one accept
establishes the relay-class link in both directions on both ends.

## 2. Strategy seams (existing traits, reused)

```rust
pub trait ConnectionStrategy: Send + Sync {
    /// The links this node should have dialed (renamed from expected_upstream:
    /// the relay instance's picks are dialed as upstream sources, the
    /// publisher instance's as downstream targets).
    fn expected_links(&self, view: &NodeView<'_>) -> BTreeSet<(PeerId, TopicId)>;
}

pub trait ConnectionAcceptanceStrategy: Send + Sync {
    fn admit(&self, emitter: &PeerId, topic: &TopicId, view: &NodeView<'_>) -> Admission;
}

pub trait FanoutStrategy: Send + Sync {
    fn targets(
        &self,
        topic: &TopicId,
        downstream: &BTreeMap<LinkKey, LinkState>,
        origin: &Origin,          // NEW: the recorded delivery's origin
        exclude: Option<&PeerId>, // split-horizon
    ) -> Vec<PeerId>;             // per-peer deduplicated
}
```

- Hash-gated selection/acceptance implementors carry `kind: LinkKind`
  (constructor default `Relay` — existing call sites unchanged) choosing the
  hash domain, and `symmetric: bool` (relay only) choosing the symmetric
  predicate.
- Publisher seam slots are `Option<Arc<dyn …>>`; `None` = feature off (M2).

## 3. Edge predicate domains (`strategies::edge`)

| Function | Domain tag | Draw |
|---|---|---|
| `is_valid_edge` | `pubsub/bucketed-pull/relay-edge/v1` (renamed from `…/edge/v1` — the tag became relay-exclusive; no experiment results existed to keep reproducible) | directional relay |
| `is_valid_edge_publisher` | `pubsub/bucketed-pull/publisher-edge/v1` | directional publisher |
| `is_valid_edge_sym` | `pubsub/bucketed-pull/edge-sym/v1` | unordered pair (canonical byte order), relay symmetric |

All three share `resolve_buckets` / `bucket_count` / `accept_cap` untouched.

## 4. Node public getters

`upstream_relays()`, `downstream_relays()`, `upstream_publishers()`,
`downstream_publishers()` — see data-model §5. `received()`, `candidates()`,
`subscriptions()`, `is_synced()` unchanged.

## 5. CLI contract

| Flag | Values / default | Semantics |
|---|---|---|
| `--relay-strategy` | `connect-to-all` (default) \| `hash-gated` \| `none` | rename of `--connection-strategy`; `none` = push-only |
| `--relay-acceptance-strategy` | `accept-from-all` (default) \| `bounded` \| `hash-gated` \| `hash-gated-bounded` \| `none` | rename of `--acceptance-strategy`; `none` = push-only |
| `--relay-degree` | int | rename of `--target-degree` |
| `--publisher-strategy` | absent (default) \| `connect-to-all` \| `hash-gated` | absent ⇒ node never dials publisher links |
| `--publisher-acceptance-strategy` | absent (default) \| same four kinds | absent ⇒ inbound publisher requests silently dropped |
| `--publisher-degree` | int | required by publisher `hash-gated` / bounded acceptance |
| `--fanout-strategy` | `forward-to-relays` (default) \| `forward-to-all` | M3 vs M5 — the **only** switch between them (the receive side is uniform) |
| ~~`--publisher-admission`~~ | — | removed (kind-agnostic receive gate; R8 superseded) |
| `--symmetric-edges` | flag | symmetric predicate on relay selection **and** acceptance together |
| `--genesis`, `--bucket-count`, `--cap-buffer` | unchanged | shared across seams |

## 6. Model recipes (per-node config; no preset)

| Model | Flags |
|---|---|
| **M1** (boundary) | `--relay-strategy none --relay-acceptance-strategy none` + the M5 publisher/fan-out flags (push-only = M5 at `k_in = 0`) |
| **M2** (baseline) | defaults — no publisher flags, `forward-to-relays` |
| **M3** | `--relay-strategy hash-gated --relay-acceptance-strategy hash-gated-bounded --relay-degree RF --publisher-strategy hash-gated --publisher-acceptance-strategy hash-gated-bounded --publisher-degree S_LINKS` |
| **M4 (approximation)** | `--relay-strategy hash-gated --relay-acceptance-strategy hash-gated --relay-degree RF --symmetric-edges` (no publisher flags) — constructed bidirectional links (ADR 0034), but binomial per-node degree; the exact M4 (min degree ≥ RF) additionally needs the uniform exactly-RF selection kind (follow-up feature) |
| **M5** | M3 flags with `--relay-degree K_IN --publisher-degree K_OUT --fanout-strategy forward-to-all` |

`--symmetric-edges` composes with capped acceptance (a capacity refusal
refuses the whole edge — one accept decision per symmetric link, ADR 0034).
M3 and M5 deliberately differ **only** in `--fanout-strategy`: the receive
side is uniform across the models (the kind-agnostic gate), so the comparison
isolates the fan-out axis.

**Parameter mapping caveat**: `--publisher-degree` is the expected number of
standing publisher **links**. The M3 model's *s* counts the intended initial
holders — the publisher **plus** its s−1 targets — so `S_LINKS = s − 1` when
parameterising from the model's tables.

## 7. Behavioural guarantees (test anchors)

- Publisher dials fire on the readiness heartbeat, unconditionally (FR-002).
- Kind-agnostic receive gate: any `Active` upstream entry admits; a
  publisher-link arrival is validated exactly like any message (FR-006 as
  amended — no owner-binding).
- Publisher links never carry relayed traffic under `forward-to-relays` (FR-005).
- Invalid signature severs the admitting link kind (FR-010).
- One send per peer regardless of coexisting link kinds (FR-011).
- Relay and publisher acceptance caps count independently (FR-004).
- `--symmetric-edges` yields reciprocal relay pairs on both ends — constructed
  by the handshake: one accept records both directions, teardown and
  severance remove both halves (FR-009, ADR 0034).

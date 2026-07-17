# Contract: link kinds, seams, and configuration surface (015)

The externally observable surface this feature adds or changes. Everything
here is testable through public getters, wire bytes, or CLI behaviour — never
through logs.

## 1. Wire contract

`PlainConnection { emitter, kind, action }`; `signed_bytes()` layout (all
multi-byte integers big-endian, `push_len_prefixed` = u32 length + bytes):

1. emitter public key — length-prefixed
2. action tag — 1 byte: `0x00` Request, `0x01` Accepted, `0x02` Terminated, `0x03` Rejected
3. topic — length-prefixed UTF-8
4. **kind tag — 1 byte: `0x00` Relay, `0x01` Publisher** (new)

The signature binds emitter, action, topic, **and kind**: a relay control
message cannot be replayed as a publisher one. Kind implies data direction —
Relay `Request`: dialer receives from acceptor; Publisher `Request`: dialer
sends to acceptor.

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
| `is_valid_edge` (unchanged) | `pubsub/bucketed-pull/edge/v1` | directional relay |
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
| `--relay-strategy` | `connect-to-all` (default) \| `hash-gated` | rename of `--connection-strategy` |
| `--relay-acceptance-strategy` | `accept-from-all` (default) \| `bounded` \| `hash-gated` \| `hash-gated-bounded` | rename of `--acceptance-strategy` |
| `--relay-degree` | int | rename of `--target-degree` |
| `--publisher-strategy` | absent (default) \| `connect-to-all` \| `hash-gated` | absent ⇒ node never dials publisher links |
| `--publisher-acceptance-strategy` | absent (default) \| same four kinds | absent ⇒ inbound publisher requests silently dropped |
| `--publisher-degree` | int | required by publisher `hash-gated` / bounded acceptance |
| `--fanout-strategy` | `forward-to-relays` (default) \| `forward-to-all` | M3 vs M5 send side |
| `--publisher-admission` | `owner-only` (default) \| `any-verified` | M3 vs M5 receive side |
| `--symmetric-edges` | flag | symmetric predicate on relay selection **and** acceptance together |
| `--genesis`, `--bucket-count`, `--cap-buffer` | unchanged | shared across seams |

## 6. Model recipes (per-node config; no preset)

| Model | Flags |
|---|---|
| **M2** (baseline) | defaults — no publisher flags, `forward-to-relays`, `owner-only` |
| **M3** | `--relay-strategy hash-gated --relay-acceptance-strategy hash-gated-bounded --relay-degree RF --publisher-strategy hash-gated --publisher-acceptance-strategy hash-gated-bounded --publisher-degree S_LINKS` |
| **M4** | `--relay-strategy hash-gated --relay-acceptance-strategy hash-gated --relay-degree RF --symmetric-edges` (no publisher flags) |
| **M5** | M3 flags with `--relay-degree K_IN --publisher-degree K_OUT --fanout-strategy forward-to-all --publisher-admission any-verified` |

M5's two switches must be paired network-wide (`forward-to-all` ⇄ `any-verified`);
deliberately not fused — the axes stay independently sweepable.

**Parameter mapping caveat**: `--publisher-degree` is the expected number of
standing publisher **links**. The M3 model's *s* counts the intended initial
holders — the publisher **plus** its s−1 targets — so `S_LINKS = s − 1` when
parameterising from the model's tables.

## 7. Behavioural guarantees (test anchors)

- Publisher dials fire on the readiness heartbeat, unconditionally (FR-002).
- Owner-binding under `owner-only`: a message over a publisher link from a
  non-owner is dropped `not_connected`-class, never recorded (FR-006).
- Publisher links never carry relayed traffic under `forward-to-relays` (FR-005).
- Invalid signature severs the admitting link kind (FR-010).
- One send per peer regardless of coexisting link kinds (FR-011).
- Relay and publisher acceptance caps count independently (FR-004).
- `--symmetric-edges` yields reciprocal relay pairs on both ends (FR-009).

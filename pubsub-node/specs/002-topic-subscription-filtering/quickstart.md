# Quickstart — Topics + Topic-Subscription Filtering

**Feature**: 002-topic-subscription-filtering
**Goal**: Reproduce the multi-topic 4-node demonstration (US2 / SC-002) in under one hour, per SC-004, without consulting any document outside this feature directory.

This quickstart layers on top of 001's substrate. If you have not yet reproduced 001's two-node Ping demo, run `../001-minimal-node-scaffold/quickstart.md` first (steps 1–2 take ~5 minutes once the toolchain is installed).

## Prerequisites

Same as 001: Rust stable ≥ 1.75, a POSIX shell, this repo checked out, working directory `pubsub-node/`. No new external dependencies.

## 1 — Build

```sh
cargo build
```

No new crates beyond 001's set — the dependency tree is unchanged. First post-001 build should be sub-second since only a handful of `.rs` files have grown.

## 2 — Run the topic-filter integration test (002 US1)

```sh
cargo test --test topic_filter
```

Expected:

```text
running 3 tests
test on_topic_message_retained ... ok
test off_topic_message_dropped_silently ... ok
test own_emission_not_in_local_snapshot ... ok

test result: ok. 3 passed; 0 failed; 0 ignored
```

Each test:

1. Builds an `Arc<InMemoryNetwork>`.
2. Constructs two `Node`s with `PeerId`s `node-a` and `node-b`, **passing the initial subscription set as a `HashSet<TopicId>` to `Node::new`** (the new fourth parameter; see `contracts/library-api.md`).
3. Issues an emission via `node-b.send(&node-a.id(), Message { topic: …, payload: MessagePayload::Ping(N) }).await`.
4. Awaits delivery via `await_delivery(&node-a, node-b.id(), &expected, Duration::from_secs(1)).await` for the on-topic case; or asserts the absence of the dropped message via `assert!(node-a.received_messages().is_empty())` after a short settle window for the off-topic case.

To observe the drop log entry from US1's off-topic scenario:

```sh
cargo test --test topic_filter -- off_topic_message_dropped_silently --nocapture
```

You should see (among any test scaffolding output) a structured tracing line like:

```text
INFO pubsub_node::node: event=topic_drop self_id=node-a from=node-b topic=t2
```

— the canonical FR-011 drop event. The `event=topic_drop` marker is greppable; the field shape is documented in `contracts/library-api.md` under "tracing events emitted by 002".

## 3 — Run the multi-topic N-node graph test (002 US2 / SC-002)

```sh
cargo test --test n_node_graph
```

This file is shared with 001 (the existing 001 US2 tests continue to pass under the new envelope shape). The 002-added tests cover:

- **`four_node_star_three_topics_filtering`** — 4-node graph with `A={T1}`, `B={T1,T2}`, `C={T2,T3}`, `D={T3}`. Emit one Ping per topic addressed to each node; assert per-node snapshots match the topic intersection with each subscription set.
- **`four_node_star_100_send_topic_isolation`** — 4-node graph with the same subscriptions; 100 emissions distributed across 3 topics (deterministic seeded sequence per Engineering Standards "Reproducible tests"). Assert that every node's `received_messages()` is exactly `intended_deliveries ∩ subscriptions(node)` — zero false-positives, zero false-negatives, across the 100-send cross-cut. This is the test that lands SC-002.

## 4 — Run the dynamic-transition test (002 US3)

```sh
cargo test --test topic_runtime
```

Tests demonstrating the runtime subscribe / unsubscribe API:

- **`subscribe_makes_subsequent_message_visible`** — Node A starts with subscriptions `{T2}`; receives `Ping(_, T2)` but drops `Ping(_, T1)`. Calls `a.subscribe("t1".into())` (expects `SubscribeOutcome::Added`); next `Ping(_, T1)` arrival is retained.
- **`unsubscribe_makes_subsequent_message_dropped`** — symmetric: after subscribing, then `a.unsubscribe("t1".into())` (expects `UnsubscribeOutcome::Removed`); next `Ping(_, T1)` arrival is dropped.
- **`unsubscribe_does_not_remove_previously_retained`** — after the sequence above, the previously-retained `Ping` from the subscribed window remains in `a.received_messages()` (snapshot grows monotonically).
- **`idempotent_outcomes`** — re-subscribing returns `AlreadyPresent` without state change; re-unsubscribing returns `NotSubscribed` without state change. Verifies SC-005.

## 5 — Run the CLI binary with a multi-topic config (002 US4)

The CLI surface from 001 is unchanged. The TOML extension is the only delta.

```sh
mkdir -p /tmp/pubsub-quickstart-002
cat > /tmp/pubsub-quickstart-002/node-a.toml <<'EOF'
[[peers]]
id = "node-b"

[[peers]]
id = "node-c"

subscribed_topics = ["governance/announcements", "defi/intents"]
EOF
```

Run:

```sh
cargo run -- --self-id node-a --config /tmp/pubsub-quickstart-002/node-a.toml
```

At default `--log-level info`, you should immediately see the node's startup banner (from 001) but no additional 002 events because no messages are flowing yet. To see what the startup parsing did, request debug output:

```sh
cargo run -- --self-id node-a --config /tmp/pubsub-quickstart-002/node-a.toml --log-level debug
```

To verify error reporting for the new `ConfigError::InvalidTopic` path (002 US4 AS-4):

```sh
cat > /tmp/pubsub-quickstart-002/broken.toml <<'EOF'
[[peers]]
id = "node-b"

subscribed_topics = ["valid", ""]
EOF
cargo run -- --self-id node-x --config /tmp/pubsub-quickstart-002/broken.toml
echo "exit code: $?"   # expect 2 (same as 001 US3 AS-2)
```

Expected: a `pubsub-node: config invalid topic entry: …` message on stderr; exit code 2.

To verify 002 US4 AS-5 (unknown top-level field continues to fail under `deny_unknown_fields`):

```sh
cat > /tmp/pubsub-quickstart-002/extra-field.toml <<'EOF'
[[peers]]
id = "node-b"

subscribed_topics = ["t1"]

unexpected_field = "value"
EOF
cargo run -- --self-id node-x --config /tmp/pubsub-quickstart-002/extra-field.toml
echo "exit code: $?"   # expect 2
```

Expected: a `pubsub-node: failed to parse TOML config …` message on stderr (the `toml::de::Error` names `unexpected_field`); exit code 2.

To observe the duplicate-topic warn (per FR-010, one event per duplicated entry; node starts successfully):

```sh
cat > /tmp/pubsub-quickstart-002/dup-topics.toml <<'EOF'
[[peers]]
id = "node-b"

subscribed_topics = ["t1", "t2", "t1"]
EOF
cargo run -- --self-id node-x --config /tmp/pubsub-quickstart-002/dup-topics.toml
```

Expected: a `WARN pubsub_node::config: event=topic_config_duplicate topic=t1 config_path=/tmp/pubsub-quickstart-002/dup-topics.toml` entry on stderr at startup. The node continues running with its in-memory subscription set `{t1, t2}`; duplicates are NOT a startup failure (contrast with the invalid-topic case above).

## 6 — Run config_loading.rs

```sh
cargo test --test config_loading
```

This existing 001 file gains six 002-specific cases (002 US4 acceptance scenarios; coverage matches data-model.md §7.5):

- `subscribed_topics_present_yields_initial_set` (AS-1)
- `subscribed_topics_absent_yields_empty_set` (AS-2)
- `subscribed_topics_empty_array_yields_empty_set` (AS-3)
- `invalid_topic_entry_yields_invalid_topic_error` (AS-4)
- `unknown_top_level_field_yields_parse_error` (AS-5)
- `duplicate_subscribed_topic_yields_dedup_set` (AS-6 — asserts on the deduplicated `subscriptions()` snapshot, not on the warn log; the log is operator UX exercised in `§5` above)

The existing 001 tests in this file continue to pass with the new envelope shape (their TOML inputs don't include `subscribed_topics`, so the field defaults to empty).

## 7 — Observability: the six new structured events

At default `--log-level info`, four events are visible (three info + one warn); the remaining two are debug-only. You can exercise the runtime ones by piping the runtime test through a non-capturing run:

```sh
cargo test --test topic_runtime -- --nocapture | grep "pubsub_node::node"
```

Expected event markers and when they fire:

| `event=` field | Level | Trigger |
|----------------|-------|---------|
| `topic_subscribed` | info | `subscribe(T)` returned `Added` (T was newly inserted) |
| `topic_unsubscribed` | info | `unsubscribe(T)` returned `Removed` (T was present and removed) |
| `topic_drop` | info | Receive task observed an inbound delivery with topic ∉ subscription set |
| `topic_config_duplicate` | warn | TOML loader detected a duplicate entry in `subscribed_topics` (one event per duplicated topic per load call) |
| `topic_subscribe_noop` | debug | `subscribe(T)` returned `AlreadyPresent` (idempotent re-subscribe) |
| `topic_unsubscribe_noop` | debug | `unsubscribe(T)` returned `NotSubscribed` (idempotent re-unsubscribe) |

For the operator triage workflow, the natural grep is `event=topic_drop` to find off-topic deliveries by sender and target topic, or `event=topic_config_duplicate` to find redundant TOML entries.

## 8 — Where things live (delta vs 001)

```text
pubsub-node/
├── src/
│   ├── peer.rs       # 001 — unchanged
│   ├── topic.rs      # NEW: TopicId, TopicIdError, FromStr, validation
│   ├── message.rs    # CHANGED: Message is now a struct { topic, payload }
│   │                 # MessagePayload (renamed from old Message enum)
│   ├── network.rs    # 001 — unchanged (FR-005 enforced by empty diff)
│   ├── node.rs       # CHANGED: subscriptions field, subscribe/unsubscribe,
│   │                 # subscriptions() getter, receive-path filter
│   ├── received.rs   # 001 — unchanged
│   ├── config.rs     # CHANGED: PeerListConfig → NodeConfig (rename per CHK017),
│   │                 # NodeConfig grows subscribed_topics, load_peer_list → load_node_config
│   ├── error.rs      # CHANGED: ConfigError grows InvalidTopic
│   ├── lib.rs        # CHANGED: re-exports TopicId, TopicIdError,
│   │                 # SubscribeOutcome, UnsubscribeOutcome, MessagePayload
│   └── main.rs       # CHANGED: passes parsed HashSet<TopicId> to Node::new
├── tests/
│   ├── two_node_ping.rs       # 001 — adapted to new envelope (tests still pass)
│   ├── n_node_graph.rs        # CHANGED: + 002 US2 / SC-002 tests
│   ├── topic_filter.rs        # NEW: 002 US1 acceptance scenarios
│   ├── topic_runtime.rs       # NEW: 002 US3 acceptance scenarios
│   ├── config_loading.rs      # CHANGED: + 002 US4 acceptance scenarios
│   └── common/mod.rs          # CHANGED: fixture builders accept subscription set
└── docs/decisions/
    └── 0008-subscription-mutator-shape.md   # NEW: ADR for FR-006/013/015 shape
```

Detailed contracts:

- `contracts/library-api.md` — 002 deltas to the Rust public surface.
- `contracts/node-config.toml.md` — 002 deltas to the TOML schema (file renamed from `peer-list.toml.md` per CHK017).
- 001's `contracts/cli.md` is inherited unchanged — no new CLI surface in 002.

Design context: `research.md` (the why behind each plan-level decision, including the envelope shape, concurrency primitive choice, and tracing field names). Data shapes: `data-model.md`.

## 9 — Common pitfalls (002 additions)

| Symptom | Likely cause |
|---------|--------------|
| Off-topic drop expected but message appears in `received_messages()` | The Node's initial subscription set includes the topic (check the test fixture / TOML). Or: the recv_task filter check is missing (Node implementation regression). |
| On-topic message expected but absent from `received_messages()` | (a) The fixture's subscription set is empty / missing the topic; (b) the test asserted before `await_delivery` resolved (race against the receive task — use the helper); (c) the topic supplied at message construction differs from the subscribed topic (string mismatch, including case sensitivity — `"T1"` ≠ `"t1"`). |
| `cargo build` fails with `Message::Ping(N)` errors after pulling 002 | Call sites still use 001's enum shape. Update to `Message { topic, payload: MessagePayload::Ping(N) }` (or use the `Message::ping(topic, n)` convenience constructor if implemented). |
| `subscriptions()` returns entries in unexpected order | Order is unspecified per FR-013 — sort the returned `Vec` before assertion, or compare as a `HashSet`. |
| `subscribe(T)` apparently has no effect on the next inbound message | Receive task processed the message before the mutator's lock release was observed. Per FR-015 the mutation IS linearized, but tests must use `await_delivery` so the assertion observes the post-mutation state, not a pre-mutation snapshot. |
| Debug-level `topic_subscribe_noop` / `topic_unsubscribe_noop` events not visible | At default `--log-level info` these events are filtered out by design — the info threshold suppresses debug. Re-run with `--log-level debug`. |
| TOML test fails with `ConfigError::InvalidTopic` instead of `Parse` | The TOML is syntactically valid; one of the `subscribed_topics` entries violates `TopicId::from_str` (empty string, contains NUL). Check the entry text. |

## 10 — Budget check (SC-004)

If you got this far in under an hour, SC-004 holds. If you spent more, please leave a note on the PR that lands 002 — the slow step is the signal the spec / quickstart most needs to improve.

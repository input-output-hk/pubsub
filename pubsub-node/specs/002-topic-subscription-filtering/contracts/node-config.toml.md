# Node-config TOML schema — 002 Deltas

**Feature**: 002-topic-subscription-filtering
**Source of truth**: `src/config.rs` (`NodeConfig`, renamed in 002 from 001's `PeerListConfig` per CHK017)
**Spec trace**: FR-010 (subscribed_topics field + duplicate-warn), FR-012 (parse at the edge), FR-002 (TopicId rules)

This contract documents **only what 002 adds** to the TOML schema, including the file/type rename. The 001 contract at `../001-minimal-node-scaffold/contracts/peer-list.toml.md` remains the canonical reference for the `peers` field and the loader's invariants for everything except `subscribed_topics`. The legacy "peer-list" name in the 001 contract path is preserved as a historical artifact; 002+ artifacts refer to the file as the "node-config TOML" and to the Rust type as `NodeConfig`.

---

## Schema additions (002)

```toml
# 002 addition: topics this node subscribes to at startup. Optional;
# absent or empty arrays are valid and yield an empty subscription set.
subscribed_topics = ["governance/announcements", "defi/intents"]

# 001 fields (unchanged):
[[peers]]
id = "node-b"

[[peers]]
id = "node-c"
```

> **Field-ordering note.** Per TOML's table-scoping rules, top-level bare keys (like `subscribed_topics`) must appear **before** any array-of-tables header (`[[peers]]`) or table header. Bare keys after such a header bind to that table's last entry — the parser would then see `subscribed_topics` as an unknown field inside the trailing `[[peers]]` entry and the load would fail with `ConfigError::Parse`. Every example below follows the "top-level keys first" ordering.

### Fields (additions only)

| Path | Type | Required | Notes |
|------|------|----------|-------|
| `subscribed_topics` | array of strings | optional | When omitted, an explicit empty array, or any other absence-of-content, the node starts with an empty subscription set. Each entry is parsed via `TopicId::from_str`; a rule violation surfaces as `ConfigError::InvalidTopic`. Duplicate entries are tolerated (the resulting in-memory `HashSet` deduplicates); per FR-010, the loader emits a warn-level structured tracing event (`event=topic_config_duplicate`, fields `topic` and `config_path`) for each duplicated entry detected during load — operator-facing misconfig signal, no startup failure. |
| `subscribed_topics[i]` (individual entry) | string | — | Non-empty UTF-8, no internal NUL byte (FR-002 — same rules as `peers[].id`). No additional character-class restrictions (no whitespace rule, no length cap). |

### Field placement

`subscribed_topics` is a **top-level** key, parallel to `peers`. Not nested under a `[subscriptions]` table; not nested under a `[node]` block. The placement decision is recorded in `research.md` §6 and was locked during the pre-spec chat (2026-05-29).

### What is intentionally NOT added by 002

- **No per-topic config block** (`[[subscribed_topics]] id = "…"`, with future fields like priority or retention). The shape is a plain string array. If future iterations need per-topic config, the field migrates to a table-array as a deliberate breaking change with an ADR — same forward-compatibility note 001 made for peers.
- **No "subscribe to all" wildcard**. `subscribed_topics = ["*"]` is parsed as a TopicId literal `"*"`, not as a wildcard. Wildcard semantics are deferred per the spec.
- **No CLI flag for topics**. The field rides inside the existing node-config TOML (renamed from 001's "peer-list TOML" per CHK017); the binary's existing `--config` flag is sufficient. No new CLI surface (FR-012; matches `contracts/cli.md` from 001 — no edit).
- **No version field on the schema**. Schema versioning remains deferred per 001's convention.

## Validation pipeline (loader: `config::load_node_config`) — extended

The loader gains step 4 below; steps 1–3 are inherited unchanged from 001.

1. Read file at `path` → `String`. On failure: `ConfigError::Io { path, source }`. (001)
2. `toml::from_str::<RawNodeConfig>(&content)` where `RawNodeConfig` is a shadow struct with `subscribed_topics: Vec<String>` (raw strings, no `TopicId` validation yet). On failure: `ConfigError::Parse { path, source }`. (001 pattern, extended).
3. For each `PeerEntry`, re-validate `id` through `PeerId::from_str`. On failure: `ConfigError::InvalidPeer(reason)`. (001)
4. **NEW**: for each `subscribed_topics` string, run `TopicId::from_str`. On failure: `ConfigError::InvalidTopic("{path}: {error}")`. Loader is fail-fast; the first invalid topic short-circuits subsequent topic validation in this load call.
5. Return `NodeConfig` with `peers: Vec<PeerEntry>` and `subscribed_topics: Vec<TopicId>` (validated).

Steps 3 and 4 are independent; their relative order is implementation-internal. The loader does NOT collect multiple errors across both fields (matches 001 precedent — first error wins).

## Examples

### Multi-topic subscriber (US1 / US2 setup)

```toml
# config/node-a.toml
subscribed_topics = ["t1", "t2"]

[[peers]]
id = "node-b"

[[peers]]
id = "node-c"
```

A node started with this config has its peer set `{node-b, node-c}` and its initial subscription set `{t1, t2}`. Messages with topic `t1` or `t2` from either peer are retained; messages with any other topic are silently dropped (FR-004 + FR-011 info log).

### Absent field (Edge Cases bullet — empty subscription set)

```toml
# config/observer.toml
# no subscribed_topics line at all

[[peers]]
id = "node-a"
```

Valid. Node starts with empty subscription set; every inbound message is dropped (each with an info-level log). The node may still emit (FR-008).

### Explicit empty array (equivalent to absent field)

```toml
# config/publisher-only.toml
subscribed_topics = []

[[peers]]
id = "node-a"
```

Equivalent to the absent-field case above. The two TOML shapes are indistinguishable to the loader; both yield `subscribed_topics: Vec::new()` in the parsed `NodeConfig`.

### Mixed peers + topics (002 US4 AS-1)

```toml
# config/node-w.toml
subscribed_topics = ["governance/announcements", "defi/intents"]

[[peers]]
id = "node-a"

[[peers]]
id = "node-b"
```

Standard configuration for an operator running a multi-topic subscriber.

### Invalid topic entry (002 US4 AS-4 negative path)

```toml
# Will fail with ConfigError::InvalidTopic (empty string)
subscribed_topics = ["t1", ""]
```

```toml
# Will fail with ConfigError::InvalidTopic (internal NUL byte)
subscribed_topics = ["t1", "bad\0topic"]
```

In both cases, the error message includes the file path and the underlying `TopicIdError` (`"topic id must not be empty"` / `"topic id must not contain a NUL byte"`). Startup fails; the node does not start with a partial subscription set.

### Duplicate topic entry (warn-on-load behavior)

```toml
# Loader warns once per duplicated topic; node starts successfully.
subscribed_topics = ["t1", "t2", "t1"]

[[peers]]
id = "node-a"
```

On load, the loader emits one warn-level tracing event per duplicate, e.g.:

```text
WARN pubsub_node::config: event=topic_config_duplicate topic=t1 config_path=/tmp/.../node.toml
```

The resulting in-memory subscription set is `{t1, t2}` — duplicates are absorbed by `HashSet` semantics. Startup succeeds; this is NOT a startup failure (contrast with the invalid-topic case above).

### Unknown top-level field (002 US4 AS-5)

```toml
subscribed_topics = ["t1"]
# Unknown field — rejected by deny_unknown_fields
some_future_field = "value"

[[peers]]
id = "node-a"
```

Startup fails with `ConfigError::Parse` (the underlying `toml::de::Error` names the unknown field). The strict-parsing contract from 001 (`#[serde(deny_unknown_fields)]` at the top level) continues to hold; adding `subscribed_topics` does not loosen it.

## Forward-compatibility note (002 additions)

The plain string array shape of `subscribed_topics` matches the data model — topics are just IDs at this stage. If a future feature needs per-topic config (priority, retention, fan-out policy), the migration path is the same one 001 left open for peers: change the field to a table-array (`[[subscribed_topics]] id = "…" priority = 5`), document the breaking change in an ADR, and update the loader. The 002 reader is strict (`deny_unknown_fields`) so any premature per-entry fields are surfaced as a parse error, not silently ignored.

> **Spec trace: FR-010.** The field name (`subscribed_topics`) and the plain-string-array shape are normative per FR-010; the `deny_unknown_fields` discipline is a contract-level best practice inherited from 001's precedent, not a new FR.

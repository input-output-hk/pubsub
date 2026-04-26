# pubsub-cli

Command-line tool for interacting with a running PubSub relay node.

## Purpose

Publish messages to topics and subscribe to receive them. Connects directly to a node's QUIC endpoint, signs published messages with an ephemeral Ed25519 key, encodes them as CBOR, and uses the wire protocol defined in `pubsub-types::traits` (`PUBLISH` + `SUBSCRIBE` tags). No HTTP fallback.

## Usage

```
pubsub-cli [OPTIONS] <COMMAND>
```

## Global options

| Flag | Default | Description |
|------|---------|-------------|
| `--node` | `127.0.0.1:9000` | Address of the relay node to connect to |

## Topic targeting

`publish` and `subscribe` both require **exactly one** of:

| Flag | Form | When to use |
|------|------|-------------|
| `--topic <name>` | TopicId = `Blake2b-256(name)` | Off-chain / mock-chain testing — node must be running without a live chain backend, or the topic must coincidentally hash-match a chain entry (vanishingly unlikely) |
| `--topic-id <u64>` | TopicId = `[BE u64; zeros]` (32 bytes) | Chain-registered topics — the on-chain `TopicDatum` stores `topic_id` as an integer, encoded into bytes 0..8 of the Rust `TopicId` |

Supplying both errors out at parse time (clap `conflicts_with`); supplying neither errors with `required_unless_present`. See the **TopicId conventions** section in `crates/pubsub-types/README.md` for the full encoding contract.

## Commands

### `publish`

```sh
pubsub-cli --node <ADDR> publish (--topic <NAME> | --topic-id <U64>) --message <TEXT> [--credential-type <KIND>]
```

The CLI generates a fresh ephemeral Ed25519 keypair, builds a `Message`, signs it, and runs a `PUBLISH` bidirectional exchange with the node. The node validates (signature + topic registry + authorised publishers), commits the message locally (`HotCache` + dashboard ringbuffer + broadcast fan-out), dispatches it to the dissemination layer, and returns a `PublishAck`.

| Outcome | CLI exit | Output |
|---------|----------|--------|
| `PublishAck::Accepted{topic_id, sequence_nr}` | 0 | `Accepted topic_id=… seq=… (<label>): <payload>` |
| `PublishAck::Rejected{reason}` | non-zero | `Error: Rejected by node (<label>): <reason>` (e.g. `TopicNotFound`, `InvalidSignature`, `Unauthorized`) |
| Connection failure | non-zero | `Error: Failed to connect to <addr>: …` |

Credential types (`--credential-type <KIND>`, default `ed25519`):

| Kind | Validator behaviour | Phase-1 status |
|------|--------------------|----------------|
| `ed25519` | Looks up `TopicConfig.authorized_publishers` on chain. Empty list ⇒ open topic, any signed key accepted. Non-empty list ⇒ key must be present. | Works against `MockChainState` and `CardanoChainState` |
| `pool-kes` | Verifies the key is in `ChainState::get_pool_kes_keys()`. | `CardanoChainState::get_pool_kes_keys` is a `todo!()` stub — panics against a real chain backend |
| `drep` | Verifies the key is in `ChainState::get_drep_keys()`. | Same — `todo!()` stub |
| `authority` | Verifies the key is in `ChainState::get_authority_keys()`. | Same — `todo!()` stub |

### `subscribe`

```sh
pubsub-cli --node <ADDR> subscribe (--topic <NAME> | --topic-id <U64>) [--since-seq <N>] [--limit <N>]
```

Opens a long-lived `SUBSCRIBE` bidirectional stream. The CLI sends one CBOR-encoded `SubscribeRequest{topic_id, since_seq, limit}`. The node first replays from `HotCache::get_since(topic_id, since_seq, limit)`, then forwards live `subscriber_tx` broadcast hits whose topic matches. Each frame is one CBOR-encoded `Message`; the CLI decodes and pretty-prints it. Loop until the peer finishes the stream or the user kills the process.

| Flag | Default | Description |
|------|---------|-------------|
| `--since-seq` | `0` | Replay starts from sequence numbers strictly greater than this. `0` ⇒ full TTL window held by the node's HotCache (default 1h). |
| `--limit` | `1000` | Soft cap on the replay batch. Live phase is unbounded. |

### `status`

```sh
pubsub-cli --node <ADDR> status
```

Not yet implemented — placeholder. Use `curl http://<node>:<http_port>/api/status` instead (default `http_port = quic_port + 1000`).

## Examples

```sh
# Off-chain local testing (mock chain, hash-form TopicId)
pubsub-cli --node 127.0.0.1:9001 publish --topic foo/bar --message "hello"

# Chain-registered topic on preprod (int-form TopicId)
pubsub-cli --node 127.0.0.1:9001 publish --topic-id 0 --message "alert"

# Subscribe and replay everything currently in cache, then go live
pubsub-cli --node 127.0.0.1:9001 subscribe --topic-id 0

# Subscribe to live messages only (no replay)
pubsub-cli --node 127.0.0.1:9001 subscribe --topic-id 0 --since-seq 999999999

# Detect rejection in scripts
if pubsub-cli --node 127.0.0.1:9001 publish --topic-id 99 --message "x"; then
  echo "accepted"
else
  echo "rejected (expected — topic 99 not registered)"
fi
```

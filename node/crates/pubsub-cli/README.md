# pubsub-cli

Command-line tool for interacting with a running PubSub relay node.

## Purpose

Send messages to topics and (in future) subscribe to receive them. Connects directly to a node's QUIC endpoint, signs the message with an ephemeral Ed25519 key, encodes it as CBOR, and sends it over the wire.

## Usage

```
pubsub-cli [OPTIONS] <COMMAND>
```

## Global options

| Flag | Default | Description |
|------|---------|-------------|
| `--node` | `127.0.0.1:9000` | Address of the relay node to connect to |

## Commands

### `publish`

Publish a message to a topic.

```bash
pubsub-cli --node 127.0.0.1:9001 publish --topic <NAME> --message <TEXT>
```

| Flag | Description |
|------|-------------|
| `--topic` | Topic name (hashed to `TopicId` with BLAKE2b) |
| `--message` | UTF-8 payload string |

The CLI generates a fresh ephemeral keypair per invocation and signs the message. For open topics (no authorized-publisher list) this is accepted by all nodes.

### `subscribe`

```bash
pubsub-cli --node 127.0.0.1:9001 subscribe --topic <NAME>
```

Not yet implemented. Full subscription requires a gRPC streaming API. To receive messages for a topic, run `pubsub-node --topics <NAME>` directly.

### `status`

```bash
pubsub-cli --node 127.0.0.1:9001 status
```

Not yet implemented. Requires a gRPC management API.

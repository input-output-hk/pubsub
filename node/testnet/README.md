# testnet

Scripts for running a local multi-node PubSub testnet.

## Purpose

Spins up N relay nodes on `localhost`, all subscribing to the same topics.
Node 1 acts as seed; all others bootstrap from it via `--peers`.

## Files

| File | Description |
|------|-------------|
| `launch.sh` | Launches N nodes in the background (default: 5) |
| `logs/` | Per-node log files (`node-1.log` … `node-N.log`) and PID files |
| `keys/` | Persistent Ed25519 key files generated on first run |

## Quick start

```bash
# Build and launch 5 nodes
./testnet/launch.sh --build

# Launch only (if already built)
./testnet/launch.sh

# Launch a different number of nodes
./testnet/launch.sh --nodes 10
./testnet/launch.sh -n 3
```

Press **Ctrl-C** to stop all nodes. Logs are in `testnet/logs/`.

## Publishing a test message

```bash
pubsub-cli --node 127.0.0.1:9001 publish \
  --topic ops/emergency/critical \
  --message "hello testnet"
```

All nodes should log `Delivered message to local subscriber`.

## Configuration

Edit the variables at the top of `launch.sh`:

| Variable | Default | Description |
|----------|---------|-------------|
| `NUM_NODES` | `5` | Default number of nodes (override with `--nodes N`) |
| `BASE_PORT` | `9001` | First QUIC port; nodes use `BASE_PORT` … `BASE_PORT + N - 1` |
| `TOPICS` | `ops/emergency/critical,...` | Comma-separated topic names all nodes subscribe to |

HTTP dashboards: `http://localhost:10001` (node 1) through `http://localhost:1000N`.

The testnet uses the mock chain state — no Blockfrost key or on-chain contracts needed.

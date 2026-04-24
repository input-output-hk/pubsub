# testnet

Scripts and configuration for running a 5-node local PubSub testnet.

## Purpose

Spins up 5 relay nodes on `localhost` ports 9001–9005, all subscribing to the same topics, using `nodes.json` for peer discovery instead of manual `--peers` flags.

## Files

| File | Description |
|------|-------------|
| `launch.sh` | Generates `nodes.json`, then launches 5 nodes in the background |
| `nodes.json` | Generated automatically by `launch.sh`; lists all node addresses and their topics |
| `logs/` | Per-node log files (`node-1.log` … `node-5.log`) and PID files |

## Quick start

```bash
# Build and launch
./testnet/launch.sh build

# Launch only (if already built)
./testnet/launch.sh
```

Press **Ctrl-C** to stop all nodes. Logs are in `testnet/logs/`.

## Publishing a test message

```bash
pubsub-cli --node 127.0.0.1:9001 publish \
  --topic ops/emergency/critical \
  --message "hello testnet"
```

All 5 nodes should log `Delivered message to local subscriber`.

## nodes.json

`launch.sh` regenerates this file on every run from the `TOPICS` and `NUM_NODES` variables at the top of the script. To change topics or node count, edit those variables — `nodes.json` is derived, not hand-maintained.

**Format:**
```json
{
  "nodes": [
    {
      "addr": "127.0.0.1:9001",
      "public_key": null,
      "subscribed_topics": ["ops/emergency/critical", "gov/drep/test", "dapp/test/notifications"]
    }
  ]
}
```

## Configuration

Edit the variables at the top of `launch.sh`:

| Variable | Default | Description |
|----------|---------|-------------|
| `NUM_NODES` | `5` | Number of nodes to launch |
| `BASE_PORT` | `9001` | First port; nodes use `BASE_PORT` … `BASE_PORT + NUM_NODES - 1` |
| `TOPICS` | `ops/emergency/critical,...` | Comma-separated topic names all nodes subscribe to |

#!/bin/bash
# Launch a local PubSub testnet.
#
# Usage: ./testnet/launch.sh [--nodes N] [--build]
#   --nodes N   number of nodes to start (default: 5)
#   --build     build the binary before launching
#
# Nodes bind to ports 9001-900N on localhost.
# Node 1 acts as seed; all others bootstrap from it.
# All nodes subscribe to the topics defined in TOPICS.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
NODE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
LOG_DIR="$SCRIPT_DIR/logs"

NUM_NODES=5
BUILD=false
BASE_PORT=9001
TOPICS="ops/emergency/critical,gov/drep/test,dapp/test/notifications"

# ── Parse args ───────────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --nodes|-n)
            NUM_NODES="$2"
            shift 2
            ;;
        --build|build)
            BUILD=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [--nodes N] [--build]"
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

if ! [[ "$NUM_NODES" =~ ^[1-9][0-9]*$ ]]; then
    echo "Error: --nodes must be a positive integer, got '$NUM_NODES'" >&2
    exit 1
fi

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# ── Build ────────────────────────────────────────────────────────────────────

if $BUILD; then
    echo -e "${BLUE}Building PubSub node...${NC}"
    cd "$NODE_DIR"
    cargo build --release
    echo -e "${GREEN}Build complete.${NC}"
fi

KEYS_DIR="$SCRIPT_DIR/keys"
mkdir -p "$LOG_DIR" "$KEYS_DIR"

SEED_ADDR="127.0.0.1:${BASE_PORT}"

# ── Find binary (prefer newest of release/debug) ─────────────────────────────

RELEASE_BIN="$NODE_DIR/target/release/pubsub-node"
DEBUG_BIN="$NODE_DIR/target/debug/pubsub-node"
BINARY=""

if [ -f "$RELEASE_BIN" ] && [ -f "$DEBUG_BIN" ]; then
    # Pick whichever was compiled more recently
    if [ "$RELEASE_BIN" -nt "$DEBUG_BIN" ]; then
        BINARY="$RELEASE_BIN"
    else
        BINARY="$DEBUG_BIN"
    fi
elif [ -f "$RELEASE_BIN" ]; then
    BINARY="$RELEASE_BIN"
elif [ -f "$DEBUG_BIN" ]; then
    BINARY="$DEBUG_BIN"
fi

if [ -z "$BINARY" ]; then
    echo -e "${RED}Error: pubsub-node binary not found. Run with 'build' argument first.${NC}"
    exit 1
fi

echo -e "${BLUE}Using binary: $BINARY${NC}"

# ── Cleanup on exit ──────────────────────────────────────────────────────────

cleanup() {
    echo -e "\n${YELLOW}Shutting down all nodes...${NC}"
    for pid_file in "$LOG_DIR"/*.pid; do
        if [ -f "$pid_file" ]; then
            pid=$(cat "$pid_file")
            if kill -0 "$pid" 2>/dev/null; then
                kill "$pid"
                echo -e "  Stopped node (PID $pid)"
            fi
            rm "$pid_file"
        fi
    done
    echo -e "${GREEN}All nodes stopped.${NC}"
}
trap cleanup EXIT

# ── Launch nodes ─────────────────────────────────────────────────────────────

echo -e "${BLUE}Launching $NUM_NODES PubSub nodes...${NC}"
echo ""

HTTP_BASE_PORT=$((BASE_PORT + 1000))

for i in $(seq 1 $NUM_NODES); do
    port=$((BASE_PORT + i - 1))
    http_port=$((HTTP_BASE_PORT + i - 1))
    name="node-$i"
    log_file="$LOG_DIR/$name.log"

    if [[ $i -eq 1 ]]; then
        # Node 1 is the seed — starts with no bootstrap peers.
        echo -e "${GREEN}  Starting $name (seed) on QUIC :$port  HTTP :$http_port${NC}"
        $BINARY \
            --bind "127.0.0.1:$port" \
            --name "$name" \
            --topics "$TOPICS" \
            --key-file "$KEYS_DIR/node-$i.sk" \
            --http-port "$http_port" \
            --log-level debug \
            > "$log_file" 2>&1 &
        echo $! > "$LOG_DIR/$name.pid"
        # Give the seed node a moment to open its QUIC listener before
        # the other nodes try to connect.
        sleep 1
    else
        # Nodes 2..N only know the seed at startup; Cyclon fills the rest.
        echo -e "${GREEN}  Starting $name on QUIC :$port  HTTP :$http_port  (seed: $SEED_ADDR)${NC}"
        $BINARY \
            --bind "127.0.0.1:$port" \
            --name "$name" \
            --topics "$TOPICS" \
            --peers "$SEED_ADDR" \
            --key-file "$KEYS_DIR/node-$i.sk" \
            --http-port "$http_port" \
            --log-level debug \
            > "$log_file" 2>&1 &
        echo $! > "$LOG_DIR/$name.pid"
    fi
done

echo ""
echo -e "${GREEN}All $NUM_NODES nodes launched.${NC}"
echo -e "Seed node: $SEED_ADDR"
echo -e "Logs: $LOG_DIR/"
echo ""
echo -e "Nodes:"
for i in $(seq 1 $NUM_NODES); do
    port=$((BASE_PORT + i - 1))
    http_port=$((HTTP_BASE_PORT + i - 1))
    echo -e "  ${BLUE}node-$i${NC} → QUIC 127.0.0.1:$port  HTTP http://localhost:$http_port"
done
echo ""
echo -e "Topics: $TOPICS"
echo ""
echo -e "Dashboard: ${YELLOW}http://localhost:${HTTP_BASE_PORT}${NC}"
echo ""
echo -e "To publish a test message:"
echo -e "  ${YELLOW}pubsub-cli --node 127.0.0.1:9001 publish --topic ops/emergency/critical --message \"test alert\"${NC}"
echo ""
echo -e "Press Ctrl+C to stop all nodes."

# Monitor nodes and report any unexpected exits
monitor_nodes() {
    while true; do
        sleep 5
        for pid_file in "$LOG_DIR"/*.pid; do
            [ -f "$pid_file" ] || continue
            pid=$(cat "$pid_file")
            name=$(basename "$pid_file" .pid)
            if ! kill -0 "$pid" 2>/dev/null; then
                echo -e "${RED}  $name (PID $pid) exited unexpectedly — check $LOG_DIR/$name.log${NC}"
                rm -f "$pid_file"
            fi
        done
        # If all nodes gone, exit
        shopt -s nullglob
        pids=("$LOG_DIR"/*.pid)
        shopt -u nullglob
        [ ${#pids[@]} -eq 0 ] && { echo -e "${RED}All nodes have exited.${NC}"; exit 1; }
    done
}

monitor_nodes &
MONITOR_PID=$!
wait || true
kill "$MONITOR_PID" 2>/dev/null || true

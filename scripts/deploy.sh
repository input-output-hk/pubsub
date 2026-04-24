#!/usr/bin/env bash
# =============================================================================
# deploy.sh — Bootstrap PubSub Cardano contracts to a target network.
#
# What this does
# ──────────────
# 1. Applies the bootstrap_utxo parameter to the registry and node-registry
#    validators to produce deployment-specific script hashes.
# 2. Derives bech32 addresses and policy IDs using the Aiken CLI.
# 3. Submits two bootstrap transactions via cardano-cli:
#      a. topic-registry: mints the "registry_head" NFT, initialises datum.
#      b. node-registry:  mints the "node_registry_head" NFT, initialises datum.
# 4. Writes a .env.<network> config file for the pubsub-node binary.
#
# On redeployment after contract changes
# ───────────────────────────────────────
# If the Aiken source changes:
#   1. Run `aiken build` in each contract directory to regenerate plutus.json.
#   2. Re-run this script with a FRESH bootstrap UTxO (the previous one was
#      consumed; a new UTxO gives a different hash → different addresses, which
#      is correct for a fresh deployment of changed contracts).
# If only the parameters change (same code, different network):
#   Re-run with the same flags but a different --network and bootstrap UTxOs.
#
# Prerequisites
# ─────────────
#   aiken        ≥ 1.1   — parameter application and blueprint tooling
#   cardano-cli  ≥ 9.0   — address/tx build/sign/submit (Conway era)
#   python3      ≥ 3.10   — CBOR encoding (stdlib only, no pip deps)
#   xxd                  — hex encoding of token names
#
# Usage
# ─────
#   ./scripts/deploy.sh \
#     --network    preprod \
#     --socket     /tmp/node.socket \
#     --payment-key /path/to/payment.skey \
#     --payment-addr addr_test1... \
#     --topic-bootstrap <txhash>#<index> \
#     --node-bootstrap  <txhash>#<index>
#
# Network values: preprod | preview | mainnet
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
LIB_PY="$SCRIPT_DIR/lib/cbor.py"
CONTRACTS="$REPO_ROOT/contracts"

# ---------------------------------------------------------------------------
# Defaults and arg parsing
# ---------------------------------------------------------------------------
NETWORK=""
SOCKET_PATH="${CARDANO_NODE_SOCKET_PATH:-}"
PAYMENT_SKEY=""
PAYMENT_ADDR_STR=""
TOPIC_BOOTSTRAP_UTXO=""
NODE_BOOTSTRAP_UTXO=""
MIN_DEPOSIT_LOVELACE=2000000

usage() {
    grep '^# ' "$0" | sed 's/^# \{0,2\}//' | sed -n '/^Usage/,/^$/p'
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --network)         NETWORK="$2";              shift 2 ;;
        --socket)          SOCKET_PATH="$2";          shift 2 ;;
        --payment-key)     PAYMENT_SKEY="$2";         shift 2 ;;
        --payment-addr)    PAYMENT_ADDR_STR="$2";     shift 2 ;;
        --topic-bootstrap) TOPIC_BOOTSTRAP_UTXO="$2"; shift 2 ;;
        --node-bootstrap)  NODE_BOOTSTRAP_UTXO="$2";  shift 2 ;;
        --min-deposit)     MIN_DEPOSIT_LOVELACE="$2"; shift 2 ;;
        -h|--help)         usage ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

# ---------------------------------------------------------------------------
# Interactive prompts — ask for any missing required value
# ---------------------------------------------------------------------------

# prompt_if_missing <var_name> <prompt_text> [default_value]
# Reads from the terminal even if stdin is redirected.
prompt_if_missing() {
    local var="$1" prompt="$2" default="${3:-}"
    if [[ -n "${!var:-}" ]]; then return; fi          # already set via flag
    if [[ ! -t 0 && ! -t 2 ]]; then                   # no tty at all
        echo "Missing required value: $prompt" >&2; exit 1
    fi
    local display_prompt="$prompt"
    [[ -n "$default" ]] && display_prompt="$prompt [${default}]"
    local input
    read -r -p "${display_prompt}: " input </dev/tty
    input="${input:-$default}"
    if [[ -z "$input" ]]; then
        echo "Value required for: $prompt" >&2; exit 1
    fi
    printf -v "$var" '%s' "$input"
}

# Network — validate in a loop until the user gives a valid value
if [[ -z "$NETWORK" ]]; then
    while true; do
        prompt_if_missing NETWORK "Network (preprod/preview/mainnet)"
        case "$NETWORK" in preprod|preview|mainnet) break ;; esac
        echo "  Must be one of: preprod, preview, mainnet" >&2
        NETWORK=""
    done
fi

case "$NETWORK" in
    preprod) MAGIC=1;         MAINNET_FLAG="" ;;
    preview) MAGIC=2;         MAINNET_FLAG="" ;;
    mainnet) MAGIC=764824073; MAINNET_FLAG="--mainnet" ;;
    *) echo "Unknown network: $NETWORK (use preprod|preview|mainnet)" >&2; exit 1 ;;
esac
NETWORK_MAGIC="${MAINNET_FLAG:---testnet-magic $MAGIC}"

# Socket path — offer the env var as default if set
SOCKET_DEFAULT="${CARDANO_NODE_SOCKET_PATH:-}"
prompt_if_missing SOCKET_PATH "Node socket path (CARDANO_NODE_SOCKET_PATH)" "$SOCKET_DEFAULT"
export CARDANO_NODE_SOCKET_PATH="$SOCKET_PATH"

prompt_if_missing PAYMENT_SKEY        "Payment signing key file"
prompt_if_missing PAYMENT_ADDR_STR    "Payment address (bech32)"
prompt_if_missing TOPIC_BOOTSTRAP_UTXO "Topic-registry bootstrap UTxO (txhash#index)"
prompt_if_missing NODE_BOOTSTRAP_UTXO  "Node-registry bootstrap UTxO (txhash#index)"

# ---------------------------------------------------------------------------
# Tool checks
# ---------------------------------------------------------------------------
for tool in aiken cardano-cli python3 xxd; do
    command -v "$tool" &>/dev/null || { echo "Required tool not found: $tool" >&2; exit 1; }
done

# ---------------------------------------------------------------------------
# Working directory — cleaned up on exit
# ---------------------------------------------------------------------------
WORK="$(mktemp -d -t pubsub-deploy-XXXXXX)"
trap 'rm -rf "$WORK"' EXIT

log() { printf '[deploy] %s\n' "$*"; }
cbor() { python3 "$LIB_PY" "$@"; }

# Wrap compiledCode hex into a cardano-cli PlutusScriptV3 JSON file
make_script_file() {
    local blueprint="$1" title="$2" out="$3"
    local code
    code=$(python3 - <<EOF
import json, sys
bp = json.load(open('$blueprint'))
for v in bp['validators']:
    if v['title'] == '$title':
        print(v['compiledCode'])
        sys.exit(0)
print('', end='')  # empty = not found
EOF
)
    if [[ -z "$code" ]]; then
        echo "Validator '$title' not found in $blueprint" >&2; exit 1
    fi
    printf '{"type":"PlutusScriptV3","description":"","cborHex":"%s"}' "$code" > "$out"
}

# Hex-encode a UTF-8 token name (no newline)
token_name_hex() { printf '%s' "$1" | xxd -p -c 256; }

# ---------------------------------------------------------------------------
# Token names
# ---------------------------------------------------------------------------
REGISTRY_HEAD_HEX=$(token_name_hex "registry_head")
NODE_REGISTRY_HEAD_HEX=$(token_name_hex "node_registry_head")

# ---------------------------------------------------------------------------
# ══ TOPIC REGISTRY ══════════════════════════════════════════════════════════
# ---------------------------------------------------------------------------
log "── Topic registry ──────────────────────────────────────────────────"

# Step 1: Encode bootstrap UTxO as CBOR (OutputReference)
TOPIC_TX_HASH="${TOPIC_BOOTSTRAP_UTXO%#*}"
TOPIC_TX_IX="${TOPIC_BOOTSTRAP_UTXO#*#}"
TOPIC_BOOTSTRAP_CBOR=$(cbor output-ref "$TOPIC_TX_HASH" "$TOPIC_TX_IX")
log "bootstrap UTxO CBOR: $TOPIC_BOOTSTRAP_CBOR"

TOPIC_BP_RAW="$CONTRACTS/topic-registry/plutus.json"
TOPIC_BP_1="$WORK/topic-bp-1.json"  # after applying bootstrap_utxo to registry
TOPIC_BP_2="$WORK/topic-bp-2.json"  # after applying policy_id to topic
TOPIC_BP_3="$WORK/topic-bp-3.json"  # after applying policy_id to publisher

# Step 2: Apply bootstrap_utxo → registry validator
log "Applying bootstrap_utxo to registry validator..."
aiken blueprint apply \
    --in  "$TOPIC_BP_RAW" \
    --out "$TOPIC_BP_1" \
    --module registry --validator registry \
    "$TOPIC_BOOTSTRAP_CBOR"

# Step 3: Derive registry policy ID from the parameterized mint validator
REGISTRY_POLICY_ID=$(aiken blueprint policy \
    --in "$TOPIC_BP_1" \
    --module registry --validator registry)
log "Registry policy ID: $REGISTRY_POLICY_ID"

# Step 4: Encode policy ID as CBOR bytes (parameter for topic + publisher)
POLICY_ID_CBOR=$(cbor policy-id "$REGISTRY_POLICY_ID")
log "Policy ID CBOR: $POLICY_ID_CBOR"

# Step 5: Apply policy ID → topic and publisher validators
log "Applying registry_policy_id to topic validator..."
aiken blueprint apply \
    --in  "$TOPIC_BP_1" \
    --out "$TOPIC_BP_2" \
    --module topic --validator topic \
    "$POLICY_ID_CBOR"

log "Applying registry_policy_id to publisher validator..."
aiken blueprint apply \
    --in  "$TOPIC_BP_2" \
    --out "$TOPIC_BP_3" \
    --module publisher --validator publisher \
    "$POLICY_ID_CBOR"

# Step 6: Derive addresses
ADDR_FLAG="${MAINNET_FLAG:---mainnet}"
[[ "$NETWORK" != "mainnet" ]] && ADDR_FLAG=""

TOPIC_REGISTRY_ADDR=$(aiken blueprint address \
    --in "$TOPIC_BP_3" \
    --module registry --validator registry \
    ${ADDR_FLAG:-})

TOPIC_VALIDATOR_ADDR=$(aiken blueprint address \
    --in "$TOPIC_BP_3" \
    --module topic --validator topic \
    ${ADDR_FLAG:-})

PUBLISHER_VAULT_ADDR=$(aiken blueprint address \
    --in "$TOPIC_BP_3" \
    --module publisher --validator publisher \
    ${ADDR_FLAG:-})

log "topic-registry address: $TOPIC_REGISTRY_ADDR"
log "topic validator address: $TOPIC_VALIDATOR_ADDR"
log "publisher vault address: $PUBLISHER_VAULT_ADDR"

# Step 7: Build cardano-cli script file for the minting policy
REGISTRY_MINT_SCRIPT="$WORK/registry_mint.script.json"
make_script_file "$TOPIC_BP_3" "registry.registry.mint" "$REGISTRY_MINT_SCRIPT"

# Step 8: Encode initial datum (RegistryHeadDatum { counter: 0, epoch: 0 })
REGISTRY_HEAD_DATUM='{"constructor":0,"fields":[{"int":0},{"int":0}]}'

# Step 9: Build + sign + submit bootstrap tx for topic-registry
log "Building topic-registry bootstrap tx..."
TOPIC_TX="$WORK/topic_bootstrap.tx"
TOPIC_TX_SIGNED="$WORK/topic_bootstrap.tx.signed"

cardano-cli conway transaction build \
    $NETWORK_MAGIC \
    --tx-in "$TOPIC_BOOTSTRAP_UTXO" \
    --tx-out "${TOPIC_REGISTRY_ADDR}+2000000+1 ${REGISTRY_POLICY_ID}.${REGISTRY_HEAD_HEX}" \
    --tx-out-inline-datum-value "$REGISTRY_HEAD_DATUM" \
    --change-address "$PAYMENT_ADDR_STR" \
    --mint "1 ${REGISTRY_POLICY_ID}.${REGISTRY_HEAD_HEX}" \
    --mint-script-file "$REGISTRY_MINT_SCRIPT" \
    --mint-redeemer-value '{"constructor":0,"fields":[]}' \
    --out-file "$TOPIC_TX"

cardano-cli conway transaction sign \
    --tx-file "$TOPIC_TX" \
    --signing-key-file "$PAYMENT_SKEY" \
    $NETWORK_MAGIC \
    --out-file "$TOPIC_TX_SIGNED"

cardano-cli conway transaction submit \
    --tx-file "$TOPIC_TX_SIGNED" \
    $NETWORK_MAGIC

TOPIC_BOOTSTRAP_TXID=$(cardano-cli transaction txid --tx-file "$TOPIC_TX_SIGNED")
log "topic-registry bootstrap tx: $TOPIC_BOOTSTRAP_TXID"

# ---------------------------------------------------------------------------
# ══ NODE REGISTRY ════════════════════════════════════════════════════════════
# ---------------------------------------------------------------------------
log "── Node registry ────────────────────────────────────────────────────"

# Step 1: Encode node-registry bootstrap UTxO
NODE_TX_HASH="${NODE_BOOTSTRAP_UTXO%#*}"
NODE_TX_IX="${NODE_BOOTSTRAP_UTXO#*#}"
NODE_BOOTSTRAP_CBOR=$(cbor output-ref "$NODE_TX_HASH" "$NODE_TX_IX")
log "bootstrap UTxO CBOR: $NODE_BOOTSTRAP_CBOR"

NODE_BP_RAW="$CONTRACTS/node-registry/plutus.json"
NODE_BP_1="$WORK/node-bp-1.json"

# Step 2: Apply bootstrap_utxo → node_registry validator
log "Applying bootstrap_utxo to node_registry validator..."
aiken blueprint apply \
    --in  "$NODE_BP_RAW" \
    --out "$NODE_BP_1" \
    --module node_registry --validator node_registry \
    "$NODE_BOOTSTRAP_CBOR"

# Step 3: Derive node-registry policy ID and address
NODE_REGISTRY_POLICY_ID=$(aiken blueprint policy \
    --in "$NODE_BP_1" \
    --module node_registry --validator node_registry)

NODE_REGISTRY_ADDR=$(aiken blueprint address \
    --in "$NODE_BP_1" \
    --module node_registry --validator node_registry \
    ${ADDR_FLAG:-})

log "node-registry policy ID: $NODE_REGISTRY_POLICY_ID"
log "node-registry address:   $NODE_REGISTRY_ADDR"

# Step 4: Build script file for node_registry minting policy
NODE_REG_MINT_SCRIPT="$WORK/node_registry_mint.script.json"
make_script_file "$NODE_BP_1" "node_registry.node_registry.mint" "$NODE_REG_MINT_SCRIPT"

# Step 5: Encode initial datum
# NodeRegistryDatum { nodes: [], min_deposit_lovelace: <N>, epoch: 0 }
NODE_REG_DATUM=$(printf '{"constructor":0,"fields":[{"list":[]},{"int":%d},{"int":0}]}' "$MIN_DEPOSIT_LOVELACE")

# Step 6: Build + sign + submit bootstrap tx for node-registry
log "Building node-registry bootstrap tx..."
NODE_TX="$WORK/node_bootstrap.tx"
NODE_TX_SIGNED="$WORK/node_bootstrap.tx.signed"

cardano-cli conway transaction build \
    $NETWORK_MAGIC \
    --tx-in "$NODE_BOOTSTRAP_UTXO" \
    --tx-out "${NODE_REGISTRY_ADDR}+2000000+1 ${NODE_REGISTRY_POLICY_ID}.${NODE_REGISTRY_HEAD_HEX}" \
    --tx-out-inline-datum-value "$NODE_REG_DATUM" \
    --change-address "$PAYMENT_ADDR_STR" \
    --mint "1 ${NODE_REGISTRY_POLICY_ID}.${NODE_REGISTRY_HEAD_HEX}" \
    --mint-script-file "$NODE_REG_MINT_SCRIPT" \
    --mint-redeemer-value '{"constructor":0,"fields":[]}' \
    --out-file "$NODE_TX"

cardano-cli conway transaction sign \
    --tx-file "$NODE_TX" \
    --signing-key-file "$PAYMENT_SKEY" \
    $NETWORK_MAGIC \
    --out-file "$NODE_TX_SIGNED"

cardano-cli conway transaction submit \
    --tx-file "$NODE_TX_SIGNED" \
    $NETWORK_MAGIC

NODE_BOOTSTRAP_TXID=$(cardano-cli transaction txid --tx-file "$NODE_TX_SIGNED")
log "node-registry bootstrap tx: $NODE_BOOTSTRAP_TXID"

# ---------------------------------------------------------------------------
# ══ Write config file ════════════════════════════════════════════════════════
# ---------------------------------------------------------------------------
ENV_FILE="$REPO_ROOT/node/.env.${NETWORK}"
log "Writing $ENV_FILE ..."

BLOCKFROST_BASE="https://cardano-${NETWORK}.blockfrost.io/api/v0"
[[ "$NETWORK" == "mainnet" ]] && BLOCKFROST_BASE="https://cardano-mainnet.blockfrost.io/api/v0"

cat > "$ENV_FILE" <<EOF
# PubSub node Cardano config — generated by scripts/deploy.sh
# Network:  $NETWORK  (magic $MAGIC)
# Date:     $(date -u +"%Y-%m-%dT%H:%M:%SZ")
# topic-registry bootstrap:  $TOPIC_BOOTSTRAP_UTXO  → tx $TOPIC_BOOTSTRAP_TXID
# node-registry  bootstrap:  $NODE_BOOTSTRAP_UTXO   → tx $NODE_BOOTSTRAP_TXID

BLOCKFROST_BASE_URL=$BLOCKFROST_BASE
BLOCKFROST_PROJECT_ID=

# Bech32 script addresses (enterprise, no staking credential)
PUBSUB_TOPIC_REGISTRY_ADDR=$TOPIC_REGISTRY_ADDR
PUBSUB_TOPIC_VALIDATOR_ADDR=$TOPIC_VALIDATOR_ADDR
PUBSUB_PUBLISHER_VAULT_ADDR=$PUBLISHER_VAULT_ADDR
PUBSUB_NODE_REGISTRY_ADDR=$NODE_REGISTRY_ADDR

# Minting policy IDs (56 hex chars = 28 bytes)
# Used to filter tokens when reading UTxOs from Blockfrost
PUBSUB_REGISTRY_POLICY_ID=$REGISTRY_POLICY_ID
PUBSUB_NODE_REGISTRY_POLICY_ID=$NODE_REGISTRY_POLICY_ID

# Optional: Demeter utxorpc endpoint
DEMETER_API_KEY=
EOF

# ---------------------------------------------------------------------------
# Done
# ---------------------------------------------------------------------------
echo ""
echo "======================================================================="
echo "Deployment complete — $NETWORK"
echo "======================================================================="
echo "  topic-registry bootstrap tx: $TOPIC_BOOTSTRAP_TXID"
echo "  node-registry  bootstrap tx: $NODE_BOOTSTRAP_TXID"
echo ""
echo "  Config written to: $ENV_FILE"
echo ""
echo "Next steps:"
echo "  1. Fill in BLOCKFROST_PROJECT_ID in $ENV_FILE"
echo "  2. cp node/.env.$NETWORK node/.env"
echo "  3. cargo run -p pubsub-node --features cardano"
echo "======================================================================="

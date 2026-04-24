# pubsub-admin

CLI for deploying and managing PubSub Cardano smart contracts.

## Prerequisites

- [Aiken](https://aiken-lang.org/installation-instructions) — contract compiler
- A [Blockfrost](https://blockfrost.io/) project ID for the target network
- A funded Cardano payment key pair (any wallet, bech32 address)

## Install

```sh
cargo install --path node/crates/pubsub-admin
```

After installation, `pubsub-admin` is available on your PATH.

---

## Deployment walkthrough

This is a one-time setup per network. Each step builds on the previous one.

### 1. Generate a payment key pair

Use `cardano-cli` to create an enterprise (no staking) key pair:

```sh
cardano-cli address key-gen \
  --normal-key \
  --signing-key-file local/payment.skey \
  --verification-key-file local/payment.vkey

cardano-cli address build \
  --payment-verification-key-file local/payment.vkey \
  --testnet-magic 1 \          # preprod; use --mainnet for mainnet
  --out-file local/payment.addr
```

Fund the address from the [preprod faucet](https://docs.cardano.org/cardano-testnets/tools/faucet/).
You need at least **50 ADA** for the full deployment (bootstrap + publish-scripts + topic creation).

### 2. Split the funding UTxO

Bootstrap needs two separate UTxOs — one to parameterize topic-registry and one for
node-registry. If you only have one UTxO, split it first:

```sh
pubsub-admin split-utxo \
  --utxo <txhash>#<index> \
  --payment-addr local/payment.addr \
  --payment-skey local/payment.skey
```

The command prints the two resulting UTxO refs:
```
--topic-utxo <txhash>#0
--node-utxo  <txhash>#1
```

Wait ~20 seconds for the split tx to be confirmed before proceeding.

### 3. Bootstrap the on-chain registries

This deploys the topic-registry and node-registry contracts. Each consumes one UTxO as
its "one-shot" parameter, so the minting policy can only ever mint once.

```sh
pubsub-admin bootstrap \
  --topic-utxo <txhash>#0 \
  --node-utxo  <txhash>#1 \
  --payment-addr local/payment.addr \
  --payment-skey local/payment.skey \
  --contracts-dir contracts \
  --output-dir local
```

On success this writes `local/.env.preprod` (or `.env.preview` / `.env.mainnet`)
containing all contract addresses, policy IDs, and the bootstrap UTxO refs.

The output looks like:
```
======================================================================
Bootstrap complete — preprod
======================================================================
  topic-registry tx:  8d4cf88feb8f9a6a0b11a136508b69caed5d70e5aaecff88952024c74cc2c824
  node-registry  tx:  87c3911e27e8969d84f361096889b7257f556da9b16904bc88eac97bc2b7cc5d

  Config: local/.env.preprod
======================================================================
```

Wait for both transactions to be confirmed (≈20 s on preprod).

### 4. Publish reference scripts

Stores the four compiled Plutus scripts as UTxOs on-chain (CIP-33). This is done once;
all subsequent node-registration and topic-creation transactions reference these UTxOs
instead of embedding the full script bytes, saving ~1–3 ADA per transaction.

You need a UTxO with at least **40 ADA**. The actual minimum per script is computed
dynamically from `coins_per_utxo_byte × script_size` (Conway era); large scripts like
the registry-mint validator (~2800 bytes) require ~13 ADA alone. Use any UTxO at your
payment address.

```sh
pubsub-admin publish-scripts \
  --env-file local/.env.preprod \
  --payment-addr local/payment.addr \
  --payment-skey local/payment.skey \
  --contracts-dir contracts \
  --funding-utxo <txhash>#<index>
```

On success the command appends four variables to `local/.env.preprod`:

```
PUBSUB_REGISTRY_MINT_SCRIPT_REF=<txhash>#0
PUBSUB_TOPIC_VALIDATOR_SCRIPT_REF=<txhash>#0
PUBSUB_PUBLISHER_VAULT_SCRIPT_REF=<txhash>#0
PUBSUB_NODE_REGISTRY_SCRIPT_REF=<txhash>#0
```

### 5. Create a topic

Register the first topic on-chain. The transaction signer becomes the topic owner.

```sh
pubsub-admin create-topic \
  --env-file local/.env.preprod \
  --payment-addr local/payment.addr \
  --payment-skey local/payment.skey \
  --funding-utxo <txhash>#<index> \
  --name "iog/spo/alerts" \
  --replication-factor 3 \
  --retention-period 86400
```

### 6. Copy env file to the node

```sh
cp local/.env.preprod node/.env
```

The node reads this file at startup to discover contract addresses, policy IDs, and
script reference UTxOs.

---

## Subcommand reference

| Subcommand | Purpose |
|---|---|
| `split-utxo` | Split one UTxO into two (when you have a single large UTxO) |
| `bootstrap` | Deploy topic-registry + node-registry on-chain (one-time per network) |
| `publish-scripts` | Publish reference script UTxOs (run once after bootstrap) |
| `create-topic` | Register a new topic on-chain via the topic-registry contract |

All subcommands prompt interactively for any argument not supplied as a flag.
The `--blockfrost-project-id` flag falls back to the `BLOCKFROST_PROJECT_ID` env var.

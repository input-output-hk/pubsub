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

### 2. Bootstrap the topic-registry

Consumes a single UTxO as a one-shot parameter to produce a unique minting policy,
then mints the registry-head NFT. Any UTxO at your payment address works.

```sh
pubsub-admin bootstrap \
  --utxo <txhash>#<index> \
  --payment-addr local/payment.addr \
  --payment-skey local/payment.skey \
  --contracts-dir contracts \
  --output-dir local
```

On success this writes `local/.env.preprod` (or `.env.preview` / `.env.mainnet`)
containing all contract addresses, policy IDs, and the bootstrap UTxO ref.

```
======================================================================
Bootstrap complete — preprod
======================================================================
  topic-registry tx: <txhash>

  Config: local/.env.preprod
  Next steps: pubsub-admin publish-scripts --env-file local/.env.preprod
======================================================================
```

Wait for the transaction to confirm (~20 s on preprod) before proceeding.

### 3. Publish reference scripts

Stores the three compiled Plutus scripts as UTxOs on-chain (CIP-33). Done once;
all subsequent topic-creation transactions reference these UTxOs instead of
embedding the full script bytes, saving ~1–3 ADA per transaction.

Min-ADA per script is computed dynamically from `coins_per_utxo_byte × script_size`
(Conway era). The registry-mint script (~2800 bytes) alone needs ~13 ADA, so use a
UTxO with **at least 40 ADA**.

```sh
pubsub-admin publish-scripts \
  --env-file local/.env.preprod \
  --payment-addr local/payment.addr \
  --payment-skey local/payment.skey \
  --contracts-dir contracts \
  --funding-utxo <txhash>#<index>
```

On success the command appends three variables to `local/.env.preprod`:

```
PUBSUB_REGISTRY_MINT_SCRIPT_REF=<txhash>#0
PUBSUB_TOPIC_VALIDATOR_SCRIPT_REF=<txhash>#0
PUBSUB_PUBLISHER_VAULT_SCRIPT_REF=<txhash>#0
```

### 4. Create a topic

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

### 5. Copy env file to the node

```sh
cp local/.env.preprod node/.env
```

The node reads this file at startup to discover contract addresses, policy IDs, and
script reference UTxOs.

---

## Subcommand reference

| Subcommand | Purpose |
|---|---|
| `bootstrap` | Deploy topic-registry on-chain (one-time per network) |
| `publish-scripts` | Publish reference script UTxOs (run once after bootstrap) |
| `create-topic` | Register a new topic on-chain via the topic-registry contract |

All subcommands prompt interactively for any argument not supplied as a flag.
The `--blockfrost-project-id` flag falls back to the `BLOCKFROST_PROJECT_ID` env var.

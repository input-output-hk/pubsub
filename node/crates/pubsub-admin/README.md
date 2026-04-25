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

On success this writes `local/config.preprod.toml` (or `config.preview.toml` / `config.mainnet.toml`)
containing all contract addresses, policy IDs, and the bootstrap UTxO ref.

```
======================================================================
Bootstrap complete — preprod
======================================================================
  topic-registry tx: <txhash>

  Config: local/config.preprod.toml

Next steps:
  pubsub-admin publish-scripts --config local/config.preprod.toml
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
  --config local/config.preprod.toml \
  --payment-addr local/payment.addr \
  --payment-skey local/payment.skey \
  --contracts-dir contracts \
  --funding-utxo <txhash>#<index>
```

On success the command appends three keys to `local/config.preprod.toml`:

```toml
registry_mint_script_ref   = "<txhash>#0"
topic_validator_script_ref = "<txhash>#0"
publisher_vault_script_ref = "<txhash>#0"
```

### 4. Create a topic

Register the first topic on-chain. The transaction signer becomes the topic owner.

```sh
pubsub-admin create-topic \
  --config local/config.preprod.toml \
  --payment-addr local/payment.addr \
  --payment-skey local/payment.skey \
  --funding-utxo <txhash>#<index> \
  --name "iog/spo/alerts" \
  --replication-factor 3 \
  --retention-period 86400
```

### 5. Start the node with the config file

Pass the generated config file to `pubsub-node` via `--config`. The node reads all
contract addresses, policy IDs, and chain backend credentials from the file. Per-instance
flags (`--bind`, `--name`, `--key-file`) are still set on the CLI.

```sh
pubsub-node \
  --bind 0.0.0.0:9000 \
  --name my-node \
  --config local/config.preprod.toml
```

For the preprod network the published on-chain addresses are in `node/config.preprod.toml`
(no API key). Copy it to `local/config.preprod.toml`, add your `blockfrost_key`, and start.

---

## Subcommand reference

| Subcommand | Purpose |
|---|---|
| `bootstrap` | Deploy topic-registry on-chain (one-time per network) |
| `publish-scripts` | Publish reference script UTxOs (run once after bootstrap) |
| `create-topic` | Register a new topic on-chain via the topic-registry contract |

All subcommands prompt interactively for any argument not supplied as a flag.
The `--blockfrost-project-id` flag falls back to the `BLOCKFROST_PROJECT_ID` env var.

---

## Config file reference

`bootstrap` writes `local/config.{network}.toml`; subsequent commands append to it.
The same file is passed to `pubsub-node --config` so no translation step is needed.

| Key | Set by | Description |
|---|---|---|
| `network` | `bootstrap` | Network name (`preprod`, `preview`, `mainnet`) |
| `blockfrost_url` | `bootstrap` | Blockfrost REST API base URL for the network |
| `blockfrost_key` | manual | Blockfrost project credential — not written by bootstrap; add manually or pass `--blockfrost-project-id` |
| `topic_validator_addr` | `bootstrap` | Address of the **topic state** validator. One UTxO per topic holds its `TopicDatum` (name, owners, replication factor, retention period). |
| `publisher_vault_addr` | `bootstrap` | Address of the **publisher vault** validator. One UTxO per publisher-topic pair holds a minimum-ADA deposit. Minting a publisher token locks funds here; burning it releases them. |
| `node_registry_addr` | `bootstrap` | Address of the **node registry** validator (future use; reserved for replication servers). |
| `registry_policy_id` | `bootstrap` | Minting policy ID shared by the registry-head NFT, per-topic NFTs, and per-publisher tokens. |
| `registry_mint_script_ref` | `publish-scripts` | UTxO holding the registry minting policy script on-chain (CIP-33). Referenced instead of inlined to save ~13 ADA per topic-creation tx. |
| `topic_validator_script_ref` | `publish-scripts` | UTxO holding the topic validator script on-chain (CIP-33). |
| `publisher_vault_script_ref` | `publish-scripts` | UTxO holding the publisher vault script on-chain (CIP-33). |

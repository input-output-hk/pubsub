# Topic Registry — On-Chain Contracts

## Overview

Aiken (Plutus V3) contracts that manage the lifecycle of PubSub topics on Cardano. Three validators — registry, topic, publisher — coordinate via NFT-based state tokens minted under a single policy. The registry assigns globally unique topic IDs, tracks each topic's owners and admins, and authorises publishers per topic.

It does **not** store messages, register relay nodes, or handle payments. Those concerns live elsewhere in the system (or are out of scope for the current branch).

## Motivation

### Why on-chain?

Topics need to be globally discoverable and tamper-proof. Putting the registry on Cardano means any relay node, light client, or auditor can read the source of truth without trusting a server. Topic creation, ownership changes, and publisher authorisation are public, ordered, and signed.

### Why a registry-head NFT with a counter?

To assign monotonically increasing topic IDs with no coordination. The bootstrap transaction mints a single `registry_head` NFT carrying a counter; spending that UTxO is what serialises topic creation, and the counter guarantees no two creators ever collide on an ID.

### Why per-publisher NFTs instead of an in-datum list?

A topic may have hundreds or thousands of authorised publishers. Storing them all in `TopicDatum` would hit Plutus execution-budget and transaction-size limits, and every publisher change would deserialise and re-serialise the full list — O(N) per change. Per-publisher mint/burn is O(1) regardless of N. The topic UTxO is **referenced** (not spent) during publisher management, so adding/removing a publisher never contends with topic-config edits.

## Lifecycle at a glance

Five operations, in the order an operator typically meets them.

**1. Bootstrap (one-time per network).** A designated UTxO is consumed and a `registry_head` NFT is minted. The NFT lives at the registry script address and starts with `counter = 0`. This step happens once; the bootstrap UTxO becomes the policy's parameter and cannot be replayed.

**2. Create topic.** Spends the registry head UTxO (incrementing the counter) and mints a topic NFT (`t` + 4-byte ID). The topic NFT lands at the topic validator address with a `TopicDatum` carrying its config and an initial owner — the transaction signer. The new registry head UTxO returns with `counter + 1`.

**3. Manage topic.** Owner / admin actions — add/remove owner, add/remove admin, set replication factor, set retention period, delete — all spend the topic NFT and return it with an updated datum. Authorisation is checked against the datum's owner/admin lists via transaction signatories. Deletion tombstones the topic (`alive = false`); the NFT is not burned.

**4. Add publisher.** Mints a publisher role token (`p` + 4-byte topic ID + 28-byte pubkey hash) and parks it in a publisher vault UTxO with a `PublisherVaultDatum`. The topic UTxO is a reference input — read for the owner/admin check, not spent. Requires owner or admin signature.

**5. Remove publisher.** Spends the publisher vault UTxO and burns the role token. Same authorisation: owner or admin signature, checked against the topic's reference input.

## Technical reference

### Validators

| Validator | Purpose |
|---|---|
| [`registry`](validators/registry.ak) | Minting policy (all token types) + registry head spend logic |
| [`topic`](validators/topic.ak) | Topic UTxO spend logic (parameterized by registry policy ID) |
| [`publisher`](validators/publisher.ak) | Publisher vault spend logic (parameterized by registry policy ID) |

Supporting modules: [`types`](lib/topic_registry/types.ak), [`auth`](lib/topic_registry/auth.ak), [`validation`](lib/topic_registry/validation.ak), [`utils`](lib/topic_registry/utils.ak).

### Token encoding

| Token | Format | Example |
|---|---|---|
| Registry head | `"registry_head"` (literal) | — |
| Topic | `0x74` + 4-byte big-endian topic ID | `t\x00\x00\x00\x05` for topic 5 |
| Publisher | `0x70` + 4-byte topic ID + 28-byte pubkey hash | `p\x00\x00\x00\x05<pkh>` |

### Datums

#### RegistryHeadDatum

| Field | Type | Description |
|---|---|---|
| `counter` | `Int` | Next topic ID to assign (monotonically increasing) |
| `epoch` | `Int` | Current epoch, advanced via `TickEpoch` |

#### TopicDatum

| Field | Type | Description |
|---|---|---|
| `topic_id` | `Int` | Unique identifier (assigned from counter at creation) |
| `name` | `ByteArray` | Human-readable topic name |
| `owners` | `List<ByteArray>` | Pubkey hashes with full control |
| `admins` | `List<ByteArray>` | Pubkey hashes with limited management rights |
| `replication_factor` | `Int` | Persistence replication target (must be > 0). See note below. |
| `retention_period` | `Int` | How long messages are retained (must be > 0). See note below. |
| `alive` | `Bool` | `False` after deletion (tombstoned) |
| `published_at_epoch` | `Int` | Epoch when the topic was created |

> [!NOTE]
> **`replication_factor` and `retention_period`.** The current contract requires both fields to be > 0, carried over from the Quint formal spec and the AUEB design which assumed persistence for all topics. For topics that only need ephemeral dissemination (no persistence), these fields may not be meaningful. This constraint may need revisiting to allow zero values.

#### PublisherVaultDatum

| Field | Type | Description |
|---|---|---|
| `topic_id` | `Int` | Which topic this publisher is authorized for |
| `publisher` | `ByteArray` | Publisher's pubkey hash |

### Redeemer actions

#### RegistryMintAction (minting policy)

| Action | What It Does | Authorization |
|---|---|---|
| `BootstrapRegistry` | Mint the `registry_head` NFT. Requires consuming the bootstrap UTxO. | Bootstrap UTxO must be spent |
| `MintTopic` | Mint a topic NFT. Registry head must be in inputs (enforces counter increment via spend). | Registry head in inputs |
| `MintPublisher { topic_id, publisher }` | Mint a publisher role token. Topic must be alive. | Owner or admin signature (checked via topic reference input) |
| `BurnPublisher { topic_id, publisher }` | Burn a publisher role token. | Unconditional (token holder can always burn) |

#### RegistryHeadAction (registry head spend)

| Action | What It Does | Authorization |
|---|---|---|
| `CreateTopic { name, replication_factor, retention_period }` | Increment counter, mint topic NFT, create topic UTxO with initial datum. Signer becomes first owner. | Transaction signer (becomes owner) |
| `TickEpoch` | Advance the epoch counter by 1. Counter stays the same. | None specified |

#### TopicAction (topic UTxO spend)

| Action | What It Does | Authorization |
|---|---|---|
| `DeleteTopic` | Set `alive = False`, zero out all fields (tombstone). | Owner |
| `AddOwner { new_owner }` | Insert a pubkey hash into the owners list. | Owner |
| `RemoveOwner { old_owner }` | Remove a pubkey hash from owners. Must keep at least 1 owner. | Owner |
| `AddAdmin { new_admin }` | Insert a pubkey hash into the admins list. | Owner |
| `RemoveAdmin { old_admin }` | Remove a pubkey hash from admins. | Owner |
| `SetReplicationFactor { r }` | Update replication factor (must be > 0). | Owner or admin |
| `SetRetentionPeriod { t }` | Update retention period (must be > 0). | Owner or admin |

#### PublisherVaultAction (publisher vault spend)

| Action | What It Does | Authorization |
|---|---|---|
| `RemovePublisher` | Spend the vault UTxO and burn the publisher token. Topic must be alive. | Owner or admin (checked via topic reference input) |

## Deployed contracts

Live deployments by network. Update `node/config.<network>.toml` to point a node at one of these.

| Field | Preprod | Preview | Mainnet |
|---|---|---|---|
| `registry_policy_id` | `2ef6c260a9c1b5fa5cc0591935a0b492d4a265a3e9bc2c464b7c6c58` | — | — |
| `topic_validator_addr` | `addr_test1wppxf6v0spdrjv94e54f8vd7u3k2peya8rzdg4pvwfkdzrs2ev79t` | — | — |
| `publisher_vault_addr` | `addr_test1wqz3anxc5jahqf6g6jjjdmsyyl5j5jq0sgkzwcyfelk7z5cv0pu9l` | — | — |
| `topic_registry_addr` (admin) | `addr_test1wqh0dsnq48qmt7jucpv3jddqkjfdfgn9505mctzxfd7xckqexqxfm` | — | — |
| `registry_mint_script_ref` (admin) | `3426814e13d25e24ca43b5af998102b99d8c51528e0acda261394847ed126928#0` | — | — |
| `topic_validator_script_ref` (admin) | `64f8142ef2501fffef93fd471a2ff0204dd3a1530bea46fb2b4566b883660ed3#0` | — | — |
| `publisher_vault_script_ref` (admin) | `940ec105c503efea382e7cf5a37e7747e2143ad8d80532b53f0420b5bcb0682e#0` | — | — |
| Bootstrap UTxO (admin) | `87c3911e27e8969d84f361096889b7257f556da9b16904bc88eac97bc2b7cc5d#1` | — | — |

### What each field is for

- **`registry_policy_id`** — minting policy hash for all three token kinds (registry head, topic, publisher). Derived from the bootstrap UTxO's TxOutRef so it is unique per deployment. Nodes use it to filter "is this token under the registry policy" when scanning UTxOs.
- **`topic_validator_addr`** — script address holding one Topic UTxO per registered topic. Each carries a `TopicDatum`. Nodes query this address to enumerate topics and read owners / admins / replication / retention.
- **`publisher_vault_addr`** — script address holding one Publisher Vault UTxO per `(topic, publisher)` pair. Each carries a `PublisherVaultDatum`. Nodes query this address (filtered by token name `0x70 + topic_id_be4 + pkh`) to enumerate authorised publishers for a topic.
- **`topic_registry_addr`** — script address holding the singleton registry-head UTxO with the topic counter. Spent only when creating a new topic. The node never reads it directly (topic discovery uses `topic_validator_addr`); kept in the file for `pubsub-admin create-topic`. Marked **admin** in the table.
- **`registry_mint_script_ref` / `topic_validator_script_ref` / `publisher_vault_script_ref`** — CIP-33 reference-script UTxOs. `pubsub-admin publish-scripts` parks each compiled validator at one UTxO so subsequent transactions reference them by `OutputRef` instead of inlining the validator bytes (cheaper, smaller tx). The node never reads these — only `pubsub-admin` does when building transactions. Marked **admin**.
- **Bootstrap UTxO** — the original UTxO consumed by `BootstrapRegistry` to parameterise `registry_policy_id`. Recorded for provenance only; cannot be reused.

`network = "preprod"` in the config tells `pubsub-cli` and the dashboard which bech32 prefix (`addr_test...` vs `addr...`) to expect when displaying NodeIds. It is independent of the contract addresses themselves.

## Building and testing

```sh
aiken build
aiken check
```

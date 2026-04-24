// =============================================================================
// CardanoChainState — on-chain state reader with pluggable backends
// =============================================================================
//
// Implements ChainState via one of three backends selected at construction time:
//
//  LocalNode  — Ouroboros Node-to-Client via Unix socket (pallas-network).
//               Run the pubsub node alongside a synced local cardano-node.
//               Lowest latency; no external API keys needed.
//
//  Blockfrost — Blockfrost HTTP API (hosted or self-hosted).
//               No local node required; useful for development and testnet.
//               Needs: reqwest (HTTP client), blockfrost-rs or direct API calls.
//               API docs: https://docs.blockfrost.io
//
//  Utxorpc    — utxorpc gRPC protocol (Demeter cloud or dolos self-hosted).
//               Same data, different transport; works with any utxorpc provider.
//               Needs: pallas-utxorpc + tonic (gRPC runtime).
//               Spec: https://utxorpc.org
//
// All three dispatch through the same ChainState trait so the rest of the
// stack sees one uniform interface.
//
// Current state: constructors + dispatch skeleton.  All method bodies are
// todo!() stubs with per-backend comments describing the exact call required.
//
// Datum mirror structs and TopicId encoding decisions are documented below.
// =============================================================================

#![cfg(feature = "cardano")]

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use bytes::Bytes;

use pubsub_types::error::PubSubError;
use pubsub_types::message::TopicId;
use pubsub_types::node::{NodeId, NodeInfo};
use pubsub_types::topic::TopicConfig;
use pubsub_types::traits::ChainState;

// ---------------------------------------------------------------------------
// Datum type mirrors
// ---------------------------------------------------------------------------

/// Mirrors the Aiken TopicDatum constructor on-chain.
///
/// PlutusData encoding (Constr 0):
///   field 0: topic_id             — PlutusData::Integer
///   field 1: name                 — PlutusData::Bytes
///   field 2: owners               — PlutusData::Array of ByteString
///   field 3: admins               — PlutusData::Array of ByteString
///   field 4: replication_factor   — PlutusData::Integer
///   field 5: retention_period     — PlutusData::Integer (seconds)
///   field 6: alive                — PlutusData::Constr(1,[]) = True
///   field 7: published_at_epoch   — PlutusData::Integer
///
/// TopicId encoding: on-chain stores the registry counter as a Plutus integer.
/// Rust TopicId is [u8;32].  Agreed encoding: big-endian u64 in bytes [0..8],
/// remaining bytes zeroed.
/// TODO: implement fn on_chain_int_to_topic_id(n: u64) -> TopicId
#[allow(dead_code)]
struct TopicDatum {
    topic_id: u64,
    name: Vec<u8>,
    owners: Vec<Vec<u8>>,
    admins: Vec<Vec<u8>>,
    replication_factor: u64,
    retention_period: u64,
    alive: bool,
    published_at_epoch: u64,
}

/// Mirrors the Aiken NodeRegistryDatum on-chain.
///
/// PlutusData encoding (Constr 0):
///   field 0: nodes                 — PlutusData::Array of NodeEntry constructors
///   field 1: min_deposit_lovelace  — PlutusData::Integer
///   field 2: epoch                 — PlutusData::Integer
#[allow(dead_code)]
struct NodeRegistryDatum {
    nodes: Vec<NodeEntryDatum>,
    min_deposit_lovelace: u64,
    epoch: u64,
}

/// Mirrors NodeEntry in the node-registry contract.
///
/// PlutusData encoding (Constr 0):
///   field 0: node_id              — PlutusData::Bytes (32 bytes, blake2b-256 of addr)
///   field 1: addr                 — PlutusData::Bytes (UTF-8 "host:port")
///   field 2: stake_key            — PlutusData::Bytes (payment key hash)
///   field 3: registered_at_epoch  — PlutusData::Integer
#[allow(dead_code)]
struct NodeEntryDatum {
    node_id: Vec<u8>,
    addr: Vec<u8>,
    stake_key: Vec<u8>,
    registered_at_epoch: u64,
}

// ---------------------------------------------------------------------------
// ChainProvider — selects the backend at construction time
// ---------------------------------------------------------------------------

/// Backend used to query Cardano chain state.
pub enum ChainProvider {
    /// Ouroboros Node-to-Client via a local Unix socket.
    ///
    /// Requires a synced `cardano-node` running on the same machine.
    /// Uses pallas-network LocalStateQuery mini-protocol to fetch UTxOs
    /// at script addresses and stake distribution.
    ///
    /// Network magic values:
    ///   mainnet  = 764_824_073
    ///   preprod  = 1
    ///   preview  = 2
    LocalNode {
        socket_path: PathBuf,
        magic: u64,
    },

    /// Blockfrost HTTP API.
    ///
    /// No local node required.  Suitable for testnet and development.
    /// Implementation needs: `reqwest` (async HTTP), direct REST calls
    /// to /addresses/{addr}/utxos and /accounts/<stake_addr>/rewards.
    ///
    /// Base URLs:
    ///   mainnet  = "https://cardano-mainnet.blockfrost.io/api/v0"
    ///   preprod  = "https://cardano-preprod.blockfrost.io/api/v0"
    ///   preview  = "https://cardano-preview.blockfrost.io/api/v0"
    Blockfrost {
        project_id: String,
        base_url: String,
    },

    /// utxorpc gRPC endpoint (Demeter cloud, self-hosted dolos, or other).
    ///
    /// Implementation needs: `pallas-utxorpc` + `tonic` gRPC runtime.
    /// Relevant RPC: QueryService.SearchUtxos (filter by address or asset).
    /// Spec: https://utxorpc.org
    Utxorpc {
        endpoint: String,
        api_key: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// CardanoChainState
// ---------------------------------------------------------------------------

/// Reads Cardano L1 state using the configured backend.
///
/// # Construction
/// ```no_run
/// use pubsub_network::pallas_chain::CardanoChainState;
///
/// // Local cardano-node on preview testnet
/// let local = CardanoChainState::local_node("/tmp/node.socket", 2);
///
/// // Blockfrost on preprod
/// let bf = CardanoChainState::blockfrost(
///     "preprodXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
///     "https://cardano-preprod.blockfrost.io/api/v0",
/// );
///
/// // Demeter utxorpc
/// let rpc = CardanoChainState::utxorpc(
///     "https://preview.utxorpc-v0.demeter.run",
///     Some("dmtr_apikey...".into()),
/// );
/// ```
pub struct CardanoChainState {
    provider: ChainProvider,
}

impl CardanoChainState {
    /// Connect via a local cardano-node Unix socket (Ouroboros NtC).
    pub fn local_node(socket_path: impl AsRef<Path>, magic: u64) -> Self {
        Self {
            provider: ChainProvider::LocalNode {
                socket_path: socket_path.as_ref().to_path_buf(),
                magic,
            },
        }
    }

    /// Query via Blockfrost HTTP API.
    pub fn blockfrost(project_id: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            provider: ChainProvider::Blockfrost {
                project_id: project_id.into(),
                base_url: base_url.into(),
            },
        }
    }

    /// Query via utxorpc gRPC (Demeter or self-hosted dolos).
    pub fn utxorpc(endpoint: impl Into<String>, api_key: Option<String>) -> Self {
        Self {
            provider: ChainProvider::Utxorpc {
                endpoint: endpoint.into(),
                api_key,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// ChainState implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl ChainState for CardanoChainState {
    async fn get_registered_nodes(&self) -> Result<Vec<NodeInfo>, PubSubError> {
        match &self.provider {
            ChainProvider::LocalNode { socket_path, magic } => {
                // LocalStateQuery → UTxOsByAddress(node_registry_addr)
                // Decode inline datum as NodeRegistryDatum (minicbor::Decode impl needed).
                // Convert each NodeEntryDatum.addr bytes (UTF-8 "host:port") → SocketAddr.
                let _ = (socket_path, magic);
                todo!("LocalNode: LocalStateQuery UTxOsByAddress(node_registry_addr) → NodeRegistryDatum")
            }
            ChainProvider::Blockfrost { project_id, base_url } => {
                // GET {base_url}/addresses/<node_registry_addr>/utxos
                // Filter UTxOs with an inline datum; decode as NodeRegistryDatum.
                // Blockfrost returns inline datums as hex-encoded CBOR under "inline_datum".
                let _ = (project_id, base_url);
                todo!("Blockfrost: GET /addresses/<node_registry_addr>/utxos → parse inline_datum")
            }
            ChainProvider::Utxorpc { endpoint, api_key } => {
                // QueryService.SearchUtxos { match: AddressPattern(node_registry_addr) }
                // Each AnyUtxo carries the inline datum as CBOR bytes; decode as NodeRegistryDatum.
                let _ = (endpoint, api_key);
                todo!("utxorpc: SearchUtxos(node_registry_addr_str) → decode inline datum")
            }
        }
    }

    async fn get_topic_config(
        &self,
        topic: &TopicId,
    ) -> Result<Option<TopicConfig>, PubSubError> {
        match &self.provider {
            ChainProvider::LocalNode { socket_path, magic } => {
                // LocalStateQuery → UTxOsByAddress(topic_registry_datum_addr)
                // For each UTxO with TopicDatum inline datum:
                //   - Decode TopicDatum (minicbor::Decode)
                //   - Convert datum.topic_id (u64) → TopicId via on_chain_int_to_topic_id()
                //   - Match against requested topic
                // Then: UTxOsByAddress(publisher_vault_addr) filtered by topic policy token
                //   - Decode PublisherVaultDatum → authorized_publishers list
                // Build TopicConfig::try_new(...)
                let _ = (topic, socket_path, magic);
                todo!("LocalNode: UTxOsByAddress(topic_datum_addr) + vault UTxO enumeration")
            }
            ChainProvider::Blockfrost { project_id, base_url } => {
                // GET {base_url}/addresses/<topic_datum_addr>/utxos
                // Find UTxO whose decoded TopicDatum.topic_id matches; decode fields.
                // GET {base_url}/addresses/<vault_addr>/utxos?asset={topic_policy_token}
                // Collect PublisherVaultDatum entries → authorized_publishers.
                let _ = (topic, project_id, base_url);
                todo!("Blockfrost: GET /addresses/<topic_datum_addr>/utxos + vault UTxOs")
            }
            ChainProvider::Utxorpc { endpoint, api_key } => {
                // SearchUtxos(topic_datum_addr_str) — scan for matching TopicDatum.
                // SearchUtxos(vault_addr, asset=topic_policy_token) — collect publishers.
                let _ = (topic, endpoint, api_key);
                todo!("utxorpc: SearchUtxos(topic_datum_addr_str) + SearchUtxos(vault_addr, asset)")
            }
        }
    }

    async fn get_all_topics(&self) -> Result<Vec<TopicConfig>, PubSubError> {
        match &self.provider {
            ChainProvider::LocalNode { socket_path, magic } => {
                // LocalStateQuery → UTxOsByAddress(topic_datum_addr)
                // Decode all TopicDatum UTxOs → Vec<TopicConfig>.
                let _ = (socket_path, magic);
                todo!("LocalNode: UTxOsByAddress(topic_datum_addr) → decode all TopicDatum UTxOs")
            }
            ChainProvider::Blockfrost { project_id, base_url } => {
                // GET {base_url}/addresses/<topic_datum_addr>/utxos (paginated)
                // Decode every inline datum as TopicDatum → Vec<TopicConfig>.
                let _ = (project_id, base_url);
                todo!("Blockfrost: GET /addresses/<topic_datum_addr>/utxos (paginated)")
            }
            ChainProvider::Utxorpc { endpoint, api_key } => {
                // SearchUtxos(topic_datum_addr_str) — stream all; decode each inline datum.
                let _ = (endpoint, api_key);
                todo!("utxorpc: SearchUtxos(topic_datum_addr_str) — stream and decode all")
            }
        }
    }

    async fn get_node_stake(&self, node: &NodeId) -> Result<u64, PubSubError> {
        match &self.provider {
            ChainProvider::LocalNode { socket_path, magic } => {
                // LocalStateQuery → QueryLedgerState::StakeDistribution
                // Requires mapping NodeId → stake credential (from node registry lookup).
                let _ = (node, socket_path, magic);
                todo!("LocalNode: QueryLedgerState::StakeDistribution → match node stake credential")
            }
            ChainProvider::Blockfrost { project_id, base_url } => {
                // GET {base_url}/accounts/<stake_addr>/rewards — fetch total rewards/stake.
                // Requires NodeId → stake_addr mapping from node registry.
                let _ = (node, project_id, base_url);
                todo!("Blockfrost: GET /accounts/<stake_addr>/rewards after NodeId → stake_addr lookup")
            }
            ChainProvider::Utxorpc { endpoint, api_key } => {
                // No direct stake distribution RPC in utxorpc v0 spec.
                // Fall back to deriving from node registry UTxO stake key field.
                let _ = (node, endpoint, api_key);
                todo!("utxorpc: derive stake from NodeRegistryDatum.stake_key (no direct StakeDistribution RPC)")
            }
        }
    }

    async fn get_pool_kes_keys(&self) -> Result<Vec<Bytes>, PubSubError> {
        match &self.provider {
            ChainProvider::LocalNode { socket_path, magic } => {
                // LocalStateQuery → QueryLedgerState::PoolState(all_pools)
                // Extract current KES vkeys from each pool's operational certificate.
                let _ = (socket_path, magic);
                todo!("LocalNode: QueryLedgerState::PoolState → extract KES vkeys from opcerts")
            }
            ChainProvider::Blockfrost { project_id, base_url } => {
                // GET {base_url}/pools (paginated) → for each pool_id:
                // GET {base_url}/pools/<pool_id> → "vrf_key" (note: this is VRF not KES).
                // KES key is in the opcert; Blockfrost does not expose opcerts directly.
                // Alternative: GET {base_url}/pools/<pool_id>/metadata — may carry opcert.
                let _ = (project_id, base_url);
                todo!("Blockfrost: /pools/<pool_id> — KES keys not directly exposed; design decision needed")
            }
            ChainProvider::Utxorpc { endpoint, api_key } => {
                // No direct pool KES RPC in utxorpc v0.  Same limitation as Blockfrost.
                let _ = (endpoint, api_key);
                todo!("utxorpc: KES keys not in utxorpc v0 spec — design decision needed")
            }
        }
    }

    async fn get_drep_keys(&self) -> Result<Vec<Bytes>, PubSubError> {
        match &self.provider {
            ChainProvider::LocalNode { socket_path, magic } => {
                // LocalStateQuery → QueryLedgerState::DRepState (Conway era, node ≥ 9.x)
                // Extracts registered DRep verification keys from CIP-1694 registrations.
                let _ = (socket_path, magic);
                todo!("LocalNode: QueryLedgerState::DRepState (Conway) → registered DRep vkeys")
            }
            ChainProvider::Blockfrost { project_id, base_url } => {
                // GET {base_url}/governance/dreps (paginated, Conway era)
                // Returns DRep IDs; GET /governance/dreps/<drep_id> for vkey.
                let _ = (project_id, base_url);
                todo!("Blockfrost: GET /governance/dreps (Conway era) → DRep vkeys")
            }
            ChainProvider::Utxorpc { endpoint, api_key } => {
                // GovernanceService (utxorpc v0 Conway extension) — DRepState query.
                // Spec under active development; verify endpoint availability.
                let _ = (endpoint, api_key);
                todo!("utxorpc: GovernanceService.DRepState (Conway extension) → DRep vkeys")
            }
        }
    }

    async fn get_authority_keys(&self) -> Result<Vec<Bytes>, PubSubError> {
        // Authority key list is not a ledger query in any backend.
        // Options:
        //   (a) Hardcoded in node config (simplest for Phase 1)
        //   (b) Stored in a known UTxO at a fixed address — readable by all three backends
        //   (c) Multi-sig governance contract (future)
        // Phase 2: read from a "authority-keys" UTxO at a known address; all three
        // backends can fetch it via /addresses/{authority_addr}/utxos.
        todo!("authority keys: design decision — config file vs on-chain UTxO (all backends)")
    }
}

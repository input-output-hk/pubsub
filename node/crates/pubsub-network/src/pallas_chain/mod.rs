// =============================================================================
// CardanoChainState — on-chain state reader with pluggable backends
// =============================================================================
//
// Four backends, one ChainState trait impl. Each backend lives in its own
// submodule and exposes `get_*` functions that this `mod.rs` dispatches to.
//
//   blockfrost — Blockfrost HTTP REST API (fully implemented).
//   ogmios     — Ogmios v6+ JSON-RPC over HTTP POST (fully implemented).
//   LocalNode  — pallas-network Ouroboros NtC via Unix socket (stubs).
//   Utxorpc    — utxorpc gRPC / Demeter (stubs).
//
// TopicId encoding
// ────────────────
// On-chain: Plutus Int (registry counter).  Rust: [u8;32].
// Conversion: big-endian u64 in bytes 0..8, remaining bytes zero.
// =============================================================================

#![cfg(feature = "cardano")]

mod blockfrost;
mod datum;
mod ogmios;

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use bytes::Bytes;

use pubsub_types::error::PubSubError;
use pubsub_types::message::TopicId;
use pubsub_types::node::{NodeId, NodeInfo};
use pubsub_types::topic::TopicConfig;
use pubsub_types::traits::ChainState;

// ---------------------------------------------------------------------------
// ContractAddresses — deployment-specific script addresses
// ---------------------------------------------------------------------------

/// Addresses and policy ID of the deployed PubSub Cardano contracts.
///
/// These are network- and deployment-specific.  Derive them from the compiled
/// `plutus.json` validator hashes after the bootstrap transaction.
///
/// Create from environment variables in your binary:
/// ```no_run
/// use pubsub_network::pallas_chain::ContractAddresses;
///
/// let contracts = ContractAddresses {
///     node_registry_addr: std::env::var("PUBSUB_NODE_REGISTRY_ADDR").unwrap_or_default(),
///     topic_validator_addr: std::env::var("PUBSUB_TOPIC_VALIDATOR_ADDR").unwrap(),
///     publisher_vault_addr: std::env::var("PUBSUB_PUBLISHER_VAULT_ADDR").unwrap(),
///     registry_policy_id: std::env::var("PUBSUB_REGISTRY_POLICY_ID").unwrap(),
/// };
/// ```
#[derive(Clone)]
pub struct ContractAddresses {
    /// Bech32 address of the node registry validator (Phase 2; may be empty string).
    pub node_registry_addr: String,
    /// Bech32 address of the topic validator — where per-topic TopicDatum UTxOs live.
    /// This is PUBSUB_TOPIC_VALIDATOR_ADDR, NOT the registry head address.
    pub topic_validator_addr: String,
    /// Bech32 address of the publisher vault validator.
    pub publisher_vault_addr: String,
    /// Hex policy ID of the registry minting policy (56 hex chars = 28 bytes).
    pub registry_policy_id: String,
}

// ---------------------------------------------------------------------------
// ChainProvider
// ---------------------------------------------------------------------------

/// Backend used to query Cardano chain state.
pub enum ChainProvider {
    /// Ouroboros Node-to-Client via a local cardano-node Unix socket.
    ///
    /// Network magic:  mainnet = 764_824_073 | preprod = 1 | preview = 2
    LocalNode { socket_path: PathBuf, magic: u64 },

    /// Blockfrost HTTP REST API.
    ///
    /// Base URLs:
    ///   mainnet  — "https://cardano-mainnet.blockfrost.io/api/v0"
    ///   preprod  — "https://cardano-preprod.blockfrost.io/api/v0"
    ///   preview  — "https://cardano-preview.blockfrost.io/api/v0"
    Blockfrost { project_id: String, base_url: String },

    /// Ogmios v6+ JSON-RPC over HTTP POST.
    ///
    /// URL examples:
    ///   local    — "http://localhost:1337"
    ///   cloud    — "https://ogmios.preprod.some-provider.io"
    ///
    /// No API key required. Requires Ogmios v6.0+ (HTTP POST support).
    Ogmios { url: String },

    /// utxorpc gRPC endpoint (Demeter cloud or self-hosted dolos).
    ///
    /// Implementation needs: pallas-utxorpc + tonic.
    Utxorpc { endpoint: String, api_key: Option<String> },
}

// ---------------------------------------------------------------------------
// CardanoChainState
// ---------------------------------------------------------------------------

/// Reads Cardano L1 state using the configured backend.
///
/// # Construction
/// ```no_run
/// use pubsub_network::pallas_chain::{CardanoChainState, ContractAddresses};
///
/// let contracts = ContractAddresses {
///     node_registry_addr: std::env::var("PUBSUB_NODE_REGISTRY_ADDR").unwrap_or_default(),
///     topic_validator_addr: std::env::var("PUBSUB_TOPIC_VALIDATOR_ADDR").unwrap(),
///     publisher_vault_addr: std::env::var("PUBSUB_PUBLISHER_VAULT_ADDR").unwrap(),
///     registry_policy_id: std::env::var("PUBSUB_REGISTRY_POLICY_ID").unwrap(),
/// };
///
/// // Local cardano-node (preview testnet)
/// let local = CardanoChainState::local_node("/tmp/node.socket", 2, contracts.clone());
///
/// // Blockfrost (preprod)
/// let bf = CardanoChainState::blockfrost(
///     std::env::var("BLOCKFROST_PROJECT_ID").unwrap(),
///     "https://cardano-preprod.blockfrost.io/api/v0",
///     contracts.clone(),
/// );
///
/// // Ogmios (local or cloud, no API key)
/// let og = CardanoChainState::ogmios("http://localhost:1337", contracts.clone());
///
/// // Demeter utxorpc
/// let rpc = CardanoChainState::utxorpc(
///     "https://preview.utxorpc-v0.demeter.run",
///     Some(std::env::var("DEMETER_API_KEY").unwrap()),
///     contracts,
/// );
/// ```
pub struct CardanoChainState {
    provider: ChainProvider,
    contracts: ContractAddresses,
}

impl CardanoChainState {
    pub fn local_node(
        socket_path: impl AsRef<Path>,
        magic: u64,
        contracts: ContractAddresses,
    ) -> Self {
        Self {
            provider: ChainProvider::LocalNode {
                socket_path: socket_path.as_ref().to_path_buf(),
                magic,
            },
            contracts,
        }
    }

    pub fn blockfrost(
        project_id: impl Into<String>,
        base_url: impl Into<String>,
        contracts: ContractAddresses,
    ) -> Self {
        Self {
            provider: ChainProvider::Blockfrost {
                project_id: project_id.into(),
                base_url: base_url.into(),
            },
            contracts,
        }
    }

    pub fn ogmios(url: impl Into<String>, contracts: ContractAddresses) -> Self {
        Self {
            provider: ChainProvider::Ogmios { url: url.into() },
            contracts,
        }
    }

    pub fn utxorpc(
        endpoint: impl Into<String>,
        api_key: Option<String>,
        contracts: ContractAddresses,
    ) -> Self {
        Self {
            provider: ChainProvider::Utxorpc {
                endpoint: endpoint.into(),
                api_key,
            },
            contracts,
        }
    }
}

#[async_trait]
impl ChainState for CardanoChainState {
    async fn get_registered_nodes(&self) -> Result<Vec<NodeInfo>, PubSubError> {
        match &self.provider {
            ChainProvider::Blockfrost { project_id, base_url } => {
                blockfrost::get_registered_nodes(project_id, base_url, &self.contracts).await
            }
            ChainProvider::Ogmios { url } => {
                ogmios::get_registered_nodes(url, &self.contracts).await
            }
            ChainProvider::LocalNode { socket_path, magic } => {
                let _ = (socket_path, magic);
                todo!("LocalNode: UTxOsByAddress(node_registry_addr) via NtC LocalStateQuery")
            }
            ChainProvider::Utxorpc { endpoint, api_key } => {
                let _ = (endpoint, api_key);
                todo!("utxorpc: SearchUtxos(node_registry_addr) — pallas-utxorpc + tonic needed")
            }
        }
    }

    async fn get_topic_config(
        &self,
        topic: &TopicId,
    ) -> Result<Option<TopicConfig>, PubSubError> {
        match &self.provider {
            ChainProvider::Blockfrost { project_id, base_url } => {
                blockfrost::get_topic_config(project_id, base_url, &self.contracts, topic).await
            }
            ChainProvider::Ogmios { url } => {
                ogmios::get_topic_config(url, &self.contracts, topic).await
            }
            ChainProvider::LocalNode { socket_path, magic } => {
                let _ = (topic, socket_path, magic);
                todo!("LocalNode: UTxOsByAddress(topic_validator_addr) + vault UTxO scan via NtC")
            }
            ChainProvider::Utxorpc { endpoint, api_key } => {
                let _ = (topic, endpoint, api_key);
                todo!("utxorpc: SearchUtxos(topic_validator_addr) + SearchUtxos(vault_addr)")
            }
        }
    }

    async fn get_all_topics(&self) -> Result<Vec<TopicConfig>, PubSubError> {
        match &self.provider {
            ChainProvider::Blockfrost { project_id, base_url } => {
                blockfrost::get_all_topics(project_id, base_url, &self.contracts).await
            }
            ChainProvider::Ogmios { url } => {
                ogmios::get_all_topics(url, &self.contracts).await
            }
            ChainProvider::LocalNode { socket_path, magic } => {
                let _ = (socket_path, magic);
                todo!("LocalNode: UTxOsByAddress(topic_validator_addr) decode all TopicDatum UTxOs")
            }
            ChainProvider::Utxorpc { endpoint, api_key } => {
                let _ = (endpoint, api_key);
                todo!("utxorpc: SearchUtxos(topic_validator_addr) stream and decode all")
            }
        }
    }

    async fn get_node_stake(&self, node: &NodeId) -> Result<u64, PubSubError> {
        match &self.provider {
            ChainProvider::Blockfrost { .. } => {
                // Requires pallas-addresses to encode the 28-byte stake key hash
                // (from NodeRegistryDatum.stake_key) into a bech32 stake address
                // before calling GET /accounts/{stake_addr}.
                let _ = node;
                Err(PubSubError::ChainState(
                    "get_node_stake via Blockfrost: needs pallas-addresses for bech32 stake addr encoding".into(),
                ))
            }
            ChainProvider::Ogmios { url } => {
                let _ = (node, url);
                Err(PubSubError::ChainState(
                    "get_node_stake via Ogmios: use queryLedgerState/rewardAccountSummaries — not yet implemented".into(),
                ))
            }
            ChainProvider::LocalNode { socket_path, magic } => {
                let _ = (node, socket_path, magic);
                todo!("LocalNode: QueryLedgerState::StakeDistribution via NtC")
            }
            ChainProvider::Utxorpc { endpoint, api_key } => {
                let _ = (node, endpoint, api_key);
                todo!("utxorpc: no direct StakeDistribution RPC in v0; derive from registry stake_key")
            }
        }
    }

    async fn get_pool_kes_keys(&self) -> Result<Vec<Bytes>, PubSubError> {
        match &self.provider {
            ChainProvider::Blockfrost { .. } => {
                // Blockfrost does not expose KES keys or operational certificates
                // in its REST API.  The /pools/{pool_id} endpoint returns VRF keys
                // and metadata, not the opcert KES vkey.
                Err(PubSubError::ChainState(
                    "get_pool_kes_keys: KES operational certificates are not exposed by the Blockfrost REST API; use the LocalNode backend for this query".into(),
                ))
            }
            ChainProvider::Ogmios { url } => {
                let _ = url;
                Err(PubSubError::ChainState(
                    "get_pool_kes_keys via Ogmios: use queryLedgerState/poolParameters — not yet implemented".into(),
                ))
            }
            ChainProvider::LocalNode { socket_path, magic } => {
                let _ = (socket_path, magic);
                todo!("LocalNode: QueryLedgerState::PoolState — extract KES vkeys from opcerts")
            }
            ChainProvider::Utxorpc { endpoint, api_key } => {
                let _ = (endpoint, api_key);
                todo!("utxorpc: KES keys not in utxorpc v0 spec")
            }
        }
    }

    async fn get_drep_keys(&self) -> Result<Vec<Bytes>, PubSubError> {
        match &self.provider {
            ChainProvider::Blockfrost { project_id, base_url } => {
                blockfrost::get_drep_keys(project_id, base_url).await
            }
            ChainProvider::Ogmios { url } => {
                let _ = url;
                // Ogmios v6 supports queryLedgerState/delegateRepresentatives but
                // the drep ID is bech32 — requires a bech32 decoder to extract the
                // raw 28-byte key hash.  Stub until pallas-addresses is added.
                Err(PubSubError::ChainState(
                    "get_drep_keys via Ogmios: queryLedgerState/delegateRepresentatives — needs bech32 drep ID decoder; not yet implemented".into(),
                ))
            }
            ChainProvider::LocalNode { socket_path, magic } => {
                let _ = (socket_path, magic);
                todo!("LocalNode: QueryLedgerState::DRepState (Conway, node >= 9.x)")
            }
            ChainProvider::Utxorpc { endpoint, api_key } => {
                let _ = (endpoint, api_key);
                todo!("utxorpc: GovernanceService.DRepState (Conway extension)")
            }
        }
    }

    async fn get_authority_keys(&self) -> Result<Vec<Bytes>, PubSubError> {
        // Authority key list is not a ledger query in any backend.
        // Phase 1: hardcode in node config.
        // Phase 2: read from a known UTxO at a fixed address (readable by all backends).
        Err(PubSubError::ChainState(
            "get_authority_keys: not a chain query — supply via node config".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Integration tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::datum::{
        decode_plutus_data, decode_topic_datum, on_chain_int_to_topic_id, topic_id_to_on_chain_int,
    };
    use super::*;
    use pallas::ledger::primitives::{BigInt, Constr};

    // ── Datum decode unit tests ───────────────────────────────────────────────

    #[test]
    fn decode_topic_datum_from_cbor() {
        // Hand-crafted CBOR for TopicDatum:
        //   Constr 0 [ 1, "news", [], [], 3, 3600, True, 5 ]
        // We build it programmatically using pallas codec to avoid byte-level errors.
        use pallas::codec::utils::{Int as PallasInt, MaybeIndefArray};
        use pallas::ledger::primitives::{BoundedBytes, PlutusData};

        let fields: Vec<PlutusData> = vec![
            PlutusData::BigInt(BigInt::Int(PallasInt::from(1i64))),
            PlutusData::BoundedBytes(BoundedBytes::from(b"news".to_vec())),
            PlutusData::Array(MaybeIndefArray::Def(vec![])),
            PlutusData::Array(MaybeIndefArray::Def(vec![])),
            PlutusData::BigInt(BigInt::Int(PallasInt::from(3i64))),
            PlutusData::BigInt(BigInt::Int(PallasInt::from(3600i64))),
            // True = Constr 1 []
            PlutusData::Constr(Constr {
                tag: 122,
                any_constructor: None,
                fields: MaybeIndefArray::Def(vec![]),
            }),
            PlutusData::BigInt(BigInt::Int(PallasInt::from(5i64))),
        ];
        let datum = PlutusData::Constr(Constr {
            tag: 121,
            any_constructor: None,
            fields: MaybeIndefArray::Def(fields),
        });

        let cbor = pallas::codec::minicbor::to_vec(&datum).expect("encode");
        let hex_str = hex::encode(&cbor);
        let decoded = decode_plutus_data(&hex_str).expect("decode hex CBOR");
        let td = decode_topic_datum(&decoded).expect("decode TopicDatum");

        assert_eq!(td.topic_id, 1);
        assert_eq!(String::from_utf8(td.name).unwrap(), "news");
        assert_eq!(td.replication_factor, 3);
        assert_eq!(td.retention_period, 3600);
        assert!(td.alive);
    }

    #[test]
    fn on_chain_int_topic_id_roundtrip() {
        let n = 42u64;
        let id = on_chain_int_to_topic_id(n);
        assert_eq!(topic_id_to_on_chain_int(&id), Some(n));
    }

    #[test]
    fn hash_topic_id_not_convertible() {
        // A Blake2b-derived TopicId has non-zero bytes beyond position 8.
        use pallas_crypto::hash::Hasher;
        let hash = Hasher::<256>::hash(b"some-topic");
        let mut id = [0u8; 32];
        id.copy_from_slice(hash.as_ref());
        let topic_id = TopicId(id);
        assert_eq!(topic_id_to_on_chain_int(&topic_id), None);
    }

    // ── Blockfrost integration tests (skipped without API key) ───────────────
    //
    // Set these env vars to run:
    //   BLOCKFROST_PROJECT_ID=preprod...
    //   BLOCKFROST_BASE_URL=https://cardano-preprod.blockfrost.io/api/v0   (optional)

    fn blockfrost_env() -> Option<(String, String)> {
        let project_id = std::env::var("BLOCKFROST_PROJECT_ID").ok()?;
        let base_url = std::env::var("BLOCKFROST_BASE_URL")
            .unwrap_or_else(|_| "https://cardano-preprod.blockfrost.io/api/v0".into());
        Some((project_id, base_url))
    }

    fn contract_env() -> Option<ContractAddresses> {
        Some(ContractAddresses {
            node_registry_addr: std::env::var("PUBSUB_NODE_REGISTRY_ADDR").ok()?,
            topic_validator_addr: std::env::var("PUBSUB_TOPIC_VALIDATOR_ADDR").ok()?,
            publisher_vault_addr: std::env::var("PUBSUB_PUBLISHER_VAULT_ADDR").ok()?,
            registry_policy_id: std::env::var("PUBSUB_REGISTRY_POLICY_ID").ok()?,
        })
    }

    #[tokio::test]
    async fn blockfrost_get_drep_keys_preprod() {
        let Some((project_id, base_url)) = blockfrost_env() else {
            eprintln!("skip: BLOCKFROST_PROJECT_ID not set");
            return;
        };
        let contracts = contract_env().unwrap_or(ContractAddresses {
            node_registry_addr: String::new(),
            topic_validator_addr: String::new(),
            publisher_vault_addr: String::new(),
            registry_policy_id: String::new(),
        });
        let chain = CardanoChainState::blockfrost(project_id, base_url, contracts);
        let dreps = chain.get_drep_keys().await.expect("get_drep_keys");
        eprintln!("preprod DRep key count: {}", dreps.len());
        for k in &dreps {
            assert!(
                k.len() == 28 || k.len() == 32,
                "unexpected key length: {}",
                k.len()
            );
        }
    }

    #[tokio::test]
    async fn blockfrost_get_all_topics_preprod() {
        let Some((project_id, base_url)) = blockfrost_env() else {
            eprintln!("skip: BLOCKFROST_PROJECT_ID not set");
            return;
        };
        let Some(contracts) = contract_env() else {
            eprintln!("skip: PUBSUB_TOPIC_REGISTRY_ADDR not set");
            return;
        };
        let chain = CardanoChainState::blockfrost(project_id, base_url, contracts);
        let topics = chain.get_all_topics().await.expect("get_all_topics");
        eprintln!("preprod topic count: {}", topics.len());
    }

    #[tokio::test]
    async fn blockfrost_get_registered_nodes_preprod() {
        let Some((project_id, base_url)) = blockfrost_env() else {
            eprintln!("skip: BLOCKFROST_PROJECT_ID not set");
            return;
        };
        let Some(contracts) = contract_env() else {
            eprintln!("skip: PUBSUB_NODE_REGISTRY_ADDR not set");
            return;
        };
        let chain = CardanoChainState::blockfrost(project_id, base_url, contracts);
        let nodes = chain
            .get_registered_nodes()
            .await
            .expect("get_registered_nodes");
        eprintln!("preprod registered nodes: {}", nodes.len());
    }
}

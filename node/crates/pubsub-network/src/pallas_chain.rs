// =============================================================================
// PallasChainState — on-chain state reader backed by a local cardano-node
// =============================================================================
//
// Implements the ChainState trait by connecting to a local cardano-node via
// its Unix socket using the Ouroboros Node-to-Client mini-protocols.
//
// Stack (all from the pallas crate, v0.34):
//   pallas::network          — multiplexer + bearer (Unix socket)
//   pallas::network::miniprotocols::localstatequeryspecific
//                            — LocalStateQuery for UTxO set queries
//   pallas::traverse         — typed access to inline datums in tx outputs
//   pallas::codec (minicbor) — CBOR decode for PlutusData constructors
//
// Current state: constructor + stub implementations.
// All methods return todo!() with a comment describing the exact
// LocalStateQuery call or datum decoding step required.
//
// Key design decisions that must be resolved before filling in the stubs:
//
// 1. TopicId encoding: the on-chain TopicDatum stores topic_id as a Plutus
//    integer (the registry counter).  The Rust TopicId is a [u8;32].
//    Agreed encoding: big-endian u64 in the first 8 bytes, rest zeroed.
//    Implement: fn on_chain_int_to_topic_id(n: u64) -> TopicId { ... }
//
// 2. authorized_publishers: the contract uses per-topic publisher vault UTxOs
//    (PublisherVaultDatum { topic_id: Int, publisher: ByteArray }).
//    Reading the authorized list requires enumerating UTxOs at the vault
//    validator address that hold the topic's policy token.
//    The ChainState::get_topic_config call should therefore return a
//    TopicConfig with authorized_publishers populated from vault UTxO scan.
//
// 3. Socket path and network magic must be supplied at construction time.
//    Preview: magic = 2.  Pre-prod: magic = 1.  Mainnet: magic = 764824073.
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
///   field 0: topic_id          — PlutusData::Integer
///   field 1: name              — PlutusData::Bytes
///   field 2: owners            — PlutusData::Array of ByteString
///   field 3: admins            — PlutusData::Array of ByteString
///   field 4: replication_factor — PlutusData::Integer
///   field 5: retention_period  — PlutusData::Integer (seconds)
///   field 6: alive             — PlutusData::Constr(1,[]) for True, Constr(0,[]) for False
///   field 7: published_at_epoch — PlutusData::Integer
///
/// Decoding: use pallas::codec (minicbor) with a custom Decode impl.
/// TODO: implement minicbor::Decode for TopicDatum matching this schema.
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
///   field 0: nodes             — PlutusData::Array of NodeEntry constructors
///   field 1: min_deposit_lovelace — PlutusData::Integer
///   field 2: epoch             — PlutusData::Integer
///
/// TODO: implement minicbor::Decode for NodeRegistryDatum.
#[allow(dead_code)]
struct NodeRegistryDatum {
    nodes: Vec<NodeEntryDatum>,
    min_deposit_lovelace: u64,
    epoch: u64,
}

/// Mirrors NodeEntry in the node-registry contract.
///
/// PlutusData encoding (Constr 0):
///   field 0: node_id           — PlutusData::Bytes (32 bytes, blake2b-256 of addr)
///   field 1: addr              — PlutusData::Bytes (UTF-8 "host:port")
///   field 2: stake_key         — PlutusData::Bytes (payment key hash)
///   field 3: registered_at_epoch — PlutusData::Integer
#[allow(dead_code)]
struct NodeEntryDatum {
    node_id: Vec<u8>,
    addr: Vec<u8>,
    stake_key: Vec<u8>,
    registered_at_epoch: u64,
}

// ---------------------------------------------------------------------------
// PallasChainState
// ---------------------------------------------------------------------------

/// Reads Cardano L1 state via a local cardano-node Unix socket.
///
/// # Construction
/// ```no_run
/// use pubsub_network::pallas_chain::PallasChainState;
/// let chain = PallasChainState::new("/tmp/node.socket", 2); // preview network
/// ```
pub struct PallasChainState {
    socket_path: PathBuf,
    /// Cardano network magic (preview = 2, preprod = 1, mainnet = 764824073).
    magic: u64,
}

impl PallasChainState {
    pub fn new(socket_path: impl AsRef<Path>, magic: u64) -> Self {
        Self {
            socket_path: socket_path.as_ref().to_path_buf(),
            magic,
        }
    }
}

#[async_trait]
impl ChainState for PallasChainState {
    async fn get_registered_nodes(&self) -> Result<Vec<NodeInfo>, PubSubError> {
        // TODO: LocalStateQuery → QueryLedgerState::UTxOsByAddress(node_registry_addr)
        //       Decode inline datum as NodeRegistryDatum.
        //       Convert each NodeEntryDatum → NodeInfo (addr bytes → SocketAddr::from_str).
        todo!("get_registered_nodes via LocalStateQuery UTxOsByAddress(node_registry_addr)")
    }

    async fn get_topic_config(
        &self,
        topic: &TopicId,
    ) -> Result<Option<TopicConfig>, PubSubError> {
        // TODO: LocalStateQuery → UTxOsByAddress(topic_registry_head_addr) + UTxOsByAddress(topic_datum_addr)
        //       For each UTxO with a TopicDatum inline datum:
        //         - Decode TopicDatum (minicbor::Decode)
        //         - Convert datum.topic_id (u64) → TopicId via on_chain_int_to_topic_id()
        //         - Match against requested topic
        //       Then: UTxOsByAddress(publisher_vault_addr) filtered by topic policy token
        //         - Decode PublisherVaultDatum → authorized_publishers list
        //       Build TopicConfig::try_new(..., authorized_publishers, ...)
        let _ = (topic, &self.socket_path, self.magic);
        todo!("get_topic_config via LocalStateQuery + TopicDatum decode + vault UTxO enumeration")
    }

    async fn get_all_topics(&self) -> Result<Vec<TopicConfig>, PubSubError> {
        // TODO: Same as get_topic_config but for all TopicDatum UTxOs.
        //       UTxOsByAddress(topic_datum_addr) → decode all → build Vec<TopicConfig>.
        todo!("get_all_topics via LocalStateQuery UTxOsByAddress(topic_datum_addr)")
    }

    async fn get_node_stake(&self, node: &NodeId) -> Result<u64, PubSubError> {
        // TODO: LocalStateQuery → QueryLedgerState::StakeDistribution
        //       Match the node's stake credential against the distribution map.
        //       Requires mapping NodeId → stake_credential (from node registry lookup).
        let _ = node;
        todo!("get_node_stake via LocalStateQuery StakeDistribution")
    }

    async fn get_pool_kes_keys(&self) -> Result<Vec<Bytes>, PubSubError> {
        // TODO: LocalStateQuery → QueryLedgerState::PoolState(all_pools)
        //       Extract current KES vkeys from each pool's operational certificate.
        //       Real KES chain verification (opcert counter, KES period) is separate.
        todo!("get_pool_kes_keys via LocalStateQuery PoolState")
    }

    async fn get_drep_keys(&self) -> Result<Vec<Bytes>, PubSubError> {
        // TODO: LocalStateQuery → QueryLedgerState::DRepState (Conway era)
        //       Extract registered DRep verification keys from CIP-1694 DRep registration UTxOs.
        //       Only available in Conway era (node version ≥ 9.x).
        todo!("get_drep_keys via LocalStateQuery DRepState (Conway)")
    }

    async fn get_authority_keys(&self) -> Result<Vec<Bytes>, PubSubError> {
        // TODO: The authority key list is governed off-chain (not a ledger query).
        //       Options: (a) hardcoded in config, (b) stored in a dedicated UTxO at a
        //       known address, (c) multi-sig governance contract.
        //       Phase 2: read from a simple "authority-keys" UTxO at a known address.
        todo!("get_authority_keys — design decision: config file vs on-chain UTxO")
    }
}

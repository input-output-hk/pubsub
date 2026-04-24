// =============================================================================
// CardanoChainState — on-chain state reader with pluggable backends
// =============================================================================
//
// Four backends, one ChainState trait impl:
//
//   LocalNode  — pallas-network Ouroboros NtC via Unix socket (stubs).
//   Blockfrost — Blockfrost HTTP REST API (fully implemented).
//   Ogmios     — Ogmios v6+ JSON-RPC over HTTP POST (fully implemented).
//   Utxorpc    — utxorpc gRPC / Demeter (stubs).
//
// Ogmios implementation
// ─────────────────────
// Uses the HTTP POST interface (Ogmios v6.0+) at the configured URL.
// Standard JSON-RPC 2.0: POST / with body {jsonrpc,method,params,id}.
// Inline datums returned as lowercase hex CBOR; same decoder as Blockfrost.
// Token presence checked via value.policies[policyId][assetNameHex] structure.
//
// No API key required — ideal for permissionless relay node operators.
//
// Blockfrost implementation
// ─────────────────────────
// Pagination: all list endpoints are walked page-by-page (100 items/page).
// Auth: `project_id` request header.
// Inline datums: returned as lowercase hex CBOR; decoded via pallas-primitives
// PlutusData (minicbor).
//
// Datum layout mirrors (Aiken → Rust)
// ────────────────────────────────────
// All datums use CBOR tag 121 (Constr 0) at the top level.
//
//   TopicDatum          — fields 0-7: topic_id(Int), name(Bytes), owners([Bytes]),
//                          admins([Bytes]), replication_factor(Int), retention_period(Int),
//                          alive(Constr 1[]=True | Constr 0[]=False), published_at_epoch(Int)
//
//   NodeRegistryDatum   — fields 0-2: nodes([NodeEntry]), min_deposit(Int), epoch(Int)
//     NodeEntry         — fields 0-3: node_id(Bytes), addr(Bytes), stake_key(Bytes), epoch(Int)
//
//   PublisherVaultDatum — fields 0-1: topic_id(Int), publisher(Bytes)
//
// TopicId encoding
// ────────────────
// On-chain: Plutus Int (registry counter).  Rust: [u8;32].
// Conversion: big-endian u64 in bytes 0..8, remaining bytes zero.
// =============================================================================

#![cfg(feature = "cardano")]

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use serde::Deserialize;

use pallas::ledger::primitives::{BigInt, Constr, PlutusData};

use pubsub_types::error::PubSubError;
use pubsub_types::message::{PublisherCredential, PublisherId, TopicId};
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
///     node_registry_addr: std::env::var("PUBSUB_NODE_REGISTRY_ADDR").unwrap(),
///     topic_registry_addr: std::env::var("PUBSUB_TOPIC_REGISTRY_ADDR").unwrap(),
///     publisher_vault_addr: std::env::var("PUBSUB_PUBLISHER_VAULT_ADDR").unwrap(),
///     registry_policy_id: std::env::var("PUBSUB_REGISTRY_POLICY_ID").unwrap(),
/// };
/// ```
#[derive(Clone)]
pub struct ContractAddresses {
    /// Bech32 address of the node registry validator.
    pub node_registry_addr: String,
    /// Bech32 address of the per-topic datum validator.
    pub topic_registry_addr: String,
    /// Bech32 address of the publisher vault validator.
    pub publisher_vault_addr: String,
    /// Hex policy ID of the registry minting policy (56 hex chars = 28 bytes).
    pub registry_policy_id: String,
}

// ---------------------------------------------------------------------------
// Datum mirrors (Aiken on-chain types → Rust)
// ---------------------------------------------------------------------------

struct TopicDatum {
    topic_id: u64,
    name: Vec<u8>,
    // owners and admins (fields 2-3) are parsed but not stored — TopicConfig
    // uses the publisher vault UTxOs for authorization, not these lists.
    replication_factor: u64,
    retention_period: u64,
    alive: bool,
    // published_at_epoch (field 7) is on-chain bookkeeping; not used off-chain.
}

struct NodeRegistryDatum {
    nodes: Vec<NodeEntryDatum>,
    #[allow(dead_code)]
    min_deposit_lovelace: u64,
    #[allow(dead_code)]
    epoch: u64,
}

struct NodeEntryDatum {
    node_id: Vec<u8>,
    addr: Vec<u8>,
    stake_key: Vec<u8>,
    #[allow(dead_code)]
    registered_at_epoch: u64,
}

struct PublisherVaultDatum {
    topic_id: u64,
    publisher: Vec<u8>,
}

// ---------------------------------------------------------------------------
// PlutusData decode helpers
// ---------------------------------------------------------------------------

fn decode_plutus_data(hex_str: &str) -> Result<PlutusData, PubSubError> {
    let bytes =
        hex::decode(hex_str).map_err(|e| PubSubError::Codec(format!("hex decode: {e}")))?;
    pallas::codec::minicbor::decode(&bytes)
        .map_err(|e| PubSubError::Codec(format!("CBOR decode: {e}")))
}

fn constr0_fields(data: &PlutusData) -> Option<&[PlutusData]> {
    match data {
        PlutusData::Constr(Constr { tag, .. }) if *tag == 121 => {
            // tag 121 = Constr 0 (index 0)
            match data {
                PlutusData::Constr(c) => Some(&c.fields),
                _ => unreachable!(),
            }
        }
        PlutusData::Constr(c) if c.constr_index() == 0 => Some(&c.fields),
        _ => None,
    }
}

fn bigint_u64(data: &PlutusData) -> Option<u64> {
    match data {
        PlutusData::BigInt(BigInt::Int(i)) => {
            let v: i128 = i128::from(*i);
            u64::try_from(v).ok()
        }
        _ => None,
    }
}

fn pdata_bytes(data: &PlutusData) -> Option<&[u8]> {
    match data {
        PlutusData::BoundedBytes(b) => Some(b),
        _ => None,
    }
}

fn pdata_array(data: &PlutusData) -> Option<&[PlutusData]> {
    match data {
        PlutusData::Array(a) => Some(a),
        _ => None,
    }
}

fn decode_topic_datum(data: &PlutusData) -> Option<TopicDatum> {
    let f = constr0_fields(data)?;
    if f.len() < 8 {
        return None;
    }
    let topic_id = bigint_u64(&f[0])?;
    let name = pdata_bytes(&f[1])?.to_vec();
    // fields 2 (owners) and 3 (admins) — parse past them for positional alignment
    pdata_array(&f[2])?;
    pdata_array(&f[3])?;
    let replication_factor = bigint_u64(&f[4])?;
    let retention_period = bigint_u64(&f[5])?;
    let alive = match &f[6] {
        PlutusData::Constr(c) => c.constr_index() == 1, // Constr 1 [] = True
        _ => return None,
    };
    // field 7 (published_at_epoch) — validate type but don't store
    bigint_u64(&f[7])?;
    Some(TopicDatum { topic_id, name, replication_factor, retention_period, alive })
}

fn decode_node_registry_datum(data: &PlutusData) -> Option<NodeRegistryDatum> {
    let f = constr0_fields(data)?;
    if f.len() < 3 {
        return None;
    }
    let nodes = pdata_array(&f[0])?
        .iter()
        .filter_map(|e| {
            let nf = constr0_fields(e)?;
            if nf.len() < 4 {
                return None;
            }
            Some(NodeEntryDatum {
                node_id: pdata_bytes(&nf[0])?.to_vec(),
                addr: pdata_bytes(&nf[1])?.to_vec(),
                stake_key: pdata_bytes(&nf[2])?.to_vec(),
                registered_at_epoch: bigint_u64(&nf[3])?,
            })
        })
        .collect();
    Some(NodeRegistryDatum {
        nodes,
        min_deposit_lovelace: bigint_u64(&f[1])?,
        epoch: bigint_u64(&f[2])?,
    })
}

fn decode_publisher_vault_datum(data: &PlutusData) -> Option<PublisherVaultDatum> {
    let f = constr0_fields(data)?;
    if f.len() < 2 {
        return None;
    }
    Some(PublisherVaultDatum {
        topic_id: bigint_u64(&f[0])?,
        publisher: pdata_bytes(&f[1])?.to_vec(),
    })
}

// ---------------------------------------------------------------------------
// TopicId / on-chain int conversion
// ---------------------------------------------------------------------------

/// On-chain topic IDs are incrementing integers.
/// Rust TopicId is [u8;32]: big-endian u64 in the first 8 bytes, rest zero.
fn on_chain_int_to_topic_id(n: u64) -> TopicId {
    let mut id = [0u8; 32];
    id[..8].copy_from_slice(&n.to_be_bytes());
    TopicId(id)
}

/// Returns None if the TopicId was not created by `on_chain_int_to_topic_id`
/// (i.e. bytes 8..32 are non-zero, meaning it's a hash-derived ID).
fn topic_id_to_on_chain_int(id: &TopicId) -> Option<u64> {
    if id.0[8..].iter().any(|&b| b != 0) {
        return None;
    }
    Some(u64::from_be_bytes(id.0[..8].try_into().unwrap()))
}

// ---------------------------------------------------------------------------
// Blockfrost JSON response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct BfUtxo {
    inline_datum: Option<String>,
    amount: Vec<BfAmount>,
}

#[derive(Deserialize)]
struct BfAmount {
    unit: String,
    #[allow(dead_code)]
    quantity: String,
}

#[derive(Deserialize)]
struct BfDrep {
    /// Raw verification key hash as lowercase hex (not bech32 drep ID).
    hex: String,
    /// If true this is a script credential, not a key — skip it.
    has_script: bool,
}


// ---------------------------------------------------------------------------
// BlockfrostClient
// ---------------------------------------------------------------------------

struct BlockfrostClient {
    project_id: String,
    base_url: String,
    client: reqwest::Client,
}

impl BlockfrostClient {
    fn new(project_id: &str, base_url: &str) -> Self {
        Self {
            project_id: project_id.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Fetch one page of JSON from a Blockfrost endpoint.
    /// Returns `None` when the endpoint returns 404 (address/resource not found).
    async fn get_page<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        page: u32,
    ) -> Result<Option<Vec<T>>, PubSubError> {
        let url = format!("{}/{}?page={}&count=100", self.base_url, path, page);
        let resp = self
            .client
            .get(&url)
            .header("project_id", &self.project_id)
            .send()
            .await
            .map_err(|e| PubSubError::ChainState(format!("Blockfrost HTTP: {e}")))?;

        if resp.status().as_u16() == 404 {
            return Ok(None);
        }
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(PubSubError::ChainState(format!(
                "Blockfrost {status}: {body}"
            )));
        }
        let items: Vec<T> = resp
            .json()
            .await
            .map_err(|e| PubSubError::ChainState(format!("Blockfrost JSON: {e}")))?;
        Ok(Some(items))
    }

    /// Walk all pages of a list endpoint, returning all items.
    async fn get_all<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
    ) -> Result<Vec<T>, PubSubError> {
        let mut all = Vec::new();
        let mut page = 1u32;
        loop {
            match self.get_page::<T>(path, page).await? {
                None => break, // 404 — address exists but has no results
                Some(items) if items.is_empty() => break,
                Some(items) => {
                    let got = items.len();
                    all.extend(items);
                    if got < 100 {
                        break;
                    }
                    page += 1;
                }
            }
        }
        Ok(all)
    }

    async fn get_utxos_at(&self, addr: &str) -> Result<Vec<BfUtxo>, PubSubError> {
        self.get_all(&format!("addresses/{}/utxos", addr)).await
    }

}

// ---------------------------------------------------------------------------
// Ogmios JSON types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct OgmiosUtxo {
    /// Inline datum as lower-case hex CBOR (same encoding as Blockfrost).
    datum: Option<String>,
    value: serde_json::Value,
}

// ---------------------------------------------------------------------------
// OgmiosClient
// ---------------------------------------------------------------------------

struct OgmiosClient {
    url: String,
    client: reqwest::Client,
}

impl OgmiosClient {
    fn new(url: &str) -> Self {
        Self {
            url: url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// POST a JSON-RPC 2.0 request and deserialise the `result` field.
    async fn query<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T, PubSubError> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": "1",
        });
        let resp = self
            .client
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .map_err(|e| PubSubError::ChainState(format!("Ogmios HTTP: {e}")))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(PubSubError::ChainState(format!("Ogmios {status}: {text}")));
        }
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| PubSubError::ChainState(format!("Ogmios JSON: {e}")))?;
        if let Some(err) = json.get("error") {
            return Err(PubSubError::ChainState(format!("Ogmios error: {err}")));
        }
        let result = json
            .get("result")
            .cloned()
            .ok_or_else(|| PubSubError::ChainState("Ogmios: missing 'result' field".into()))?;
        serde_json::from_value(result)
            .map_err(|e| PubSubError::ChainState(format!("Ogmios result decode: {e}")))
    }

    async fn query_utxos(&self, address: &str) -> Result<Vec<OgmiosUtxo>, PubSubError> {
        self.query(
            "queryLedgerState/utxo",
            serde_json::json!({ "addresses": [address] }),
        )
        .await
    }
}

/// Returns true if the Ogmios UTxO holds any token under `policy_id`
/// whose hex asset name starts with `asset_name_prefix`.
fn ogmios_has_token(utxo: &OgmiosUtxo, policy_id: &str, asset_name_prefix: &str) -> bool {
    utxo.value
        .get("policies")
        .and_then(|p| p.get(policy_id))
        .and_then(|a| a.as_object())
        .map(|obj| obj.keys().any(|k| k.starts_with(asset_name_prefix)))
        .unwrap_or(false)
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
///     node_registry_addr: std::env::var("PUBSUB_NODE_REGISTRY_ADDR").unwrap(),
///     topic_registry_addr: std::env::var("PUBSUB_TOPIC_REGISTRY_ADDR").unwrap(),
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
    pub fn local_node(socket_path: impl AsRef<Path>, magic: u64, contracts: ContractAddresses) -> Self {
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

// ---------------------------------------------------------------------------
// ChainState implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl ChainState for CardanoChainState {
    async fn get_registered_nodes(&self) -> Result<Vec<NodeInfo>, PubSubError> {
        match &self.provider {
            ChainProvider::Blockfrost { project_id, base_url } => {
                let bf = BlockfrostClient::new(project_id, base_url);
                let utxos = bf.get_utxos_at(&self.contracts.node_registry_addr).await?;

                let mut nodes = Vec::new();
                for utxo in &utxos {
                    let Some(hex) = &utxo.inline_datum else {
                        continue;
                    };
                    let data = decode_plutus_data(hex)?;
                    let Some(registry) = decode_node_registry_datum(&data) else {
                        continue;
                    };
                    for entry in registry.nodes {
                        let addr_str = String::from_utf8_lossy(&entry.addr).to_string();
                        let addr: SocketAddr = addr_str.parse().map_err(|e| {
                            PubSubError::ChainState(format!("node addr parse: {e}"))
                        })?;
                        let mut node_id = [0u8; 32];
                        let len = entry.node_id.len().min(32);
                        node_id[..len].copy_from_slice(&entry.node_id[..len]);
                        nodes.push(NodeInfo {
                            node_id: NodeId(node_id),
                            addr,
                            public_key: entry.stake_key,
                            subscribed_topics: vec![],
                        });
                    }
                }
                Ok(nodes)
            }
            ChainProvider::Ogmios { url } => {
                let og = OgmiosClient::new(url);
                let utxos = og.query_utxos(&self.contracts.node_registry_addr).await?;
                let mut nodes = Vec::new();
                for utxo in &utxos {
                    let Some(hex) = &utxo.datum else { continue };
                    let data = decode_plutus_data(hex)?;
                    let Some(registry) = decode_node_registry_datum(&data) else { continue };
                    for entry in registry.nodes {
                        let addr_str = String::from_utf8_lossy(&entry.addr).to_string();
                        let addr: SocketAddr = addr_str.parse().map_err(|e| {
                            PubSubError::ChainState(format!("node addr parse: {e}"))
                        })?;
                        let mut node_id = [0u8; 32];
                        let len = entry.node_id.len().min(32);
                        node_id[..len].copy_from_slice(&entry.node_id[..len]);
                        nodes.push(NodeInfo {
                            node_id: NodeId(node_id),
                            addr,
                            public_key: entry.stake_key,
                            subscribed_topics: vec![],
                        });
                    }
                }
                Ok(nodes)
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
                let target_n = topic_id_to_on_chain_int(topic).ok_or_else(|| {
                    PubSubError::ChainState(
                        "TopicId bytes 8-31 are non-zero; not a registry-originated topic".into(),
                    )
                })?;

                let bf = BlockfrostClient::new(project_id, base_url);
                let utxos = bf.get_utxos_at(&self.contracts.topic_registry_addr).await?;

                for utxo in &utxos {
                    let Some(hex) = &utxo.inline_datum else {
                        continue;
                    };
                    let data = decode_plutus_data(hex)?;
                    let Some(td) = decode_topic_datum(&data) else {
                        continue;
                    };
                    if td.topic_id != target_n || !td.alive {
                        continue;
                    }

                    // Fetch publisher vault UTxOs for this topic.
                    // Publisher token name: [4-byte-big-endian-topic-id][28-byte-pkh][0x70]
                    // Filter on policy_id + 4-byte topic_id; pkh and suffix follow.
                    let topic_hex_prefix = format!(
                        "{}{:08x}",
                        self.contracts.registry_policy_id, td.topic_id
                    );
                    let vault_utxos =
                        bf.get_utxos_at(&self.contracts.publisher_vault_addr).await?;
                    let mut authorized_publishers = Vec::new();
                    for vault in &vault_utxos {
                        let has_token = vault
                            .amount
                            .iter()
                            .any(|a| a.unit.starts_with(&topic_hex_prefix));
                        if !has_token {
                            continue;
                        }
                        let Some(vhex) = &vault.inline_datum else {
                            continue;
                        };
                        let vdata = decode_plutus_data(vhex)?;
                        if let Some(vd) = decode_publisher_vault_datum(&vdata) {
                            if vd.topic_id == td.topic_id {
                                let cred = PublisherCredential::ed25519(Bytes::from(vd.publisher));
                                authorized_publishers.push(PublisherId(cred));
                            }
                        }
                    }

                    let name = String::from_utf8_lossy(&td.name).to_string();
                    let config = TopicConfig::try_new(
                        topic.clone(),
                        name,
                        None,
                        authorized_publishers,
                        Duration::from_secs(td.retention_period),
                        td.replication_factor as u32,
                    )
                    .map_err(|e| PubSubError::ChainState(e.to_string()))?;
                    return Ok(Some(config));
                }
                Ok(None)
            }
            ChainProvider::Ogmios { url } => {
                let target_n = topic_id_to_on_chain_int(topic).ok_or_else(|| {
                    PubSubError::ChainState(
                        "TopicId bytes 8-31 are non-zero; not a registry-originated topic".into(),
                    )
                })?;
                let og = OgmiosClient::new(url);
                let utxos = og.query_utxos(&self.contracts.topic_registry_addr).await?;
                for utxo in &utxos {
                    let Some(hex) = &utxo.datum else { continue };
                    let data = decode_plutus_data(hex)?;
                    let Some(td) = decode_topic_datum(&data) else { continue };
                    if td.topic_id != target_n || !td.alive {
                        continue;
                    }
                    let asset_name_prefix = format!("{:08x}", td.topic_id);
                    let vault_utxos =
                        og.query_utxos(&self.contracts.publisher_vault_addr).await?;
                    let mut authorized_publishers = Vec::new();
                    for vault in &vault_utxos {
                        if !ogmios_has_token(
                            vault,
                            &self.contracts.registry_policy_id,
                            &asset_name_prefix,
                        ) {
                            continue;
                        }
                        let Some(vhex) = &vault.datum else { continue };
                        let vdata = decode_plutus_data(vhex)?;
                        if let Some(vd) = decode_publisher_vault_datum(&vdata) {
                            if vd.topic_id == td.topic_id {
                                let cred =
                                    PublisherCredential::ed25519(Bytes::from(vd.publisher));
                                authorized_publishers.push(PublisherId(cred));
                            }
                        }
                    }
                    let name = String::from_utf8_lossy(&td.name).to_string();
                    let config = TopicConfig::try_new(
                        topic.clone(),
                        name,
                        None,
                        authorized_publishers,
                        Duration::from_secs(td.retention_period),
                        td.replication_factor as u32,
                    )
                    .map_err(|e| PubSubError::ChainState(e.to_string()))?;
                    return Ok(Some(config));
                }
                Ok(None)
            }
            ChainProvider::LocalNode { socket_path, magic } => {
                let _ = (topic, socket_path, magic);
                todo!("LocalNode: UTxOsByAddress(topic_registry_addr) + vault UTxO scan via NtC")
            }
            ChainProvider::Utxorpc { endpoint, api_key } => {
                let _ = (topic, endpoint, api_key);
                todo!("utxorpc: SearchUtxos(topic_registry_addr) + SearchUtxos(vault_addr)")
            }
        }
    }

    async fn get_all_topics(&self) -> Result<Vec<TopicConfig>, PubSubError> {
        match &self.provider {
            ChainProvider::Blockfrost { project_id, base_url } => {
                let bf = BlockfrostClient::new(project_id, base_url);
                let utxos = bf.get_utxos_at(&self.contracts.topic_registry_addr).await?;
                // Eagerly fetch all vault UTxOs once; filter per-topic below.
                let vault_utxos = bf
                    .get_utxos_at(&self.contracts.publisher_vault_addr)
                    .await?;

                let mut topics = Vec::new();
                for utxo in &utxos {
                    let Some(hex) = &utxo.inline_datum else {
                        continue;
                    };
                    let data = decode_plutus_data(hex)?;
                    let Some(td) = decode_topic_datum(&data) else {
                        continue;
                    };
                    if !td.alive {
                        continue;
                    }
                    let topic_id = on_chain_int_to_topic_id(td.topic_id);
                    // Publisher token name: [4-byte-big-endian-topic-id][28-byte-pkh][0x70]
                    // Filter on policy_id + 4-byte topic_id; pkh and suffix follow.
                    let topic_hex_prefix = format!(
                        "{}{:08x}",
                        self.contracts.registry_policy_id, td.topic_id
                    );
                    let mut authorized_publishers = Vec::new();
                    for vault in &vault_utxos {
                        let has_token = vault
                            .amount
                            .iter()
                            .any(|a| a.unit.starts_with(&topic_hex_prefix));
                        if !has_token {
                            continue;
                        }
                        let Some(vhex) = &vault.inline_datum else {
                            continue;
                        };
                        let vdata = decode_plutus_data(vhex)?;
                        if let Some(vd) = decode_publisher_vault_datum(&vdata) {
                            if vd.topic_id == td.topic_id {
                                let cred = PublisherCredential::ed25519(Bytes::from(vd.publisher));
                                authorized_publishers.push(PublisherId(cred));
                            }
                        }
                    }
                    let name = String::from_utf8_lossy(&td.name).to_string();
                    if let Ok(config) = TopicConfig::try_new(
                        topic_id,
                        name,
                        None,
                        authorized_publishers,
                        Duration::from_secs(td.retention_period),
                        td.replication_factor as u32,
                    ) {
                        topics.push(config);
                    }
                }
                Ok(topics)
            }
            ChainProvider::Ogmios { url } => {
                let og = OgmiosClient::new(url);
                let utxos = og.query_utxos(&self.contracts.topic_registry_addr).await?;
                let vault_utxos = og.query_utxos(&self.contracts.publisher_vault_addr).await?;
                let mut topics = Vec::new();
                for utxo in &utxos {
                    let Some(hex) = &utxo.datum else { continue };
                    let data = decode_plutus_data(hex)?;
                    let Some(td) = decode_topic_datum(&data) else { continue };
                    if !td.alive {
                        continue;
                    }
                    let topic_id = on_chain_int_to_topic_id(td.topic_id);
                    let asset_name_prefix = format!("{:08x}", td.topic_id);
                    let mut authorized_publishers = Vec::new();
                    for vault in &vault_utxos {
                        if !ogmios_has_token(
                            vault,
                            &self.contracts.registry_policy_id,
                            &asset_name_prefix,
                        ) {
                            continue;
                        }
                        let Some(vhex) = &vault.datum else { continue };
                        let vdata = decode_plutus_data(vhex)?;
                        if let Some(vd) = decode_publisher_vault_datum(&vdata) {
                            if vd.topic_id == td.topic_id {
                                let cred =
                                    PublisherCredential::ed25519(Bytes::from(vd.publisher));
                                authorized_publishers.push(PublisherId(cred));
                            }
                        }
                    }
                    let name = String::from_utf8_lossy(&td.name).to_string();
                    if let Ok(config) = TopicConfig::try_new(
                        topic_id,
                        name,
                        None,
                        authorized_publishers,
                        Duration::from_secs(td.retention_period),
                        td.replication_factor as u32,
                    ) {
                        topics.push(config);
                    }
                }
                Ok(topics)
            }
            ChainProvider::LocalNode { socket_path, magic } => {
                let _ = (socket_path, magic);
                todo!("LocalNode: UTxOsByAddress(topic_registry_addr) decode all TopicDatum UTxOs")
            }
            ChainProvider::Utxorpc { endpoint, api_key } => {
                let _ = (endpoint, api_key);
                todo!("utxorpc: SearchUtxos(topic_registry_addr) stream and decode all")
            }
        }
    }

    async fn get_node_stake(&self, node: &NodeId) -> Result<u64, PubSubError> {
        match &self.provider {
            ChainProvider::Blockfrost { .. } => {
                // Requires pallas-addresses to encode the 28-byte stake key hash
                // (from NodeRegistryDatum.stake_key) into a bech32 stake address
                // before calling GET /accounts/{stake_addr}.
                // TODO: add pallas-addresses dep and implement bech32 encoding.
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
                // Options: (a) use a local node for this query, (b) store KES keys
                // off-chain in a known config address, (c) skip KES-based publishing.
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
                let bf = BlockfrostClient::new(project_id, base_url);
                // GET /governance/dreps — Conway era; returns registered DReps.
                let dreps: Vec<BfDrep> = bf.get_all("governance/dreps").await?;
                let keys = dreps
                    .into_iter()
                    .filter(|d| !d.has_script)
                    .filter_map(|d| hex::decode(&d.hex).ok())
                    .map(Bytes::from)
                    .collect();
                Ok(keys)
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
    use super::*;

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
                tag: 122, // Constr 1
                any_constructor: None,
                fields: MaybeIndefArray::Def(vec![]),
            }),
            PlutusData::BigInt(BigInt::Int(PallasInt::from(5i64))),
        ];
        let datum = PlutusData::Constr(Constr {
            tag: 121, // Constr 0
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
    //
    // These tests hit real Blockfrost endpoints and require a preprod API key.

    fn blockfrost_env() -> Option<(String, String)> {
        let project_id = std::env::var("BLOCKFROST_PROJECT_ID").ok()?;
        let base_url = std::env::var("BLOCKFROST_BASE_URL").unwrap_or_else(|_| {
            "https://cardano-preprod.blockfrost.io/api/v0".into()
        });
        Some((project_id, base_url))
    }

    fn contract_env() -> Option<ContractAddresses> {
        Some(ContractAddresses {
            node_registry_addr: std::env::var("PUBSUB_NODE_REGISTRY_ADDR").ok()?,
            topic_registry_addr: std::env::var("PUBSUB_TOPIC_REGISTRY_ADDR").ok()?,
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
            topic_registry_addr: String::new(),
            publisher_vault_addr: String::new(),
            registry_policy_id: String::new(),
        });
        let chain = CardanoChainState::blockfrost(project_id, base_url, contracts);
        let dreps = chain.get_drep_keys().await.expect("get_drep_keys");
        eprintln!("preprod DRep key count: {}", dreps.len());
        // On preprod Conway era there are registered DReps; exact count is unpredictable.
        // Just assert the call succeeds and keys are 28 or 32 bytes (key hash).
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
        let nodes = chain.get_registered_nodes().await.expect("get_registered_nodes");
        eprintln!("preprod registered nodes: {}", nodes.len());
    }
}

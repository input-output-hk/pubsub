//! Blockfrost backend for `CardanoChainState`.
//!
//! Pagination: all list endpoints walked page-by-page (100 items/page).
//! Auth: `project_id` request header.
//! Inline datums: returned as lowercase hex CBOR; decoded via PlutusData (minicbor).

use std::net::SocketAddr;
use std::time::Duration;

use bytes::Bytes;
use serde::Deserialize;

use pubsub_types::error::PubSubError;
use pubsub_types::message::{PublisherCredential, PublisherId, TopicId};
use pubsub_types::node::{NodeId, NodeInfo};
use pubsub_types::topic::TopicConfig;

use super::ContractAddresses;
use super::datum::{
    decode_node_registry_datum, decode_plutus_data, decode_publisher_vault_datum,
    decode_topic_datum, on_chain_int_to_topic_id, topic_id_to_on_chain_int,
};

#[derive(Deserialize)]
struct BfUtxo {
    inline_datum: Option<String>,
    amount: Vec<BfAmount>,
}

#[derive(Deserialize)]
struct BfAmount {
    unit: String,
}

#[derive(Deserialize)]
struct BfDrep {
    /// Raw verification key hash as lowercase hex (not bech32 drep ID).
    hex: String,
    /// If true this is a script credential, not a key — skip it.
    has_script: bool,
}

pub(super) struct BlockfrostClient {
    project_id: String,
    base_url: String,
    client: reqwest::Client,
}

impl BlockfrostClient {
    pub fn new(project_id: &str, base_url: &str) -> Self {
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
                None => break,
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

pub(super) async fn get_registered_nodes(
    project_id: &str,
    base_url: &str,
    contracts: &ContractAddresses,
) -> Result<Vec<NodeInfo>, PubSubError> {
    let bf = BlockfrostClient::new(project_id, base_url);
    let utxos = bf.get_utxos_at(&contracts.node_registry_addr).await?;

    let mut nodes = Vec::new();
    for utxo in &utxos {
        let Some(hex) = &utxo.inline_datum else { continue };
        let data = decode_plutus_data(hex)?;
        let Some(registry) = decode_node_registry_datum(&data) else { continue };
        for entry in registry.nodes {
            let addr_str = String::from_utf8_lossy(&entry.addr).to_string();
            let addr: SocketAddr = addr_str
                .parse()
                .map_err(|e| PubSubError::ChainState(format!("node addr parse: {e}")))?;
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

pub(super) async fn get_topic_config(
    project_id: &str,
    base_url: &str,
    contracts: &ContractAddresses,
    topic: &TopicId,
) -> Result<Option<TopicConfig>, PubSubError> {
    let target_n = topic_id_to_on_chain_int(topic).ok_or_else(|| {
        PubSubError::ChainState(
            "TopicId bytes 8-31 are non-zero; not a registry-originated topic".into(),
        )
    })?;

    let bf = BlockfrostClient::new(project_id, base_url);
    let utxos = bf.get_utxos_at(&contracts.topic_validator_addr).await?;

    for utxo in &utxos {
        let Some(hex) = &utxo.inline_datum else { continue };
        let data = decode_plutus_data(hex)?;
        let Some(td) = decode_topic_datum(&data) else { continue };
        if td.topic_id != target_n || !td.alive {
            continue;
        }

        // Publisher token name: [4-byte-big-endian-topic-id][28-byte-pkh][0x70]
        // Filter on policy_id + 4-byte topic_id; pkh and suffix follow.
        let topic_hex_prefix =
            format!("{}{:08x}", contracts.registry_policy_id, td.topic_id);
        let vault_utxos = bf.get_utxos_at(&contracts.publisher_vault_addr).await?;
        let mut authorized_publishers = Vec::new();
        for vault in &vault_utxos {
            let has_token = vault
                .amount
                .iter()
                .any(|a| a.unit.starts_with(&topic_hex_prefix));
            if !has_token {
                continue;
            }
            let Some(vhex) = &vault.inline_datum else { continue };
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

pub(super) async fn get_all_topics(
    project_id: &str,
    base_url: &str,
    contracts: &ContractAddresses,
) -> Result<Vec<TopicConfig>, PubSubError> {
    let bf = BlockfrostClient::new(project_id, base_url);
    let utxos = bf.get_utxos_at(&contracts.topic_validator_addr).await?;
    // Eagerly fetch all vault UTxOs once; filter per-topic below.
    let vault_utxos = bf.get_utxos_at(&contracts.publisher_vault_addr).await?;

    let mut topics = Vec::new();
    for utxo in &utxos {
        let Some(hex) = &utxo.inline_datum else { continue };
        let data = decode_plutus_data(hex)?;
        let Some(td) = decode_topic_datum(&data) else { continue };
        if !td.alive {
            continue;
        }
        let topic_id = on_chain_int_to_topic_id(td.topic_id);
        let topic_hex_prefix =
            format!("{}{:08x}", contracts.registry_policy_id, td.topic_id);
        let mut authorized_publishers = Vec::new();
        for vault in &vault_utxos {
            let has_token = vault
                .amount
                .iter()
                .any(|a| a.unit.starts_with(&topic_hex_prefix));
            if !has_token {
                continue;
            }
            let Some(vhex) = &vault.inline_datum else { continue };
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

pub(super) async fn get_drep_keys(
    project_id: &str,
    base_url: &str,
) -> Result<Vec<Bytes>, PubSubError> {
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

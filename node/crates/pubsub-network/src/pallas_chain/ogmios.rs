//! Ogmios v6+ JSON-RPC backend for `CardanoChainState`.
//!
//! Uses the HTTP POST interface (Ogmios v6.0+) at the configured URL.
//! Standard JSON-RPC 2.0: POST / with body `{jsonrpc, method, params, id}`.
//! Inline datums returned as lowercase hex CBOR; same decoder as Blockfrost.
//! Token presence is checked via `value.policies[policyId][assetNameHex]`.
//!
//! No API key required — ideal for permissionless relay node operators.

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
struct OgmiosUtxo {
    /// Inline datum as lower-case hex CBOR (same encoding as Blockfrost).
    datum: Option<String>,
    value: serde_json::Value,
}

pub(super) struct OgmiosClient {
    url: String,
    client: reqwest::Client,
}

impl OgmiosClient {
    pub fn new(url: &str) -> Self {
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

pub(super) async fn get_registered_nodes(
    url: &str,
    contracts: &ContractAddresses,
) -> Result<Vec<NodeInfo>, PubSubError> {
    let og = OgmiosClient::new(url);
    let utxos = og.query_utxos(&contracts.node_registry_addr).await?;
    let mut nodes = Vec::new();
    for utxo in &utxos {
        let Some(hex) = &utxo.datum else { continue };
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
    url: &str,
    contracts: &ContractAddresses,
    topic: &TopicId,
) -> Result<Option<TopicConfig>, PubSubError> {
    let target_n = topic_id_to_on_chain_int(topic).ok_or_else(|| {
        PubSubError::ChainState(
            "TopicId bytes 8-31 are non-zero; not a registry-originated topic".into(),
        )
    })?;
    let og = OgmiosClient::new(url);
    let utxos = og.query_utxos(&contracts.topic_validator_addr).await?;
    for utxo in &utxos {
        let Some(hex) = &utxo.datum else { continue };
        let data = decode_plutus_data(hex)?;
        let Some(td) = decode_topic_datum(&data) else { continue };
        if td.topic_id != target_n || !td.alive {
            continue;
        }
        let asset_name_prefix = format!("{:08x}", td.topic_id);
        let vault_utxos = og.query_utxos(&contracts.publisher_vault_addr).await?;
        let mut authorized_publishers = Vec::new();
        for vault in &vault_utxos {
            if !ogmios_has_token(vault, &contracts.registry_policy_id, &asset_name_prefix) {
                continue;
            }
            let Some(vhex) = &vault.datum else { continue };
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
    url: &str,
    contracts: &ContractAddresses,
) -> Result<Vec<TopicConfig>, PubSubError> {
    let og = OgmiosClient::new(url);
    let utxos = og.query_utxos(&contracts.topic_validator_addr).await?;
    let vault_utxos = og.query_utxos(&contracts.publisher_vault_addr).await?;
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
            if !ogmios_has_token(vault, &contracts.registry_policy_id, &asset_name_prefix) {
                continue;
            }
            let Some(vhex) = &vault.datum else { continue };
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

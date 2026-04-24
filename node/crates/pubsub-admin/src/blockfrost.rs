use anyhow::{anyhow, bail, Context, Result};
use reqwest::Client;
use serde::Deserialize;

pub struct BlockfrostClient {
    client: Client,
    base_url: String,
    project_id: String,
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ProtocolParams {
    pub min_fee_a: u64,
    pub min_fee_b: u64,
    pub execution_unit_prices: ExecPrices,
    /// Cost model parameters per Plutus version (alphabetically-sorted BTreeMap
    /// via serde_json, which gives us the correct canonical ordering).
    pub cost_models: serde_json::Value,
}

#[derive(Deserialize)]
pub struct ExecPrices {
    pub price_memory: String,
    pub price_steps: String,
}

impl ExecPrices {
    /// Execution fee in lovelace for the given budget.
    pub fn fee(&self, mem: u64, steps: u64) -> u64 {
        let pm: f64 = self.price_memory.parse().unwrap_or(0.0577);
        let ps: f64 = self.price_steps.parse().unwrap_or(0.0000721);
        (pm * mem as f64 + ps * steps as f64).ceil() as u64
    }
}

impl ProtocolParams {
    /// Returns the Plutus V3 cost model as a sorted Vec<i64> ready for
    /// `StagingTransaction::language_view(ScriptKind::PlutusV3, ...)`.
    ///
    /// serde_json deserialises JSON objects into a BTreeMap (keys sorted
    /// alphabetically), which matches the Cardano canonical ordering.
    pub fn cost_model_v3(&self) -> Result<Vec<i64>> {
        let obj = self
            .cost_models
            .get("PlutusV3")
            .and_then(|v| v.as_object())
            .ok_or_else(|| anyhow!("PlutusV3 cost model not found in protocol params"))?;
        obj.values()
            .map(|v| v.as_i64().ok_or_else(|| anyhow!("cost model value is not an integer")))
            .collect()
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Utxo {
    pub tx_hash: String,
    pub tx_index: u64,
    pub amount: Vec<Amount>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Amount {
    pub unit: String,
    pub quantity: String,
}

impl Utxo {
    pub fn lovelace(&self) -> u64 {
        self.amount
            .iter()
            .find(|a| a.unit == "lovelace")
            .and_then(|a| a.quantity.parse().ok())
            .unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

impl BlockfrostClient {
    pub fn new(project_id: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            project_id: project_id.into(),
        }
    }

    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = format!("{}/{}", self.base_url, path);
        let resp = self
            .client
            .get(&url)
            .header("project_id", &self.project_id)
            .send()
            .await
            .context("Blockfrost HTTP request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("Blockfrost {status}: {body}");
        }
        resp.json::<T>().await.context("Blockfrost JSON parse failed")
    }

    pub async fn protocol_params(&self) -> Result<ProtocolParams> {
        self.get("epochs/latest/parameters").await
    }

    pub async fn utxos_at(&self, address: &str) -> Result<Vec<Utxo>> {
        // Walk pages until empty.
        let mut all = Vec::new();
        let mut page = 1u32;
        loop {
            let path = format!("addresses/{}/utxos?page={}&count=100", address, page);
            let page_items: Vec<Utxo> = match self.get(&path).await {
                Ok(items) => items,
                Err(e) if e.to_string().contains("404") => break,
                Err(e) => return Err(e),
            };
            let done = page_items.len() < 100;
            all.extend(page_items);
            if done { break; }
            page += 1;
        }
        Ok(all)
    }

    /// Find a specific UTxO by hash and index within a known address's UTxO set.
    pub async fn find_utxo(
        &self,
        payment_addr: &str,
        tx_hash: &str,
        tx_index: u64,
    ) -> Result<Utxo> {
        let utxos = self.utxos_at(payment_addr).await?;
        utxos
            .into_iter()
            .find(|u| u.tx_hash == tx_hash && u.tx_index == tx_index)
            .ok_or_else(|| anyhow!("UTxO {tx_hash}#{tx_index} not found at {payment_addr}"))
    }

    /// Submit a raw transaction (CBOR bytes).  Returns the tx hash.
    pub async fn submit_tx(&self, tx_cbor: &[u8]) -> Result<String> {
        let url = format!("{}/tx/submit", self.base_url);
        let resp = self
            .client
            .post(&url)
            .header("project_id", &self.project_id)
            .header("Content-Type", "application/cbor")
            .body(tx_cbor.to_vec())
            .send()
            .await
            .context("Blockfrost submit request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("Blockfrost submit {status}: {body}");
        }
        // Response is the tx hash as a JSON string.
        let tx_id: String = resp.json().await.context("parsing submit response")?;
        Ok(tx_id)
    }
}

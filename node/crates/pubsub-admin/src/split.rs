use anyhow::{anyhow, Context, Result};
use pallas_addresses::Address;
use pallas_txbuilder::{BuildConway, Input, Output, StagingTransaction};

use crate::blockfrost::BlockfrostClient;
use crate::tx::load_signing_key;

pub struct SplitArgs {
    pub blockfrost_project_id: String,
    pub network_base_url: &'static str,
    pub payment_addr: String,
    pub payment_skey_path: std::path::PathBuf,
    pub utxo: String, // "txhash#index"
}

pub async fn run(args: SplitArgs) -> Result<()> {
    let bf = BlockfrostClient::new(&args.blockfrost_project_id, args.network_base_url);

    let (tx_hash, tx_index) = split_utxo_ref(&args.utxo)?;

    println!("Fetching protocol parameters...");
    let params = bf.protocol_params().await.context("fetching protocol params")?;

    println!("Looking up UTxO {}...", args.utxo);
    let utxo = bf
        .find_utxo(&args.payment_addr, &tx_hash, tx_index)
        .await
        .context("finding UTxO")?;
    let total = utxo.lovelace();
    println!("  value: {} lovelace ({:.2} ADA)", total, total as f64 / 1_000_000.0);

    let signing_key = load_signing_key(&args.payment_skey_path).context("loading signing key")?;

    let addr = Address::from_bech32(&args.payment_addr)
        .map_err(|e| anyhow!("invalid payment address: {e}"))?;

    let utxo_hash_bytes: [u8; 32] = hex::decode(&tx_hash)
        .context("decoding UTxO tx hash")?
        .try_into()
        .map_err(|_| anyhow!("UTxO hash must be 32 bytes"))?;

    // Two-pass fee calculation (no scripts, so exec_fee = 0).
    let tx_bytes = build_pass(
        utxo_hash_bytes,
        tx_index,
        total,
        &addr,
        params.min_fee_a,
        params.min_fee_b,
        &signing_key,
        500_000,
    )?;

    let actual_fee = params.min_fee_a * tx_bytes.len() as u64 + params.min_fee_b;

    let tx_bytes = build_pass(
        utxo_hash_bytes,
        tx_index,
        total,
        &addr,
        params.min_fee_a,
        params.min_fee_b,
        &signing_key,
        actual_fee,
    )?;

    let half = total / 2;
    let other = total - half - actual_fee;
    println!("Splitting into:");
    println!("  output 0: {} lovelace", half);
    println!("  output 1: {} lovelace  (after {} lovelace fee)", other, actual_fee);

    println!("Submitting...");
    let txid = bf.submit_tx(&tx_bytes).await.context("submitting split tx")?;
    println!("  ✓ tx: {txid}");
    println!();
    println!("Bootstrap UTxOs (use after the tx confirms ~1–2 min):");
    println!("  --topic-utxo {txid}#0");
    println!("  --node-utxo  {txid}#1");

    Ok(())
}

fn build_pass(
    utxo_hash_bytes: [u8; 32],
    utxo_index: u64,
    total_lovelace: u64,
    addr: &Address,
    min_fee_a: u64,
    min_fee_b: u64,
    signing_key: &pallas_crypto::key::ed25519::SecretKey,
    fee: u64,
) -> Result<Vec<u8>> {
    let half = total_lovelace / 2;
    let other = total_lovelace
        .checked_sub(half + fee)
        .ok_or_else(|| anyhow!("insufficient funds: need {} lovelace for fee", fee))?;

    let _ = (min_fee_a, min_fee_b); // used by caller for fee calc

    let input = Input::new(utxo_hash_bytes.into(), utxo_index);
    let staging = StagingTransaction::new()
        .input(input)
        .output(Output::new(addr.clone(), half))
        .output(Output::new(addr.clone(), other))
        .fee(fee);

    let built = staging.build_conway_raw().map_err(|e| anyhow!("tx build failed: {e}"))?;

    let sig = signing_key.sign(&built.tx_hash.0);
    let sig_bytes: [u8; 64] = sig
        .as_ref()
        .try_into()
        .map_err(|_| anyhow!("unexpected signature length"))?;

    let signed = built
        .add_signature(signing_key.public_key(), sig_bytes)
        .map_err(|e| anyhow!("signing failed: {e}"))?;

    Ok(signed.tx_bytes.0)
}

fn split_utxo_ref(utxo: &str) -> Result<(String, u64)> {
    let mut parts = utxo.splitn(2, '#');
    let hash = parts
        .next()
        .ok_or_else(|| anyhow!("invalid UTxO: {utxo}"))?
        .to_string();
    let index: u64 = parts
        .next()
        .ok_or_else(|| anyhow!("UTxO missing index: {utxo}"))?
        .parse()
        .map_err(|_| anyhow!("UTxO index is not a number: {utxo}"))?;
    Ok((hash, index))
}

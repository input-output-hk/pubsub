use anyhow::{anyhow, Context, Result};
use pallas_addresses::Address;
use pallas_crypto::key::ed25519::SecretKey;
use pallas_txbuilder::{
    BuildConway, ExUnits, Input, Output, ScriptKind, StagingTransaction,
};

use crate::blockfrost::ProtocolParams;

/// Conservative execution unit budget for a simple Plutus V3 minting policy
/// that only checks `must_spend_utxo`.  We over-declare; the actual usage is
/// far lower, but we pay slightly more fee in exchange for not needing a
/// Blockfrost evaluate round-trip.
pub const BOOTSTRAP_EX_UNITS: ExUnits = ExUnits {
    mem: 700_000,
    steps: 250_000_000,
};

/// Parse a 32-byte ed25519 signing key from a cardano-cli JSON key file.
///
/// Expected format:
/// ```json
/// { "type": "PaymentSigningKeyShelley_ed25519",
///   "cborHex": "5820<64 hex chars>" }
/// ```
pub fn load_signing_key(path: &std::path::Path) -> Result<SecretKey> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("reading key file {}", path.display()))?;
    let json: serde_json::Value = serde_json::from_str(&content).context("parsing key file JSON")?;

    let cbor_hex = json["cborHex"]
        .as_str()
        .ok_or_else(|| anyhow!("key file missing 'cborHex' field"))?;

    // Strip CBOR bytestring header — 5820 = 0x58 0x20 (32-byte bytestring)
    let hex_str = cbor_hex
        .strip_prefix("5820")
        .or_else(|| cbor_hex.strip_prefix("5821"))
        .unwrap_or(cbor_hex);

    let key_bytes = hex::decode(hex_str).context("hex-decoding signing key")?;
    let key_array: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| anyhow!("signing key must be exactly 32 bytes"))?;

    Ok(SecretKey::from(key_array))
}

/// Parse a hex policy ID string into a 28-byte array.
pub fn parse_policy_id(hex: &str) -> Result<pallas_crypto::hash::Hash<28>> {
    let bytes = hex::decode(hex).context("decoding policy ID hex")?;
    let arr: [u8; 28] = bytes
        .try_into()
        .map_err(|_| anyhow!("policy ID must be 28 bytes (56 hex chars)"))?;
    Ok(arr.into())
}

/// Build, sign, and CBOR-encode a bootstrap transaction for one registry contract.
///
/// The bootstrap UTxO is consumed as both the regular input and the collateral
/// input (valid for key-spend UTxOs with no native tokens).
///
/// Returns the raw CBOR bytes ready for Blockfrost `/tx/submit`.
pub fn build_bootstrap_tx(
    bootstrap_utxo_hash: &str,
    bootstrap_utxo_index: u64,
    bootstrap_utxo_lovelace: u64,
    script_address: &str,
    policy_id_hex: &str,
    token_name: &[u8],
    datum_cbor: &[u8],
    mint_script_cbor: &[u8],
    change_address: &str,
    params: &ProtocolParams,
    signing_key: &SecretKey,
) -> Result<Vec<u8>> {
    let policy_id = parse_policy_id(policy_id_hex)?;
    let script_addr = Address::from_bech32(script_address)
        .map_err(|e| anyhow!("invalid script address: {e}"))?;
    let change_addr = Address::from_bech32(change_address)
        .map_err(|e| anyhow!("invalid change address: {e}"))?;

    // BootstrapRegistry redeemer = Constr 0 [] — CBOR d87980
    let redeemer_cbor = hex::decode("d87980").unwrap();

    // Cost model for script_data_hash
    let cost_model = params.cost_model_v3()?;
    let exec_fee = params.exec_prices().fee(
        BOOTSTRAP_EX_UNITS.mem,
        BOOTSTRAP_EX_UNITS.steps,
    );

    // Two-pass fee calculation: first with a generous estimate, then exact.
    let tx_bytes = build_pass(
        bootstrap_utxo_hash,
        bootstrap_utxo_index,
        bootstrap_utxo_lovelace,
        &script_addr,
        &change_addr,
        policy_id,
        token_name,
        datum_cbor,
        mint_script_cbor,
        &redeemer_cbor,
        &cost_model,
        params.min_fee_a,
        params.min_fee_b,
        exec_fee,
        signing_key,
        500_000, // initial over-estimate for fee
    )?;

    // Measure actual size, recompute exact fee.
    let actual_fee = params.min_fee_a * tx_bytes.len() as u64 + params.min_fee_b + exec_fee;

    // Rebuild with the exact fee (change output adjusts automatically).
    build_pass(
        bootstrap_utxo_hash,
        bootstrap_utxo_index,
        bootstrap_utxo_lovelace,
        &script_addr,
        &change_addr,
        policy_id,
        token_name,
        datum_cbor,
        mint_script_cbor,
        &redeemer_cbor,
        &cost_model,
        params.min_fee_a,
        params.min_fee_b,
        exec_fee,
        signing_key,
        actual_fee,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_pass(
    utxo_hash: &str,
    utxo_index: u64,
    input_lovelace: u64,
    script_addr: &Address,
    change_addr: &Address,
    policy_id: pallas_crypto::hash::Hash<28>,
    token_name: &[u8],
    datum_cbor: &[u8],
    mint_script_cbor: &[u8],
    redeemer_cbor: &[u8],
    cost_model: &[i64],
    min_fee_a: u64,
    _min_fee_b: u64,
    exec_fee: u64,
    signing_key: &SecretKey,
    fee: u64,
) -> Result<Vec<u8>> {
    let _ = (min_fee_a, exec_fee); // used by caller

    let utxo_hash_bytes: [u8; 32] = hex::decode(utxo_hash)
        .context("decoding UTxO tx hash")?
        .try_into()
        .map_err(|_| anyhow!("UTxO hash must be 32 bytes"))?;

    let script_output_lovelace = 2_000_000u64; // minimum UTxO with token
    let change_lovelace = input_lovelace
        .checked_sub(script_output_lovelace + fee)
        .ok_or_else(|| {
            anyhow!(
                "Insufficient funds: UTxO has {} lovelace, need at least {} (2 ADA output + {} fee)",
                input_lovelace,
                script_output_lovelace + fee,
                fee,
            )
        })?;

    let input = Input::new(utxo_hash_bytes.into(), utxo_index);

    let script_output = Output::new(script_addr.clone(), script_output_lovelace)
        .add_asset(policy_id, token_name.to_vec(), 1)
        .context("adding token to output")?
        .set_inline_datum(datum_cbor.to_vec());

    let change_output = Output::new(change_addr.clone(), change_lovelace);

    let staging = StagingTransaction::new()
        .input(input.clone())
        .collateral_input(input)
        .output(script_output)
        .output(change_output)
        .mint_asset(policy_id, token_name.to_vec(), 1)
        .context("adding mint")?
        .add_mint_redeemer(
            policy_id,
            redeemer_cbor.to_vec(),
            Some(BOOTSTRAP_EX_UNITS),
        )
        .script(ScriptKind::PlutusV3, mint_script_cbor.to_vec())
        .language_view(ScriptKind::PlutusV3, cost_model.to_vec())
        .fee(fee);

    let built = staging.build_conway_raw().map_err(|e| anyhow!("tx build failed: {e}"))?;

    // Sign: ed25519_sign(tx_hash)
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

use anyhow::{bail, Context, Result};
use std::{path::Path, process::Command};

/// Apply a CBOR-encoded parameter to a specific validator in a blueprint,
/// writing the parameterized blueprint to `output`.
///
/// `module` and `validator` select the target (e.g. "registry", "registry").
pub fn apply_param(
    blueprint_in: &Path,
    blueprint_out: &Path,
    module: &str,
    validator: &str,
    cbor_hex: &str,
) -> Result<()> {
    let status = Command::new("aiken")
        .args([
            "blueprint",
            "apply",
            "--in",
            &blueprint_in.to_string_lossy(),
            "--out",
            &blueprint_out.to_string_lossy(),
            "--module",
            module,
            "--validator",
            validator,
            cbor_hex,
        ])
        .status()
        .context("failed to run `aiken blueprint apply`")?;

    if !status.success() {
        bail!("`aiken blueprint apply` failed (module={module}, validator={validator})");
    }
    Ok(())
}

/// Derive the bech32 script address for a validator in a blueprint.
/// `mainnet=true` produces an `addr1...` address; otherwise `addr_test1...`.
pub fn address(
    blueprint: &Path,
    module: &str,
    validator: &str,
    mainnet: bool,
) -> Result<String> {
    let mut cmd = Command::new("aiken");
    cmd.args([
        "blueprint",
        "address",
        "--in",
        &blueprint.to_string_lossy(),
        "--module",
        module,
        "--validator",
        validator,
    ]);
    if mainnet {
        cmd.arg("--mainnet");
    }

    let out = cmd
        .output()
        .context("failed to run `aiken blueprint address`")?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        bail!("`aiken blueprint address` failed: {stderr}");
    }
    Ok(String::from_utf8(out.stdout)
        .context("aiken address output is not UTF-8")?
        .trim()
        .to_string())
}

/// Derive the policy ID (script hash) for a minting validator in a blueprint.
pub fn policy_id(blueprint: &Path, module: &str, validator: &str) -> Result<String> {
    let out = Command::new("aiken")
        .args([
            "blueprint",
            "policy",
            "--in",
            &blueprint.to_string_lossy(),
            "--module",
            module,
            "--validator",
            validator,
        ])
        .output()
        .context("failed to run `aiken blueprint policy`")?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        bail!("`aiken blueprint policy` failed: {stderr}");
    }
    Ok(String::from_utf8(out.stdout)
        .context("aiken policy output is not UTF-8")?
        .trim()
        .to_string())
}

/// Extract the `compiledCode` hex for a named validator from a blueprint JSON file.
pub fn compiled_code(blueprint: &Path, title: &str) -> Result<String> {
    let content = std::fs::read_to_string(blueprint)
        .with_context(|| format!("reading blueprint {}", blueprint.display()))?;
    let json: serde_json::Value =
        serde_json::from_str(&content).context("parsing blueprint JSON")?;

    json["validators"]
        .as_array()
        .and_then(|arr| {
            arr.iter().find(|v| v["title"].as_str() == Some(title))
        })
        .and_then(|v| v["compiledCode"].as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("validator '{title}' not found in blueprint"))
}

//! CLI / config resolution helpers.
//!
//! Each subcommand wants the same set of inputs (network, Blockfrost ID,
//! signing key, payment address, funding UTxO, .env path). The helpers below
//! resolve each one from, in order: a CLI flag → environment variable →
//! interactive prompt. Validation lives next to each resolver so a bad value
//! either gets re-prompted or fails fast.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use dialoguer::Select;

use crate::bootstrap::Network;

// ---------------------------------------------------------------------------
// Network (arrow-key Select — raw mode is fine here, no paste involved)
// ---------------------------------------------------------------------------

pub fn resolve_network(flag: Option<String>) -> Result<Network> {
    if let Some(s) = flag {
        return parse_network(&s);
    }

    let choices = &["preprod", "preview", "mainnet"];
    let idx = Select::new()
        .with_prompt("Network")
        .items(choices)
        .default(0)
        .interact()
        .map_err(|e| anyhow!("network selection failed: {e}"))?;

    parse_network(choices[idx])
}

fn parse_network(s: &str) -> Result<Network> {
    match s.to_lowercase().as_str() {
        "preprod" => Ok(Network::Preprod),
        "preview" => Ok(Network::Preview),
        "mainnet" => Ok(Network::Mainnet),
        _ => Err(anyhow!(
            "unknown network '{s}' — expected preprod, preview, or mainnet"
        )),
    }
}

// ---------------------------------------------------------------------------
// Text prompts — plain readline (no raw mode, paste-safe)
// ---------------------------------------------------------------------------

fn readline(prompt: &str) -> Result<String> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    write!(out, "{prompt}: ")?;
    out.flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

fn prompt_validated(
    prompt: &str,
    validate: impl Fn(&str) -> Result<(), &'static str>,
) -> Result<String> {
    loop {
        let val = readline(prompt)?;
        match validate(&val) {
            Ok(()) => return Ok(val),
            Err(msg) => eprintln!("  ✗ {msg}"),
        }
    }
}

pub fn resolve_blockfrost_id(flag: Option<String>) -> Result<String> {
    if let Some(id) = flag {
        return Ok(id);
    }
    if let Ok(id) = std::env::var("BLOCKFROST_PROJECT_ID") {
        if !id.is_empty() {
            return Ok(id);
        }
    }
    prompt_validated("Blockfrost project ID", |s| {
        if s.is_empty() {
            Err("cannot be empty")
        } else {
            Ok(())
        }
    })
}

pub fn resolve_skey_path(flag: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = flag {
        if !p.exists() {
            return Err(anyhow!("signing key file not found: {}", p.display()));
        }
        return Ok(p);
    }
    let s = prompt_validated("Payment signing key path (.skey)", |s| {
        if std::path::Path::new(s).exists() {
            Ok(())
        } else {
            Err("file not found")
        }
    })?;
    Ok(PathBuf::from(s))
}

pub fn resolve_payment_addr(flag: Option<String>) -> Result<String> {
    if let Some(a) = flag {
        return load_addr(a);
    }
    let s = prompt_validated("Payment address or .addr file path", |s| {
        let resolved = if std::path::Path::new(s).exists() {
            std::fs::read_to_string(s).unwrap_or_default()
        } else {
            s.to_string()
        };
        if resolved.trim().starts_with("addr") {
            Ok(())
        } else {
            Err("must be a bech32 addr or a path to a .addr file")
        }
    })?;
    load_addr(s)
}

fn load_addr(s: String) -> Result<String> {
    let path = std::path::Path::new(&s);
    let looks_like_path = s.contains('/') || s.contains('\\') || path.extension().is_some();

    let addr = if looks_like_path {
        std::fs::read_to_string(path)
            .with_context(|| {
                format!(
                    "reading address file '{}' (cwd: {})",
                    s,
                    std::env::current_dir().unwrap_or_default().display()
                )
            })?
            .trim()
            .to_string()
    } else {
        s
    };

    if addr.starts_with("addr") {
        Ok(addr)
    } else {
        Err(anyhow!(
            "invalid payment address '{addr}' — expected bech32 starting with 'addr'"
        ))
    }
}

pub fn resolve_utxo(flag: Option<String>, label: &str) -> Result<String> {
    if let Some(u) = flag {
        return validate_utxo(u, label);
    }
    let s = prompt_validated(
        &format!("{label} (<64-hex-txhash>#<index>)"),
        validate_utxo_str,
    )?;
    Ok(s)
}

fn validate_utxo(s: String, label: &str) -> Result<String> {
    validate_utxo_str(&s).map_err(|e| anyhow!("invalid {label} '{s}': {e}"))?;
    Ok(s)
}

fn validate_utxo_str(s: &str) -> Result<(), &'static str> {
    let mut parts = s.splitn(2, '#');
    let hash = parts.next().unwrap_or("");
    let index = parts.next().unwrap_or("");
    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("tx hash must be 64 hex characters");
    }
    if index.parse::<u64>().is_err() {
        return Err("index must be a non-negative integer");
    }
    Ok(())
}

pub fn resolve_env_file(flag: Option<PathBuf>, network: &Network) -> Result<PathBuf> {
    if let Some(p) = flag {
        if !p.exists() {
            return Err(anyhow!(".env file not found: {}", p.display()));
        }
        return Ok(p);
    }
    let default = PathBuf::from(format!("local/.env.{}", network.env_name()));
    if default.exists() {
        println!("Using .env file: {}", default.display());
        return Ok(default);
    }
    let s = prompt_validated(".env file path (written by bootstrap)", |s| {
        if std::path::Path::new(s).exists() {
            Ok(())
        } else {
            Err("file not found")
        }
    })?;
    Ok(PathBuf::from(s))
}

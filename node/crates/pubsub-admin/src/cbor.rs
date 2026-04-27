//! CBOR / PlutusData encode + decode helpers shared by `bootstrap`,
//! `publish_scripts`, and `create_topic`.

use anyhow::{anyhow, Context, Result};

/// CBOR-encode an OutputReference (`Constr 0 [ByteArray(txhash), Int(index)]`).
pub fn cbor_output_ref(tx_hash: &str, index: u64) -> Result<String> {
    let hash_bytes = hex::decode(tx_hash).context("decoding UTxO tx hash")?;
    if hash_bytes.len() != 32 {
        return Err(anyhow!("tx hash must be 32 bytes"));
    }
    let mut out = vec![0xd8, 0x79, 0x82, 0x58, 0x20];
    out.extend_from_slice(&hash_bytes);
    out.extend_from_slice(&cbor_uint(index));
    Ok(hex::encode(out))
}

/// CBOR-encode a PolicyId (28-byte raw bytestring).
pub fn cbor_policy_id(policy_id_hex: &str) -> Result<String> {
    let bytes = hex::decode(policy_id_hex).context("decoding policy ID hex")?;
    if bytes.len() != 28 {
        return Err(anyhow!("policy ID must be 28 bytes"));
    }
    let mut out = vec![0x58, 0x1c];
    out.extend_from_slice(&bytes);
    Ok(hex::encode(out))
}

/// CBOR unsigned integer encoding (canonical: smallest representation).
pub fn cbor_uint(n: u64) -> Vec<u8> {
    if n <= 23 {
        vec![n as u8]
    } else if n <= 0xff {
        vec![0x18, n as u8]
    } else if n <= 0xffff {
        vec![0x19, (n >> 8) as u8, n as u8]
    } else if n <= 0xffff_ffff {
        vec![0x1a, (n >> 24) as u8, (n >> 16) as u8, (n >> 8) as u8, n as u8]
    } else {
        vec![
            0x1b,
            (n >> 56) as u8, (n >> 48) as u8, (n >> 40) as u8, (n >> 32) as u8,
            (n >> 24) as u8, (n >> 16) as u8, (n >> 8) as u8, n as u8,
        ]
    }
}

/// CBOR Plutus constructor: tags 121-127 for indices 0-6, tag 102 for 7+.
pub fn cbor_constr(n: u8, fields: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    if n <= 6 {
        out.push(0xd8);
        out.push(0x79 + n);
    } else {
        out.push(0xd8);
        out.push(0x66);
        out.push(0x82);
        out.extend_from_slice(&cbor_uint(n as u64));
    }
    out.extend_from_slice(&cbor_array_header(fields.len()));
    for f in fields {
        out.extend_from_slice(f);
    }
    out
}

pub fn cbor_array_header(len: usize) -> Vec<u8> {
    if len <= 23 {
        vec![0x80 | len as u8]
    } else if len <= 0xff {
        vec![0x98, len as u8]
    } else {
        vec![0x99, (len >> 8) as u8, len as u8]
    }
}

pub fn cbor_bytes(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let len = b.len();
    if len <= 23 {
        out.push(0x40 | len as u8);
    } else if len <= 0xff {
        out.push(0x58);
        out.push(len as u8);
    } else if len <= 0xffff {
        out.push(0x59);
        out.push((len >> 8) as u8);
        out.push(len as u8);
    } else {
        out.push(0x5a);
        out.extend_from_slice(&(len as u32).to_be_bytes());
    }
    out.extend_from_slice(b);
    out
}

/// Decode a CBOR unsigned integer. Returns the value and the remaining slice.
pub fn decode_cbor_uint(buf: &[u8]) -> Result<(u64, &[u8])> {
    if buf.is_empty() {
        return Err(anyhow!("unexpected end of CBOR data"));
    }
    match buf[0] {
        n @ 0x00..=0x17 => Ok((n as u64, &buf[1..])),
        0x18 if buf.len() >= 2 => Ok((buf[1] as u64, &buf[2..])),
        0x19 if buf.len() >= 3 => Ok((u16::from_be_bytes([buf[1], buf[2]]) as u64, &buf[3..])),
        0x1a if buf.len() >= 5 => Ok((
            u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]) as u64,
            &buf[5..],
        )),
        0x1b if buf.len() >= 9 => Ok((
            u64::from_be_bytes(buf[1..9].try_into().unwrap()),
            &buf[9..],
        )),
        other => Err(anyhow!("unexpected CBOR byte 0x{other:02x}")),
    }
}

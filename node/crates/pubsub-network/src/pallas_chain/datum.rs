//! Mirrors of the Aiken on-chain datum types and the PlutusData decoders.
//!
//! Datum layout (Aiken → Rust)
//! ───────────────────────────
//! All datums use CBOR tag 121 (Constr 0) at the top level.
//!
//!   TopicDatum          — fields 0-7: topic_id(Int), name(Bytes), owners([Bytes]),
//!                          admins([Bytes]), replication_factor(Int), retention_period(Int),
//!                          alive(Constr 1[]=True | Constr 0[]=False), published_at_epoch(Int)
//!
//!   NodeRegistryDatum   — fields 0-2: nodes([NodeEntry]), min_deposit(Int), epoch(Int)
//!     NodeEntry         — fields 0-3: node_id(Bytes), addr(Bytes), stake_key(Bytes), epoch(Int)
//!
//!   PublisherVaultDatum — fields 0-1: topic_id(Int), publisher(Bytes)

use pallas::ledger::primitives::{BigInt, Constr, PlutusData};

use pubsub_types::error::PubSubError;
use pubsub_types::message::TopicId;

pub(super) struct TopicDatum {
    pub topic_id: u64,
    pub name: Vec<u8>,
    // owners and admins (fields 2-3) are parsed but not stored — TopicConfig
    // uses the publisher vault UTxOs for authorization, not these lists.
    pub replication_factor: u64,
    pub retention_period: u64,
    pub alive: bool,
    // published_at_epoch (field 7) is on-chain bookkeeping; not used off-chain.
}

pub(super) struct NodeRegistryDatum {
    pub nodes: Vec<NodeEntryDatum>,
}

pub(super) struct NodeEntryDatum {
    pub node_id: Vec<u8>,
    pub addr: Vec<u8>,
    pub stake_key: Vec<u8>,
}

pub(super) struct PublisherVaultDatum {
    pub topic_id: u64,
    pub publisher: Vec<u8>,
}

pub(super) fn decode_plutus_data(hex_str: &str) -> Result<PlutusData, PubSubError> {
    let bytes =
        hex::decode(hex_str).map_err(|e| PubSubError::Codec(format!("hex decode: {e}")))?;
    pallas::codec::minicbor::decode(&bytes)
        .map_err(|e| PubSubError::Codec(format!("CBOR decode: {e}")))
}

fn constr0_fields(data: &PlutusData) -> Option<&[PlutusData]> {
    match data {
        PlutusData::Constr(Constr { tag, .. }) if *tag == 121 => match data {
            PlutusData::Constr(c) => Some(&c.fields),
            _ => unreachable!(),
        },
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

pub(super) fn decode_topic_datum(data: &PlutusData) -> Option<TopicDatum> {
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
    Some(TopicDatum {
        topic_id,
        name,
        replication_factor,
        retention_period,
        alive,
    })
}

pub(super) fn decode_node_registry_datum(data: &PlutusData) -> Option<NodeRegistryDatum> {
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
            // field 3 (registered_at_epoch) is on-chain bookkeeping; validate type only.
            bigint_u64(&nf[3])?;
            Some(NodeEntryDatum {
                node_id: pdata_bytes(&nf[0])?.to_vec(),
                addr: pdata_bytes(&nf[1])?.to_vec(),
                stake_key: pdata_bytes(&nf[2])?.to_vec(),
            })
        })
        .collect();
    // fields 1 (min_deposit_lovelace) and 2 (epoch) — validate types but don't store.
    bigint_u64(&f[1])?;
    bigint_u64(&f[2])?;
    Some(NodeRegistryDatum { nodes })
}

pub(super) fn decode_publisher_vault_datum(data: &PlutusData) -> Option<PublisherVaultDatum> {
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
pub(super) fn on_chain_int_to_topic_id(n: u64) -> TopicId {
    let mut id = [0u8; 32];
    id[..8].copy_from_slice(&n.to_be_bytes());
    TopicId(id)
}

/// Returns None if the TopicId was not created by `on_chain_int_to_topic_id`
/// (i.e. bytes 8..32 are non-zero, meaning it's a hash-derived ID).
pub(super) fn topic_id_to_on_chain_int(id: &TopicId) -> Option<u64> {
    if id.0[8..].iter().any(|&b| b != 0) {
        return None;
    }
    Some(u64::from_be_bytes(id.0[..8].try_into().unwrap()))
}

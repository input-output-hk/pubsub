use std::collections::BTreeMap;
use std::net::SocketAddr;

use anyhow::Result;
use bytes::Bytes;
use clap::{Parser, Subcommand};
use pallas_crypto::key::ed25519::SecretKey;

use pubsub_types::message::{
    CredentialType, Message, PublishAck, PublisherCredential, PublisherId, SubscribeRequest,
    TopicId,
};

use pubsub_network::codec::CborCodec;
use pubsub_network::transport::QuicTransport;
use pubsub_types::traits::{Codec, PublishTransport, SubscribeTransport};

#[derive(Parser)]
#[command(name = "pubsub-cli", about = "Cardano PubSub CLI")]
struct Cli {
    /// PubSub node address to connect to
    #[arg(short, long, default_value = "127.0.0.1:9000")]
    node: SocketAddr,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Publish a message to a topic.
    ///
    /// Specify the topic with EITHER `--topic <name>` (TopicId = Blake2b-256(name) —
    /// works for off-chain / mock-chain topics) OR `--topic-id <u64>` (the on-chain
    /// integer id encoded as a 32-byte TopicId — required for chain-registered
    /// topics).  Exactly one is required.
    Publish {
        /// Topic name. Hashed with Blake2b-256 to form TopicId.
        #[arg(short, long, conflicts_with = "topic_id", required_unless_present = "topic_id")]
        topic: Option<String>,

        /// On-chain topic id (u64). Encoded as a 32-byte TopicId with BE u64 in bytes 0..8.
        #[arg(long, conflicts_with = "topic")]
        topic_id: Option<u64>,

        /// Message payload (text)
        #[arg(short, long)]
        message: String,

        /// Credential type tag: ed25519 | pool-kes | drep | authority
        #[arg(long, default_value = "ed25519")]
        credential_type: String,
    },

    /// Subscribe to a topic and print received messages.
    ///
    /// Specify the topic with EITHER `--topic <name>` OR `--topic-id <u64>`.
    /// See `publish` for the difference. Exactly one is required.
    Subscribe {
        /// Topic name. Hashed with Blake2b-256 to form TopicId.
        #[arg(short, long, conflicts_with = "topic_id", required_unless_present = "topic_id")]
        topic: Option<String>,

        /// On-chain topic id (u64).
        #[arg(long, conflicts_with = "topic")]
        topic_id: Option<u64>,

        /// Replay starts after this sequence number.  Default 0 = full TTL
        /// window held in the node's HotCache.
        #[arg(long, default_value_t = 0)]
        since_seq: u64,

        /// Soft cap on the replay batch.
        #[arg(long, default_value_t = 1000)]
        limit: u32,
    },

    /// Show node status
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Publish { topic, topic_id, message, credential_type } => {
            let target = TopicTarget::from_args(topic, topic_id)?;
            publish(&cli.node, target, &message, &credential_type).await?;
        }
        Commands::Subscribe { topic, topic_id, since_seq, limit } => {
            let target = TopicTarget::from_args(topic, topic_id)?;
            subscribe(&cli.node, target, since_seq, limit).await?;
        }
        Commands::Status => {
            println!("Status check not yet implemented (needs gRPC API)");
        }
    }

    Ok(())
}

/// One of (--topic <name>) or (--topic-id <u64>).  Carries the resolved
/// TopicId plus a human-readable label for logs.
struct TopicTarget {
    topic_id: TopicId,
    label: String,
}

impl TopicTarget {
    fn from_args(name: Option<String>, id: Option<u64>) -> Result<Self> {
        match (name, id) {
            (Some(n), None) => Ok(Self {
                topic_id: topic_id_from_name(&n),
                label: format!("name='{n}'"),
            }),
            (None, Some(n)) => Ok(Self {
                topic_id: topic_id_from_int(n),
                label: format!("on-chain-id={n}"),
            }),
            // clap's `conflicts_with` + `required_unless_present` catches these,
            // but keep a defensive arm in case the attributes drift.
            (Some(_), Some(_)) => {
                Err(anyhow::anyhow!("specify either --topic or --topic-id, not both"))
            }
            (None, None) => {
                Err(anyhow::anyhow!("specify either --topic or --topic-id"))
            }
        }
    }

    fn hex(&self) -> String {
        self.topic_id.0.iter().map(|b| format!("{b:02x}")).collect()
    }
}

async fn publish(
    node_addr: &SocketAddr,
    target: TopicTarget,
    payload: &str,
    cred_type_str: &str,
) -> Result<()> {
    // Generate ephemeral signing key (in production, load from keyfile)
    let signing_key = {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).expect("OS RNG failed");
        SecretKey::from(bytes)
    };
    let public_key = signing_key.public_key();
    let key_bytes = Bytes::copy_from_slice(public_key.as_ref());

    let cred_type = match cred_type_str {
        "pool-kes" => CredentialType::PoolKes,
        "drep" => CredentialType::DRepCredential,
        "authority" => CredentialType::AuthorityKey,
        _ => CredentialType::Ed25519,
    };
    let cred = match cred_type {
        CredentialType::PoolKes => PublisherCredential::pool_kes(key_bytes, None),
        CredentialType::DRepCredential => PublisherCredential::drep(key_bytes),
        CredentialType::AuthorityKey => PublisherCredential::authority(key_bytes, None),
        CredentialType::Ed25519 => PublisherCredential::ed25519(key_bytes),
    };

    let msg = Message {
        topic_id: target.topic_id.clone(),
        sequence_nr: 0,
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as u64,
        publisher_id: PublisherId(cred),
        signature: Bytes::new(), // will be filled below
        payload: Bytes::from(payload.to_string()),
        metadata: BTreeMap::new(),
    };

    // Sign
    let signable = msg.signable_bytes();
    let signature = signing_key.sign(&signable);
    let msg = Message {
        signature: Bytes::copy_from_slice(signature.as_ref()),
        ..msg
    };

    // Encode and send
    let codec = CborCodec;
    let data = codec.encode(&msg)?;

    // Connect, then run a PUBLISH bi exchange so the node returns an ack.
    let bind_addr: SocketAddr = "127.0.0.1:0".parse()?;
    let mut ephemeral_seed = [0u8; 32];
    getrandom::fill(&mut ephemeral_seed).expect("OS RNG failed");
    let transport = QuicTransport::new(bind_addr, &ephemeral_seed).await?;

    let node_id = transport
        .connect_bootstrap(*node_addr)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to {node_addr}: {e}"))?;

    let ack_bytes = transport
        .publish_exchange(&node_id, *node_addr, data)
        .await
        .map_err(|e| anyhow::anyhow!("publish_exchange failed: {e}"))?;
    let ack: PublishAck = ciborium::de::from_reader(&ack_bytes[..])
        .map_err(|e| anyhow::anyhow!("malformed PublishAck from node: {e}"))?;

    match ack {
        PublishAck::Accepted { topic_id, sequence_nr } => {
            let hex: String = topic_id.0.iter().map(|b| format!("{b:02x}")).collect();
            println!(
                "Accepted topic_id={} seq={} ({}): {}",
                hex, sequence_nr, target.label, payload
            );
            Ok(())
        }
        PublishAck::Rejected { reason } => {
            Err(anyhow::anyhow!("Rejected by node ({}): {}", target.label, reason))
        }
    }
}

async fn subscribe(
    node_addr: &SocketAddr,
    target: TopicTarget,
    since_seq: u64,
    limit: u32,
) -> Result<()> {
    let topic_id = target.topic_id.clone();

    // Ephemeral key (subscribers don't sign anything; the cert is only for TLS).
    let bind_addr: SocketAddr = "127.0.0.1:0".parse()?;
    let mut ephemeral_seed = [0u8; 32];
    getrandom::fill(&mut ephemeral_seed).expect("OS RNG failed");
    let transport = QuicTransport::new(bind_addr, &ephemeral_seed).await?;

    // Bootstrap-connect derives the node's real NodeId from its TLS cert.
    let node_id = transport
        .connect_bootstrap(*node_addr)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to {node_addr}: {e}"))?;

    // CBOR-encode the control frame.
    let req = SubscribeRequest { topic_id: topic_id.clone(), since_seq, limit };
    let mut control = Vec::new();
    ciborium::ser::into_writer(&req, &mut control)?;

    println!(
        "Subscribed topic_id={} ({}) on {} (since_seq={}, limit={}). Ctrl-C to exit.",
        target.hex(),
        target.label,
        node_addr,
        since_seq,
        limit
    );

    let mut rx = transport
        .subscribe_stream(&node_id, *node_addr, control)
        .await
        .map_err(|e| anyhow::anyhow!("subscribe_stream failed: {e}"))?;

    let codec = CborCodec;
    while let Some(frame) = rx.recv().await {
        match codec.decode(&frame) {
            Ok(msg) => print_message(&msg),
            Err(e) => eprintln!("(decode error) {e}"),
        }
    }

    println!("(stream closed by node)");
    Ok(())
}

fn print_message(m: &Message) {
    let payload = std::str::from_utf8(&m.payload)
        .map(|s| s.to_string())
        .unwrap_or_else(|_| format!("<binary {} bytes>", m.payload.len()));
    println!(
        "[{}] {} seq={} ts={}: {}",
        m.topic_id, m.publisher_id, m.sequence_nr, m.timestamp_ms, payload
    );
}

fn topic_id_from_name(name: &str) -> TopicId {
    use pallas_crypto::hash::Hasher;
    let hash = Hasher::<256>::hash(name.as_bytes());
    let mut id = [0u8; 32];
    id.copy_from_slice(hash.as_ref());
    TopicId(id)
}

/// Encode an on-chain integer topic id as a 32-byte TopicId.
/// Mirrors `pallas_chain::on_chain_int_to_topic_id`: BE u64 in bytes 0..8, zeros in 8..32.
fn topic_id_from_int(n: u64) -> TopicId {
    let mut id = [0u8; 32];
    id[..8].copy_from_slice(&n.to_be_bytes());
    TopicId(id)
}

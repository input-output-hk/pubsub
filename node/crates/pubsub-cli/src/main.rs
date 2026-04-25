use std::collections::BTreeMap;
use std::net::SocketAddr;

use anyhow::Result;
use bytes::Bytes;
use clap::{Parser, Subcommand};
use pallas_crypto::key::ed25519::SecretKey;

use pubsub_types::message::{
    CredentialType, Message, PublisherCredential, PublisherId, SubscribeRequest, TopicId,
};

use pubsub_network::codec::CborCodec;
use pubsub_network::transport::QuicTransport;
use pubsub_types::traits::{Codec, SubscribeTransport, Transport};

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
    /// Publish a message to a topic
    Publish {
        /// Topic name (used to derive TopicId via Blake2b-256 unless `--topic-id` is set).
        #[arg(short, long)]
        topic: String,

        /// On-chain topic id (u64).  Overrides `--topic` hashing — required to
        /// reach a topic registered on-chain, where TopicId is an int encoded
        /// as a 32-byte buffer with the BE u64 in bytes 0..8 and zero padding.
        #[arg(long)]
        topic_id: Option<u64>,

        /// Message payload (text)
        #[arg(short, long)]
        message: String,

        /// Credential type tag: ed25519 | pool-kes | drep | authority
        #[arg(long, default_value = "ed25519")]
        credential_type: String,
    },

    /// Subscribe to a topic and print received messages
    Subscribe {
        /// Topic name (used to derive TopicId via Blake2b-256 unless `--topic-id` is set).
        #[arg(short, long)]
        topic: String,

        /// On-chain topic id (u64).  See `publish --topic-id`.
        #[arg(long)]
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
            publish(&cli.node, &topic, topic_id, &message, &credential_type).await?;
        }
        Commands::Subscribe { topic, topic_id, since_seq, limit } => {
            subscribe(&cli.node, &topic, topic_id, since_seq, limit).await?;
        }
        Commands::Status => {
            println!("Status check not yet implemented (needs gRPC API)");
        }
    }

    Ok(())
}

async fn publish(
    node_addr: &SocketAddr,
    topic_name: &str,
    topic_id_override: Option<u64>,
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

    let topic_id = match topic_id_override {
        Some(n) => topic_id_from_int(n),
        None => topic_id_from_name(topic_name),
    };

    let msg = Message {
        topic_id,
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

    // Connect to node and send
    let bind_addr: SocketAddr = "127.0.0.1:0".parse()?;
    let mut ephemeral_seed = [0u8; 32];
    getrandom::fill(&mut ephemeral_seed).expect("OS RNG failed");
    let transport = QuicTransport::new(bind_addr, &ephemeral_seed).await?;

    let node_info = pubsub_types::node::NodeInfo {
        node_id: pubsub_types::node::NodeId([0; 32]),
        addr: *node_addr,
        public_key: vec![],
        subscribed_topics: vec![],
    };
    transport.connect(&node_info).await?;
    transport
        .send(&pubsub_types::node::NodeId([0; 32]), &data)
        .await?;

    println!(
        "Published to topic '{}': {}",
        topic_name,
        payload
    );

    Ok(())
}

async fn subscribe(
    node_addr: &SocketAddr,
    topic_name: &str,
    topic_id_override: Option<u64>,
    since_seq: u64,
    limit: u32,
) -> Result<()> {
    let topic_id = match topic_id_override {
        Some(n) => topic_id_from_int(n),
        None => topic_id_from_name(topic_name),
    };

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
        "Subscribed to '{}' on {} (since_seq={}, limit={}). Ctrl-C to exit.",
        topic_name, node_addr, since_seq, limit
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

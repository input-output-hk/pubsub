use std::collections::BTreeMap;
use std::net::SocketAddr;

use anyhow::Result;
use bytes::Bytes;
use clap::{Parser, Subcommand};
use pallas_crypto::key::ed25519::SecretKey;

use pubsub_types::message::{CredentialType, Message, PublisherCredential, PublisherId, TopicId};

use pubsub_network::codec::CborCodec;
use pubsub_network::transport::QuicTransport;
use pubsub_types::traits::{Codec, Transport};

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
        /// Topic name
        #[arg(short, long)]
        topic: String,

        /// Message payload (text)
        #[arg(short, long)]
        message: String,

        /// Credential type tag: ed25519 | pool-kes | drep | authority
        #[arg(long, default_value = "ed25519")]
        credential_type: String,
    },

    /// Subscribe to a topic and print received messages
    Subscribe {
        /// Topic name
        #[arg(short, long)]
        topic: String,
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
        Commands::Publish { topic, message, credential_type } => {
            publish(&cli.node, &topic, &message, &credential_type).await?;
        }
        Commands::Subscribe { topic } => {
            subscribe(&cli.node, &topic).await?;
        }
        Commands::Status => {
            println!("Status check not yet implemented (needs gRPC API)");
        }
    }

    Ok(())
}

async fn publish(node_addr: &SocketAddr, topic_name: &str, payload: &str, cred_type_str: &str) -> Result<()> {
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

    let topic_id = topic_id_from_name(topic_name);

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

async fn subscribe(_node_addr: &SocketAddr, topic_name: &str) -> Result<()> {
    println!("Subscribing to topic '{}'...", topic_name);
    println!("(Full subscription requires gRPC streaming API — not yet implemented)");
    println!("For now, run pubsub-node with --topics {} to receive messages", topic_name);
    Ok(())
}

fn topic_id_from_name(name: &str) -> TopicId {
    use pallas_crypto::hash::Hasher;
    let hash = Hasher::<256>::hash(name.as_bytes());
    let mut id = [0u8; 32];
    id.copy_from_slice(hash.as_ref());
    TopicId(id)
}

use aiken/primitive/bytearray
use cardano/assets.{AssetName}

pub type RegistryHeadDatum {
  counter: Int,
  epoch: Int,
}

pub type TopicDatum {
  topic_id: Int,
  name: ByteArray,
  owners: List<ByteArray>,
  admins: List<ByteArray>,
  replication_factor: Int,
  retention_period: Int,
  alive: Bool,
  published_at_epoch: Int,
}

pub type PublisherVaultDatum {
  topic_id: Int,
  publisher: ByteArray,
}

pub type RegistryMintAction {
  BootstrapRegistry
  MintTopic
  MintPublisher { topic_id: Int, publisher: ByteArray }
  BurnPublisher { topic_id: Int, publisher: ByteArray }
}

pub type RegistryHeadAction {
  CreateTopic {
    name: ByteArray,
    replication_factor: Int,
    retention_period: Int,
  }
  TickEpoch
}

pub type TopicAction {
  DeleteTopic
  AddOwner { new_owner: ByteArray }
  RemoveOwner { old_owner: ByteArray }
  AddAdmin { new_admin: ByteArray }
  RemoveAdmin { old_admin: ByteArray }
  SetReplicationFactor { r: Int }
  SetRetentionPeriod { t: Int }
}

pub type PublisherVaultAction {
  RemovePublisher
}

pub const registry_head_token_name: AssetName = "registry_head"

/// Encode a topic ID as "t" + 4-byte big-endian ID.
pub fn topic_token_name(topic_id: Int) -> AssetName {
  bytearray.push(bytearray.from_int_big_endian(topic_id, 4), 0x74)
}

/// Encode a publisher role token: "p" + 4-byte topic_id + 28-byte pkh.
pub fn publisher_token_name(topic_id: Int, pkh: ByteArray) -> AssetName {
  let topic_bytes = bytearray.from_int_big_endian(topic_id, 4)
  bytearray.push(bytearray.concat(topic_bytes, pkh), 0x70)
}

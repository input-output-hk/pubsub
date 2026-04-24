use node_registry/types.{NodeEntry, NodeRegistryDatum}

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

pub const node1_id: ByteArray =
  #"1100000000000000000000000000000000000000000000000000000000000011"

pub const node2_id: ByteArray =
  #"2200000000000000000000000000000000000000000000000000000000000022"

pub const node3_id: ByteArray =
  #"3300000000000000000000000000000000000000000000000000000000000033"

pub const alice_key: ByteArray =
  #"aa000000000000000000000000000000000000000000000000000001"

pub const bob_key: ByteArray =
  #"bb000000000000000000000000000000000000000000000000000002"

pub fn empty_registry() -> NodeRegistryDatum {
  NodeRegistryDatum { nodes: [], min_deposit_lovelace: 2_000_000, epoch: 0 }
}

pub fn sample_node_entry() -> NodeEntry {
  NodeEntry {
    node_id: node1_id,
    addr: "127.0.0.1:9001",
    stake_key: alice_key,
    registered_at_epoch: 0,
  }
}

pub fn registry_with_node1() -> NodeRegistryDatum {
  NodeRegistryDatum {
    nodes: [sample_node_entry()],
    min_deposit_lovelace: 2_000_000,
    epoch: 0,
  }
}

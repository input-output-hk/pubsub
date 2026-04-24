use cardano/assets.{AssetName}

// ---------------------------------------------------------------------------
// NodeEntry — one registered relay node
// ---------------------------------------------------------------------------

pub type NodeEntry {
  // 32-byte BLAKE2b-256 of the node's SocketAddr string ("host:port")
  node_id: ByteArray,
  // "host:port" UTF-8 encoded
  addr: ByteArray,
  // Ed25519 verification key hash (28 bytes) — identifies the registering key
  stake_key: ByteArray,
  // registry epoch at registration time (informational)
  registered_at_epoch: Int,
}

// ---------------------------------------------------------------------------
// Datum — full registry state stored in a single state-thread UTxO
// ---------------------------------------------------------------------------

pub type NodeRegistryDatum {
  nodes: List<NodeEntry>,
  // minimum lovelace deposit each node must lock (Sybil resistance)
  min_deposit_lovelace: Int,
  epoch: Int,
}

// ---------------------------------------------------------------------------
// Redeemer — actions that can be performed on the registry
// ---------------------------------------------------------------------------

pub type NodeRegistryAction {
  // Register a new relay node.  The tx must be signed by stake_key and
  // lock at least min_deposit_lovelace at the registry address.
  RegisterNode { node_id: ByteArray, addr: ByteArray }
  // Deregister an existing node and reclaim the deposit.  The tx must be
  // signed by the key that originally registered the node.
  DeregisterNode { node_id: ByteArray }
  // Advance the epoch counter stored in the datum.
  TickEpoch
}

// ---------------------------------------------------------------------------
// Minting redeemer — used to bootstrap the initial registry UTxO
// ---------------------------------------------------------------------------

pub type NodeRegistryMintAction {
  // Consume a one-time UTxO to produce exactly one registry-head NFT.
  BootstrapNodeRegistry
}

// ---------------------------------------------------------------------------
// Token-name helpers
// ---------------------------------------------------------------------------

pub const registry_head_token_name: AssetName = "node_registry_head"

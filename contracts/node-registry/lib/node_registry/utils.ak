use aiken/collection/list
use node_registry/types.{NodeEntry, NodeRegistryDatum}

// ---------------------------------------------------------------------------
// Registry helpers
// ---------------------------------------------------------------------------

/// Find a node entry by its node_id.
pub fn find_node(
  nodes: List<NodeEntry>,
  node_id: ByteArray,
) -> Option<NodeEntry> {
  list.find(nodes, fn(n) { n.node_id == node_id })
}

/// Check whether a node_id is already registered.
pub fn is_registered(nodes: List<NodeEntry>, node_id: ByteArray) -> Bool {
  list.any(nodes, fn(n) { n.node_id == node_id })
}

/// Remove a node by node_id.  Returns None if not found.
pub fn remove_node(
  nodes: List<NodeEntry>,
  node_id: ByteArray,
) -> Option<List<NodeEntry>> {
  when nodes is {
    [] -> None
    [head, ..tail] ->
      if head.node_id == node_id {
        Some(tail)
      } else {
        when remove_node(tail, node_id) is {
          None -> None
          Some(new_tail) -> Some([head, ..new_tail])
        }
      }
  }
}

/// Build the expected post-state datum after a RegisterNode action.
pub fn register_node_result(
  datum: NodeRegistryDatum,
  node_id: ByteArray,
  addr: ByteArray,
  stake_key: ByteArray,
) -> NodeRegistryDatum {
  let entry =
    NodeEntry {
      node_id,
      addr,
      stake_key,
      registered_at_epoch: datum.epoch,
    }
  NodeRegistryDatum { ..datum, nodes: [entry, ..datum.nodes] }
}

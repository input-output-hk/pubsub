use node_registry/test_helpers.{
  alice_key, bob_key, empty_registry, node1_id, node2_id, node3_id,
  registry_with_node1, sample_node_entry,
}
use node_registry/types.{NodeEntry, NodeRegistryDatum}
use node_registry/utils

// ---------------------------------------------------------------------------
// find_node tests
// ---------------------------------------------------------------------------

test find_node_existing() {
  utils.find_node([sample_node_entry()], node1_id) == Some(sample_node_entry())
}

test find_node_missing() {
  utils.find_node([sample_node_entry()], node2_id) == None
}

test find_node_empty_list() {
  utils.find_node([], node1_id) == None
}

// ---------------------------------------------------------------------------
// is_registered tests
// ---------------------------------------------------------------------------

test is_registered_present() {
  utils.is_registered([sample_node_entry()], node1_id)
}

test is_registered_absent() {
  !utils.is_registered([sample_node_entry()], node2_id)
}

test is_registered_empty() {
  !utils.is_registered([], node1_id)
}

// ---------------------------------------------------------------------------
// remove_node tests
// ---------------------------------------------------------------------------

test remove_node_only_entry() {
  utils.remove_node([sample_node_entry()], node1_id) == Some([])
}

test remove_node_first_of_two() {
  let n2 =
    NodeEntry { ..sample_node_entry(), node_id: node2_id }
  utils.remove_node([sample_node_entry(), n2], node1_id) == Some([n2])
}

test remove_node_second_of_two() {
  let n2 =
    NodeEntry { ..sample_node_entry(), node_id: node2_id }
  utils.remove_node([sample_node_entry(), n2], node2_id) == Some([sample_node_entry()])
}

test remove_node_not_found_returns_none() {
  utils.remove_node([sample_node_entry()], node2_id) == None
}

test remove_node_empty_returns_none() {
  utils.remove_node([], node1_id) == None
}

// ---------------------------------------------------------------------------
// register_node_result tests
// ---------------------------------------------------------------------------

test register_node_adds_entry() {
  let reg = empty_registry()
  let result = utils.register_node_result(reg, node1_id, "127.0.0.1:9001", alice_key)
  let expected_entry =
    NodeEntry {
      node_id: node1_id,
      addr: "127.0.0.1:9001",
      stake_key: alice_key,
      registered_at_epoch: 0,
    }
  result.nodes == [expected_entry]
}

test register_node_stamps_epoch() {
  let reg = NodeRegistryDatum { ..empty_registry(), epoch: 3 }
  let result = utils.register_node_result(reg, node2_id, "127.0.0.1:9002", bob_key)
  when result.nodes is {
    [entry] -> entry.registered_at_epoch == 3
    _ -> False
  }
}

test register_node_preserves_min_deposit() {
  let reg = empty_registry()
  let result = utils.register_node_result(reg, node1_id, "127.0.0.1:9001", alice_key)
  result.min_deposit_lovelace == reg.min_deposit_lovelace
}

// ---------------------------------------------------------------------------
// Workflow: register then deregister
// ---------------------------------------------------------------------------

test register_then_deregister() {
  let reg = empty_registry()
  let after_register = utils.register_node_result(reg, node1_id, "127.0.0.1:9001", alice_key)
  expect Some(after_deregister) = utils.remove_node(after_register.nodes, node1_id)
  after_deregister == []
}

// ---------------------------------------------------------------------------
// Multiple nodes
// ---------------------------------------------------------------------------

test register_two_nodes() {
  let reg = empty_registry()
  let reg1 = utils.register_node_result(reg, node1_id, "127.0.0.1:9001", alice_key)
  let reg2 = utils.register_node_result(reg1, node2_id, "127.0.0.1:9002", bob_key)
  utils.is_registered(reg2.nodes, node1_id) && utils.is_registered(reg2.nodes, node2_id)
}

test deregister_one_of_two_nodes() {
  let reg = empty_registry()
  let reg1 = utils.register_node_result(reg, node1_id, "127.0.0.1:9001", alice_key)
  let reg2 = utils.register_node_result(reg1, node2_id, "127.0.0.1:9002", bob_key)
  expect Some(remaining) = utils.remove_node(reg2.nodes, node1_id)
  !utils.is_registered(remaining, node1_id) && utils.is_registered(remaining, node2_id)
}

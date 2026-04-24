use cardano/transaction.{NoDatum, Output}
use topic_registry/test_helpers.{
  alice, bob, carol, nft_value, script_address, test_policy_id,
}
use topic_registry/validation

// --- set_insert tests ---

test set_insert_into_empty() {
  validation.set_insert([], alice) == [alice]
}

test set_insert_maintains_order() {
  validation.set_insert([alice, carol], bob) == [alice, bob, carol]
}

test set_insert_at_beginning() {
  validation.set_insert([bob, carol], alice) == [alice, bob, carol]
}

test set_insert_at_end() {
  validation.set_insert([alice, bob], carol) == [alice, bob, carol]
}

test set_insert_idempotent() {
  validation.set_insert([alice, bob], alice) == [alice, bob]
}

// --- set_remove tests ---

test set_remove_existing() {
  validation.set_remove([alice, bob, carol], bob) == Some([alice, carol])
}

test set_remove_first() {
  validation.set_remove([alice, bob], alice) == Some([bob])
}

test set_remove_last() {
  validation.set_remove([alice, bob], bob) == Some([alice])
}

test set_remove_only_element() {
  validation.set_remove([alice], alice) == Some([])
}

test set_remove_nonexistent_returns_none() {
  validation.set_remove([alice, carol], bob) == None
}

test set_remove_from_empty_returns_none() {
  validation.set_remove([], alice) == None
}

// --- set_contains tests ---

test set_contains_present() {
  validation.set_contains([alice, bob], alice)
}

test set_contains_absent() {
  !validation.set_contains([alice, bob], carol)
}

test set_contains_empty() {
  !validation.set_contains([], alice)
}

// --- find_continuing_output tests ---

test find_continuing_output_single_match() {
  let addr = script_address(test_policy_id)
  let value = nft_value(test_policy_id, "token")
  let output =
    Output { address: addr, value, datum: NoDatum, reference_script: None }
  let result =
    validation.find_continuing_output([output], test_policy_id, "token", addr)
  result == Some(output)
}

test find_continuing_output_no_match() {
  let addr = script_address(test_policy_id)
  validation.find_continuing_output([], test_policy_id, "token", addr) == None
}

test find_continuing_output_wrong_address() {
  let addr = script_address(test_policy_id)
  let other =
    script_address(#"ee000000000000000000000000000000000000000000000000000000")
  let value = nft_value(test_policy_id, "token")
  let output =
    Output { address: other, value, datum: NoDatum, reference_script: None }
  validation.find_continuing_output([output], test_policy_id, "token", addr) == None
}

test find_continuing_output_rejects_duplicates() {
  let addr = script_address(test_policy_id)
  let value = nft_value(test_policy_id, "token")
  let output =
    Output { address: addr, value, datum: NoDatum, reference_script: None }
  validation.find_continuing_output(
    [output, output],
    test_policy_id,
    "token",
    addr,
  ) == None
}

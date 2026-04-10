// Integration tests for topic operations.
// Tests the validation logic that mirrors validators/topic.ak.

use aiken/collection/list
use cardano/transaction.{InlineDatum, Transaction}
use topic_registry/auth
use topic_registry/test_helpers.{
  alice, base_transaction, bob, carol, dave, make_input, make_output, nft_value,
  sample_topic_datum, script_address, test_policy_id, test_tx_id,
}
use topic_registry/types.{
  AddAdmin, AddOwner, DeleteTopic, RemoveAdmin, RemoveOwner,
  SetReplicationFactor, SetRetentionPeriod, TopicAction, TopicDatum,
  topic_token_name,
}
use topic_registry/utils
use topic_registry/validation

/// Simulate the topic validator's spend logic for testing.
fn validate_topic_spend(
  input_datum: TopicDatum,
  redeemer: TopicAction,
  signatories: List<ByteArray>,
  output_datum: TopicDatum,
) -> Bool {
  let token_name = topic_token_name(input_datum.topic_id)
  let addr = script_address(test_policy_id)
  let value = nft_value(test_policy_id, token_name)
  let input = make_input(test_tx_id, 1, addr, value, InlineDatum(input_datum))
  let output = make_output(addr, value, InlineDatum(output_datum))
  let tx =
    Transaction {
      ..base_transaction(),
      inputs: [input],
      outputs: [output],
      extra_signatories: signatories,
    }

  // Re-implement core validation matching validators/topic.ak
  let policy_id = test_policy_id

  input_datum.alive && {
    expect Some(spent_input) =
      validation.find_input_with_nft(tx.inputs, policy_id, token_name)
    expect Some(cont_output) =
      validation.find_continuing_output(
        tx.outputs,
        policy_id,
        token_name,
        spent_input.output.address,
      )
    expect InlineDatum(out_data) = cont_output.datum
    expect out_datum: TopicDatum = out_data

    when redeemer is {
      DeleteTopic ->
        auth.signed_by_owner(input_datum, signatories) && out_datum == utils.tombstone(
          input_datum.topic_id,
        )
      AddOwner { new_owner } ->
        auth.signed_by_owner(input_datum, signatories) && utils.only_owners_changed(
          input_datum,
          out_datum,
        ) && out_datum.owners == validation.set_insert(
          input_datum.owners,
          new_owner,
        )
      RemoveOwner { old_owner } -> {
        let enough = list.length(input_datum.owners) >= 2
        let removed = validation.set_remove(input_datum.owners, old_owner)
        auth.signed_by_owner(input_datum, signatories) && enough && when
          removed
        is {
          Some(expected) ->
            utils.only_owners_changed(input_datum, out_datum) && out_datum.owners == expected
          None -> False
        }
      }
      AddAdmin { new_admin } ->
        auth.signed_by_owner(input_datum, signatories) && utils.only_admins_changed(
          input_datum,
          out_datum,
        ) && out_datum.admins == validation.set_insert(
          input_datum.admins,
          new_admin,
        )
      RemoveAdmin { old_admin } -> {
        let removed = validation.set_remove(input_datum.admins, old_admin)
        auth.signed_by_owner(input_datum, signatories) && when removed is {
          Some(expected) ->
            utils.only_admins_changed(input_datum, out_datum) && out_datum.admins == expected
          None -> False
        }
      }
      SetReplicationFactor { r } ->
        auth.signed_by_owner_or_admin(input_datum, signatories) && r > 0 && utils.only_replication_factor_changed(
          input_datum,
          out_datum,
        ) && out_datum.replication_factor == r
      SetRetentionPeriod { t } ->
        auth.signed_by_owner_or_admin(input_datum, signatories) && t > 0 && utils.only_retention_period_changed(
          input_datum,
          out_datum,
        ) && out_datum.retention_period == t
    }
  }
}

// === DeleteTopic ===

test delete_topic_by_owner() {
  let input = sample_topic_datum()
  validate_topic_spend(
    input,
    DeleteTopic,
    [alice],
    utils.tombstone(input.topic_id),
  )
}

test delete_topic_by_non_owner_fails() {
  let input = sample_topic_datum()
  !validate_topic_spend(
    input,
    DeleteTopic,
    [bob],
    utils.tombstone(input.topic_id),
  )
}

test delete_topic_produces_clean_tombstone() {
  let ts = utils.tombstone(0)
  utils.is_valid_tombstone(ts) && ts.topic_id == 0
}

test delete_topic_preserves_topic_id() {
  let input = TopicDatum { ..sample_topic_datum(), topic_id: 42 }
  let output = utils.tombstone(42)
  validate_topic_spend(input, DeleteTopic, [alice], output) && output.topic_id == 42
}

test delete_dead_topic_fails() {
  let input = TopicDatum { ..sample_topic_datum(), alive: False }
  !validate_topic_spend(
    input,
    DeleteTopic,
    [alice],
    utils.tombstone(input.topic_id),
  )
}

// === AddOwner ===

test add_owner_success() {
  let input = sample_topic_datum()
  let output = TopicDatum { ..input, owners: [alice, bob] }
  validate_topic_spend(input, AddOwner { new_owner: bob }, [alice], output)
}

test add_owner_idempotent() {
  let input = sample_topic_datum()
  validate_topic_spend(input, AddOwner { new_owner: alice }, [alice], input)
}

test add_owner_unauthorized_fails() {
  let input = sample_topic_datum()
  let output = TopicDatum { ..input, owners: [alice, bob] }
  !validate_topic_spend(input, AddOwner { new_owner: bob }, [bob], output)
}

// === RemoveOwner ===

test remove_owner_success() {
  let input = TopicDatum { ..sample_topic_datum(), owners: [alice, bob] }
  let output = TopicDatum { ..input, owners: [alice] }
  validate_topic_spend(input, RemoveOwner { old_owner: bob }, [alice], output)
}

test remove_last_owner_fails() {
  let input = sample_topic_datum()
  let output = TopicDatum { ..input, owners: [] }
  !validate_topic_spend(
    input,
    RemoveOwner { old_owner: alice },
    [alice],
    output,
  )
}

test remove_nonmember_owner_fails() {
  let input = TopicDatum { ..sample_topic_datum(), owners: [alice, bob] }
  !validate_topic_spend(input, RemoveOwner { old_owner: carol }, [alice], input)
}

// === AddAdmin / RemoveAdmin ===

test add_admin_success() {
  let input = sample_topic_datum()
  let output = TopicDatum { ..input, admins: [bob] }
  validate_topic_spend(input, AddAdmin { new_admin: bob }, [alice], output)
}

test add_admin_unauthorized_by_admin_fails() {
  let input = TopicDatum { ..sample_topic_datum(), admins: [bob] }
  let output = TopicDatum { ..input, admins: [bob, carol] }
  !validate_topic_spend(input, AddAdmin { new_admin: carol }, [bob], output)
}

test remove_admin_success() {
  let input = TopicDatum { ..sample_topic_datum(), admins: [bob] }
  let output = TopicDatum { ..input, admins: [] }
  validate_topic_spend(input, RemoveAdmin { old_admin: bob }, [alice], output)
}

test remove_admin_nonmember_fails() {
  let input = TopicDatum { ..sample_topic_datum(), admins: [bob] }
  !validate_topic_spend(input, RemoveAdmin { old_admin: carol }, [alice], input)
}

// === SetReplicationFactor ===

test set_replication_factor_by_owner() {
  let input = sample_topic_datum()
  let output = TopicDatum { ..input, replication_factor: 5 }
  validate_topic_spend(input, SetReplicationFactor { r: 5 }, [alice], output)
}

test set_replication_factor_by_admin() {
  let input = TopicDatum { ..sample_topic_datum(), admins: [bob] }
  let output = TopicDatum { ..input, replication_factor: 5 }
  validate_topic_spend(input, SetReplicationFactor { r: 5 }, [bob], output)
}

test set_replication_factor_zero_fails() {
  let input = sample_topic_datum()
  !validate_topic_spend(
    input,
    SetReplicationFactor { r: 0 },
    [alice],
    TopicDatum { ..input, replication_factor: 0 },
  )
}

test set_replication_factor_unauthorized_fails() {
  let input = sample_topic_datum()
  !validate_topic_spend(
    input,
    SetReplicationFactor { r: 5 },
    [dave],
    TopicDatum { ..input, replication_factor: 5 },
  )
}

// === SetRetentionPeriod ===

test set_retention_period_by_owner() {
  let input = sample_topic_datum()
  let output = TopicDatum { ..input, retention_period: 200 }
  validate_topic_spend(input, SetRetentionPeriod { t: 200 }, [alice], output)
}

test set_retention_period_by_admin() {
  let input = TopicDatum { ..sample_topic_datum(), admins: [bob] }
  let output = TopicDatum { ..input, retention_period: 200 }
  validate_topic_spend(input, SetRetentionPeriod { t: 200 }, [bob], output)
}

test set_retention_period_zero_fails() {
  let input = sample_topic_datum()
  !validate_topic_spend(
    input,
    SetRetentionPeriod { t: 0 },
    [alice],
    TopicDatum { ..input, retention_period: 0 },
  )
}

// === Cross-cutting ===

test operation_on_dead_topic_fails() {
  let input = TopicDatum { ..sample_topic_datum(), alive: False }
  !validate_topic_spend(
    input,
    AddOwner { new_owner: bob },
    [alice],
    TopicDatum { ..input, owners: [alice, bob] },
  )
}

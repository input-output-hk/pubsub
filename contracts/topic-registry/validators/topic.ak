use aiken/collection/list
use cardano/assets.{PolicyId}
use cardano/transaction.{InlineDatum, OutputReference, Transaction}
use topic_registry/auth
use topic_registry/types.{
  AddAdmin, AddOwner, DeleteTopic, RemoveAdmin, RemoveOwner,
  SetReplicationFactor, SetRetentionPeriod, TopicAction, TopicDatum,
  topic_token_name,
}
use topic_registry/utils
use topic_registry/validation

validator topic(registry_policy_id: PolicyId) {
  spend(
    datum: Option<TopicDatum>,
    redeemer: TopicAction,
    _utxo: OutputReference,
    self: Transaction,
  ) {
    expect Some(input_datum) = datum
    expect input_datum.alive

    let token_name = topic_token_name(input_datum.topic_id)
    let signatories = self.extra_signatories

    expect Some(spent_input) =
      validation.find_input_with_nft(
        self.inputs,
        registry_policy_id,
        token_name,
      )

    when redeemer is {
      DeleteTopic -> {
        expect auth.signed_by_owner(input_datum, signatories)
        expect Some(output) =
          validation.find_continuing_output(
            self.outputs,
            registry_policy_id,
            token_name,
            spent_input.output.address,
          )
        expect InlineDatum(out_data) = output.datum
        expect out_datum: TopicDatum = out_data
        out_datum == utils.tombstone(input_datum.topic_id)
      }

      AddOwner { new_owner } -> {
        expect auth.signed_by_owner(input_datum, signatories)
        expect Some(output) =
          validation.find_continuing_output(
            self.outputs,
            registry_policy_id,
            token_name,
            spent_input.output.address,
          )
        expect InlineDatum(out_data) = output.datum
        expect out_datum: TopicDatum = out_data
        expect utils.only_owners_changed(input_datum, out_datum)
        out_datum.owners == validation.set_insert(input_datum.owners, new_owner)
      }

      RemoveOwner { old_owner } -> {
        expect auth.signed_by_owner(input_datum, signatories)
        expect list.length(input_datum.owners) >= 2
        expect Some(expected_owners) =
          validation.set_remove(input_datum.owners, old_owner)
        expect Some(output) =
          validation.find_continuing_output(
            self.outputs,
            registry_policy_id,
            token_name,
            spent_input.output.address,
          )
        expect InlineDatum(out_data) = output.datum
        expect out_datum: TopicDatum = out_data
        expect utils.only_owners_changed(input_datum, out_datum)
        out_datum.owners == expected_owners
      }

      AddAdmin { new_admin } -> {
        expect auth.signed_by_owner(input_datum, signatories)
        expect Some(output) =
          validation.find_continuing_output(
            self.outputs,
            registry_policy_id,
            token_name,
            spent_input.output.address,
          )
        expect InlineDatum(out_data) = output.datum
        expect out_datum: TopicDatum = out_data
        expect utils.only_admins_changed(input_datum, out_datum)
        out_datum.admins == validation.set_insert(input_datum.admins, new_admin)
      }

      RemoveAdmin { old_admin } -> {
        expect auth.signed_by_owner(input_datum, signatories)
        expect Some(expected_admins) =
          validation.set_remove(input_datum.admins, old_admin)
        expect Some(output) =
          validation.find_continuing_output(
            self.outputs,
            registry_policy_id,
            token_name,
            spent_input.output.address,
          )
        expect InlineDatum(out_data) = output.datum
        expect out_datum: TopicDatum = out_data
        expect utils.only_admins_changed(input_datum, out_datum)
        out_datum.admins == expected_admins
      }

      SetReplicationFactor { r } -> {
        expect auth.signed_by_owner_or_admin(input_datum, signatories)
        expect r > 0
        expect Some(output) =
          validation.find_continuing_output(
            self.outputs,
            registry_policy_id,
            token_name,
            spent_input.output.address,
          )
        expect InlineDatum(out_data) = output.datum
        expect out_datum: TopicDatum = out_data
        expect utils.only_replication_factor_changed(input_datum, out_datum)
        out_datum.replication_factor == r
      }

      SetRetentionPeriod { t } -> {
        expect auth.signed_by_owner_or_admin(input_datum, signatories)
        expect t > 0
        expect Some(output) =
          validation.find_continuing_output(
            self.outputs,
            registry_policy_id,
            token_name,
            spent_input.output.address,
          )
        expect InlineDatum(out_data) = output.datum
        expect out_datum: TopicDatum = out_data
        expect utils.only_retention_period_changed(input_datum, out_datum)
        out_datum.retention_period == t
      }
    }
  }

  else(_) {
    fail
  }
}

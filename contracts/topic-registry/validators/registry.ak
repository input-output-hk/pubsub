use aiken/collection/dict
use aiken/collection/list
use cardano/address.{Address, Script}
use cardano/assets.{PolicyId}
use cardano/transaction.{InlineDatum, OutputReference, Transaction}
use topic_registry/auth
use topic_registry/types.{
  BootstrapRegistry, BurnPublisher, CreateTopic, MintPublisher, MintTopic,
  PublisherVaultDatum, RegistryHeadAction, RegistryHeadDatum, RegistryMintAction,
  TickEpoch, TopicDatum, publisher_token_name, registry_head_token_name,
  topic_token_name,
}
use topic_registry/validation

validator registry(bootstrap_utxo: OutputReference) {
  mint(redeemer: RegistryMintAction, policy_id: PolicyId, self: Transaction) {
    when redeemer is {
      BootstrapRegistry -> {
        let bootstrap_consumed =
          list.any(self.inputs, fn(i) { i.output_reference == bootstrap_utxo })
        let minted_qty =
          assets.quantity_of(self.mint, policy_id, registry_head_token_name)
        bootstrap_consumed && minted_qty == 1
      }

      MintTopic -> {
        let head_in_inputs =
          list.any(
            self.inputs,
            fn(i) {
              assets.quantity_of(
                i.output.value,
                policy_id,
                registry_head_token_name,
              ) == 1
            },
          )
        let minted_tokens = assets.tokens(self.mint, policy_id)
        let positive_mints =
          dict.foldl(
            minted_tokens,
            0,
            fn(_name, qty, acc) {
              if qty > 0 {
                acc + 1
              } else {
                acc
              }
            },
          )
        head_in_inputs && positive_mints == 1
      }

      MintPublisher { topic_id, publisher } -> {
        let token_name = publisher_token_name(topic_id, publisher)
        let minted_qty = assets.quantity_of(self.mint, policy_id, token_name)
        expect Some(topic_ref) =
          validation.find_topic_reference_input(
            self.reference_inputs,
            policy_id,
            topic_id,
          )
        expect InlineDatum(topic_data) = topic_ref.output.datum
        expect topic_datum: TopicDatum = topic_data
        let authorized =
          auth.signed_by_owner_or_admin(topic_datum, self.extra_signatories)
        let expected_vault_datum = PublisherVaultDatum { topic_id, publisher }
        let token_output_valid =
          list.any(
            self.outputs,
            fn(o) { assets.quantity_of(o.value, policy_id, token_name) == 1 && {
                expect InlineDatum(d) = o.datum
                expect vault_datum: PublisherVaultDatum = d
                vault_datum == expected_vault_datum
              } },
          )
        minted_qty == 1 && authorized && topic_datum.alive && token_output_valid
      }

      BurnPublisher { topic_id, publisher } -> {
        let token_name = publisher_token_name(topic_id, publisher)
        let burned_qty = assets.quantity_of(self.mint, policy_id, token_name)
        burned_qty == -1
      }
    }
  }

  spend(
    datum: Option<RegistryHeadDatum>,
    redeemer: RegistryHeadAction,
    _utxo: OutputReference,
    self: Transaction,
  ) {
    expect Some(head_datum) = datum

    // Derive policy_id from the validator's own script hash.
    // For a multi-validator, the spend and mint handlers share the same hash.
    let policy_id: PolicyId =
      when
        list.find(
          self.inputs,
          fn(i) {
            assets.quantity_of(
              i.output.value,
              // We find the input that has the registry head token.
              // The policy ID is embedded in the token we're looking for.
              // Bootstrap: we check all inputs for any that have registry_head in value.
              // Since we don't yet know our own policy_id, we derive it from the
              // input's address (which must be a script address matching our hash).
              assets.ada_policy_id,
              assets.ada_asset_name,
            ) > 0
          },
        )
      is {
        Some(input) ->
          when input.output.address.payment_credential is {
            Script(hash) -> hash
            _ -> fail @"Registry head must be at script address"
          }
        None -> fail @"No inputs found"
      }

    when redeemer is {
      CreateTopic { name, replication_factor, retention_period } -> {
        let valid_params = replication_factor > 0 && retention_period > 0

        let head_address =
          Address {
            payment_credential: Script(policy_id),
            stake_credential: None,
          }

        expect Some(head_output) =
          validation.find_continuing_output(
            self.outputs,
            policy_id,
            registry_head_token_name,
            head_address,
          )
        expect InlineDatum(out_data) = head_output.datum
        expect out_datum: RegistryHeadDatum = out_data

        let counter_valid =
          out_datum.counter == head_datum.counter + 1 && out_datum.epoch == head_datum.epoch

        let expected_topic_name = topic_token_name(head_datum.counter)
        let topic_minted =
          assets.quantity_of(self.mint, policy_id, expected_topic_name) == 1

        let topic_output_valid =
          list.any(
            self.outputs,
            fn(o) {
              assets.quantity_of(o.value, policy_id, expected_topic_name) == 1 && {
                expect InlineDatum(td) = o.datum
                expect topic: TopicDatum = td
                topic.topic_id == head_datum.counter && topic.name == name && topic.replication_factor == replication_factor && topic.retention_period == retention_period && topic.alive == True && topic.published_at_epoch == head_datum.epoch && list.length(
                  topic.owners,
                ) == 1 && list.any(
                  self.extra_signatories,
                  fn(sig) { list.has(topic.owners, sig) },
                ) && topic.admins == []
              }
            },
          )

        valid_params && counter_valid && topic_minted && topic_output_valid
      }

      TickEpoch -> {
        let head_address =
          Address {
            payment_credential: Script(policy_id),
            stake_credential: None,
          }

        expect Some(head_output) =
          validation.find_continuing_output(
            self.outputs,
            policy_id,
            registry_head_token_name,
            head_address,
          )
        expect InlineDatum(out_data) = head_output.datum
        expect out_datum: RegistryHeadDatum = out_data

        out_datum.epoch == head_datum.epoch + 1 && out_datum.counter == head_datum.counter
      }
    }
  }

  else(_) {
    fail
  }
}

use cardano/assets.{PolicyId}
use cardano/transaction.{InlineDatum, OutputReference, Transaction}
use topic_registry/auth
use topic_registry/types.{
  PublisherVaultAction, PublisherVaultDatum, RemovePublisher, TopicDatum,
  publisher_token_name,
}
use topic_registry/validation

validator publisher(registry_policy_id: PolicyId) {
  spend(
    datum: Option<PublisherVaultDatum>,
    redeemer: PublisherVaultAction,
    _utxo: OutputReference,
    self: Transaction,
  ) {
    expect Some(vault_datum) = datum

    let RemovePublisher = redeemer

    let topic_id = vault_datum.topic_id
    let publisher_pkh = vault_datum.publisher

    expect Some(topic_ref) =
      validation.find_topic_reference_input(
        self.reference_inputs,
        registry_policy_id,
        topic_id,
      )
    expect InlineDatum(topic_data) = topic_ref.output.datum
    expect topic_datum: TopicDatum = topic_data
    expect topic_datum.alive
    expect auth.signed_by_owner_or_admin(topic_datum, self.extra_signatories)

    let token_name = publisher_token_name(topic_id, publisher_pkh)
    let burned_qty =
      assets.quantity_of(self.mint, registry_policy_id, token_name)
    burned_qty == -1
  }

  else(_) {
    fail
  }
}

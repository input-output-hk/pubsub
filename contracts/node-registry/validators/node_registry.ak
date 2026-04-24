use aiken/collection/list
use cardano/address.{Address, Script}
use cardano/assets.{PolicyId}
use cardano/transaction.{InlineDatum, OutputReference, Transaction}
use node_registry/types.{
  BootstrapNodeRegistry, DeregisterNode, NodeRegistryAction,
  NodeRegistryDatum, NodeRegistryMintAction, RegisterNode, TickEpoch,
  registry_head_token_name,
}
use node_registry/utils

// ---------------------------------------------------------------------------
// node_registry — multi-validator (mint + spend)
//
// Bootstrap:
//   mint(BootstrapNodeRegistry) — consumes bootstrap_utxo, mints exactly one
//   registry-head NFT.  The NFT must be locked at the script address with an
//   initial datum.
//
// Every subsequent operation is a spend that:
//   1. Verifies the tx signer has the right key
//   2. Computes the expected new datum
//   3. Checks the continuing output carries the head NFT with the new datum
// ---------------------------------------------------------------------------

validator node_registry(bootstrap_utxo: OutputReference) {
  mint(
    redeemer: NodeRegistryMintAction,
    policy_id: PolicyId,
    self: Transaction,
  ) {
    let BootstrapNodeRegistry = redeemer
    // Exactly one registry-head NFT minted
    let minted_qty =
      assets.quantity_of(self.mint, policy_id, registry_head_token_name)
    // The one-time bootstrap UTxO must be consumed to prevent replay
    let bootstrap_consumed =
      list.any(self.inputs, fn(i) { i.output_reference == bootstrap_utxo })
    minted_qty == 1 && bootstrap_consumed
  }

  spend(
    datum: Option<NodeRegistryDatum>,
    redeemer: NodeRegistryAction,
    utxo: OutputReference,
    self: Transaction,
  ) {
    expect Some(reg) = datum

    // Derive policy_id from the validator's own script hash.
    // For a multi-validator, the spend and mint handlers share the same hash.
    // We find our own input using the spending UTxO reference (guaranteed to be
    // the registry head) and extract the script hash from its address.
    let policy_id: PolicyId = {
      expect Some(own_input) =
        list.find(self.inputs, fn(i) { i.output_reference == utxo })
      when own_input.output.address.payment_credential is {
        Script(hash) -> hash
        _ -> fail @"Node registry must be at script address"
      }
    }

    let head_address =
      Address { payment_credential: Script(policy_id), stake_credential: None }

    // Find the single continuing output that carries the head NFT.
    expect Some(head_output) =
      list.find(
        self.outputs,
        fn(o) {
          o.address == head_address && assets.quantity_of(
            o.value,
            policy_id,
            registry_head_token_name,
          ) == 1
        },
      )
    expect InlineDatum(out_data) = head_output.datum
    expect out_datum: NodeRegistryDatum = out_data

    when redeemer is {
      RegisterNode { node_id, addr } -> {
        // Prevent duplicate registrations
        expect !utils.is_registered(reg.nodes, node_id)
        // Tx must be signed — the first signatory becomes the stake_key
        expect [stake_key, ..] = self.extra_signatories
        // Compute expected new datum
        let expected =
          utils.register_node_result(reg, node_id, addr, stake_key)
        // Deposit check: the output must lock at least min_deposit_lovelace
        let deposit_ok =
          assets.lovelace_of(head_output.value) >= reg.min_deposit_lovelace
        out_datum == expected && deposit_ok
      }

      DeregisterNode { node_id } -> {
        // Look up the node to find its registered stake_key
        expect Some(node) = utils.find_node(reg.nodes, node_id)
        // Only the registered key may deregister
        expect list.has(self.extra_signatories, node.stake_key)
        // Remove the node from the list
        expect Some(new_nodes) = utils.remove_node(reg.nodes, node_id)
        let expected = NodeRegistryDatum { ..reg, nodes: new_nodes }
        out_datum == expected
      }

      TickEpoch -> {
        let expected =
          NodeRegistryDatum { ..reg, epoch: reg.epoch + 1, nodes: reg.nodes }
        out_datum == expected
      }
    }
  }

  else(_) {
    fail
  }
}

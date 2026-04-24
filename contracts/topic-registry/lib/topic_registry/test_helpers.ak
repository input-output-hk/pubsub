use aiken/collection/dict
use aiken/interval
use cardano/address.{Address, Script}
use cardano/assets.{PolicyId, Value}
use cardano/transaction.{Datum, Input, Output, OutputReference, Transaction}
use topic_registry/types.{RegistryHeadDatum, TopicDatum}

pub const test_tx_id: ByteArray =
  #"0000000000000000000000000000000000000000000000000000000000000000"

pub const test_bootstrap_utxo: OutputReference =
  OutputReference { transaction_id: test_tx_id, output_index: 0 }

pub const alice: ByteArray =
  #"aa000000000000000000000000000000000000000000000000000001"

pub const bob: ByteArray =
  #"bb000000000000000000000000000000000000000000000000000002"

pub const carol: ByteArray =
  #"cc000000000000000000000000000000000000000000000000000003"

pub const dave: ByteArray =
  #"dd000000000000000000000000000000000000000000000000000004"

pub const test_policy_id: PolicyId =
  #"ff000000000000000000000000000000000000000000000000000000"

pub fn script_address(hash: ByteArray) -> Address {
  Address { payment_credential: Script(hash), stake_credential: None }
}

pub fn base_transaction() -> Transaction {
  Transaction {
    inputs: [],
    reference_inputs: [],
    outputs: [],
    fee: 0,
    mint: assets.zero,
    certificates: [],
    withdrawals: [],
    validity_range: interval.everything,
    extra_signatories: [],
    redeemers: [],
    datums: dict.empty,
    id: test_tx_id,
    votes: [],
    proposal_procedures: [],
    current_treasury_amount: None,
    treasury_donation: None,
  }
}

pub fn make_input(
  tx_id: ByteArray,
  index: Int,
  address: Address,
  value: Value,
  datum: Datum,
) -> Input {
  Input {
    output_reference: OutputReference {
      transaction_id: tx_id,
      output_index: index,
    },
    output: Output { address, value, datum, reference_script: None },
  }
}

pub fn make_output(address: Address, value: Value, datum: Datum) -> Output {
  Output { address, value, datum, reference_script: None }
}

pub fn nft_value(policy_id: PolicyId, asset_name: ByteArray) -> Value {
  assets.from_lovelace(2_000_000)
    |> assets.add(policy_id, asset_name, 1)
}

pub fn sample_topic_datum() -> TopicDatum {
  TopicDatum {
    topic_id: 0,
    name: "test-topic",
    owners: [alice],
    admins: [],
    replication_factor: 3,
    retention_period: 100,
    alive: True,
    published_at_epoch: 0,
  }
}

pub fn sample_head_datum() -> RegistryHeadDatum {
  RegistryHeadDatum { counter: 0, epoch: 0 }
}

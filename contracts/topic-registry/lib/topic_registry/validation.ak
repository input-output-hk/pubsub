use aiken/collection/list
use aiken/primitive/bytearray
use cardano/address.{Address}
use cardano/assets.{AssetName, PolicyId}
use cardano/transaction.{Input, Output}
use topic_registry/types

/// Insert an element into a sorted list (set union with single element).
/// Idempotent: if already present, returns the list unchanged.
pub fn set_insert(xs: List<ByteArray>, elem: ByteArray) -> List<ByteArray> {
  when xs is {
    [] -> [elem]
    [head, ..tail] ->
      when bytearray.compare(elem, head) is {
        Less -> [elem, ..xs]
        Equal -> xs
        Greater -> [head, ..set_insert(tail, elem)]
      }
  }
}

/// Remove an element from a sorted list.
/// Returns None if the element is not found.
pub fn set_remove(
  xs: List<ByteArray>,
  elem: ByteArray,
) -> Option<List<ByteArray>> {
  when xs is {
    [] -> None
    [head, ..tail] ->
      when bytearray.compare(elem, head) is {
        Equal -> Some(tail)
        Less -> None
        Greater ->
          when set_remove(tail, elem) is {
            None -> None
            Some(new_tail) -> Some([head, ..new_tail])
          }
      }
  }
}

/// Check if an element is in a list.
pub fn set_contains(xs: List<ByteArray>, elem: ByteArray) -> Bool {
  list.has(xs, elem)
}

/// Find the single output that contains the given NFT at the given address.
pub fn find_continuing_output(
  outputs: List<Output>,
  policy_id: PolicyId,
  asset_name: AssetName,
  input_address: Address,
) -> Option<Output> {
  let matching =
    list.filter(
      outputs,
      fn(o) {
        o.address == input_address && assets.quantity_of(
          o.value,
          policy_id,
          asset_name,
        ) == 1
      },
    )
  when matching is {
    [single] -> Some(single)
    _ -> None
  }
}

/// Find the input that contains the given NFT.
pub fn find_input_with_nft(
  inputs: List<Input>,
  policy_id: PolicyId,
  asset_name: AssetName,
) -> Option<Input> {
  list.find(
    inputs,
    fn(i) { assets.quantity_of(i.output.value, policy_id, asset_name) == 1 },
  )
}

/// Find a reference input containing a topic NFT for the given topic_id.
pub fn find_topic_reference_input(
  reference_inputs: List<Input>,
  policy_id: PolicyId,
  topic_id: Int,
) -> Option<Input> {
  let token_name = types.topic_token_name(topic_id)
  list.find(
    reference_inputs,
    fn(i) { assets.quantity_of(i.output.value, policy_id, token_name) == 1 },
  )
}

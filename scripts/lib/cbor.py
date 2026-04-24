#!/usr/bin/env python3
"""
Minimal CBOR encoder for Cardano Plutus data items.
No external dependencies — only stdlib.

Used by deploy.sh to encode parameters and datums for plutus.json application
and cardano-cli transaction building.
"""
import struct
import sys


def _cbor_uint(n: int) -> bytes:
    """Encode a non-negative integer as a CBOR unsigned int."""
    if n < 0:
        raise ValueError(f"Expected non-negative int, got {n}")
    if n <= 23:
        return bytes([n])
    if n <= 0xFF:
        return bytes([0x18, n])
    if n <= 0xFFFF:
        return struct.pack(">BH", 0x19, n)
    if n <= 0xFFFFFFFF:
        return struct.pack(">BI", 0x1a, n)
    return struct.pack(">BQ", 0x1b, n)


def _cbor_bytes(data: bytes) -> bytes:
    """Encode bytes as a CBOR definite bytestring."""
    n = len(data)
    if n <= 23:
        return bytes([0x40 | n]) + data
    if n <= 0xFF:
        return bytes([0x58, n]) + data
    if n <= 0xFFFF:
        return struct.pack(">BH", 0x59, n) + data
    raise ValueError(f"Bytestring too long: {n}")


def _cbor_array(items: list[bytes]) -> bytes:
    """Encode a definite-length CBOR array of pre-encoded items."""
    n = len(items)
    if n <= 23:
        header = bytes([0x80 | n])
    elif n <= 0xFF:
        header = bytes([0x98, n])
    else:
        raise ValueError(f"Array too large: {n}")
    return header + b"".join(items)


def _cbor_constr(index: int, fields: list[bytes]) -> bytes:
    """Encode a Plutus Constr (alternative constructor) as CBOR tagged array.

    Constr indices 0..6 map to CBOR tags 121..127.
    Indices >= 7 use tag 102 with an explicit alternative integer prefix.
    """
    if 0 <= index <= 6:
        tag = 121 + index
        # CBOR tag encoding: major type 6, then tag number
        if tag <= 23:
            tag_header = bytes([0xC0 | tag])
        elif tag <= 0xFF:
            tag_header = bytes([0xD8, tag])
        else:
            tag_header = struct.pack(">BH", 0xD9, tag)
        return tag_header + _cbor_array(fields)
    else:
        # Tag 102: indefinite-width alternative
        tag_header = bytes([0xD8, 102])
        alt_int = _cbor_uint(index)
        return tag_header + _cbor_array([alt_int] + fields)


# ---------------------------------------------------------------------------
# Plutus parameter encoders
# ---------------------------------------------------------------------------

def encode_output_reference(tx_hash_hex: str, output_index: int) -> str:
    """Encode an OutputReference as CBOR hex for `aiken blueprint apply`.

    OutputReference = Constr 0 { transaction_id: ByteArray, output_index: Int }
    """
    tx_hash = bytes.fromhex(tx_hash_hex)
    if len(tx_hash) != 32:
        raise ValueError(f"tx_hash must be 32 bytes, got {len(tx_hash)}")
    return _cbor_constr(0, [_cbor_bytes(tx_hash), _cbor_uint(output_index)]).hex()


def encode_policy_id(policy_id_hex: str) -> str:
    """Encode a PolicyId (28-byte script hash) as CBOR hex for `aiken blueprint apply`.

    PolicyId is a raw ByteArray in Aiken — no constructor wrapper.
    """
    policy_id = bytes.fromhex(policy_id_hex)
    if len(policy_id) != 28:
        raise ValueError(f"PolicyId must be 28 bytes, got {len(policy_id)}")
    return _cbor_bytes(policy_id).hex()


# ---------------------------------------------------------------------------
# Datum encoders (for cardano-cli --tx-out-inline-datum-cbor-file)
# ---------------------------------------------------------------------------

def encode_registry_head_datum(counter: int = 0, epoch: int = 0) -> str:
    """RegistryHeadDatum = Constr 0 [Int(counter), Int(epoch)]"""
    return _cbor_constr(0, [_cbor_uint(counter), _cbor_uint(epoch)]).hex()


def encode_node_registry_datum(
    nodes: list = None,
    min_deposit_lovelace: int = 2_000_000,
    epoch: int = 0,
) -> str:
    """NodeRegistryDatum = Constr 0 [List([NodeEntry...]), Int, Int]

    For bootstrap, nodes is always [].
    """
    if nodes is None:
        nodes = []
    if nodes:
        raise NotImplementedError("Non-empty node list encoding not needed for bootstrap")
    empty_list = _cbor_array([])
    return _cbor_constr(
        0, [empty_list, _cbor_uint(min_deposit_lovelace), _cbor_uint(epoch)]
    ).hex()


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

COMMANDS = {
    "output-ref": {
        "help": "<tx_hash> <output_index>",
        "fn": lambda args: encode_output_reference(args[0], int(args[1])),
    },
    "policy-id": {
        "help": "<policy_id_hex_56_chars>",
        "fn": lambda args: encode_policy_id(args[0]),
    },
    "registry-head-datum": {
        "help": "[counter=0] [epoch=0]",
        "fn": lambda args: encode_registry_head_datum(
            int(args[0]) if len(args) > 0 else 0,
            int(args[1]) if len(args) > 1 else 0,
        ),
    },
    "node-registry-datum": {
        "help": "[min_deposit_lovelace=2000000] [epoch=0]",
        "fn": lambda args: encode_node_registry_datum(
            min_deposit_lovelace=int(args[0]) if len(args) > 0 else 2_000_000,
            epoch=int(args[1]) if len(args) > 1 else 0,
        ),
    },
}

if __name__ == "__main__":
    if len(sys.argv) < 2 or sys.argv[1] not in COMMANDS:
        print("Usage: cbor.py <command> [args...]")
        print()
        print("Commands:")
        for name, meta in COMMANDS.items():
            print(f"  {name} {meta['help']}")
        sys.exit(1)

    cmd = sys.argv[1]
    args = sys.argv[2:]
    try:
        result = COMMANDS[cmd]["fn"](args)
        print(result)
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)

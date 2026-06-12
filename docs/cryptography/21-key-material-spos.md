# Key material analysis: Cardano credential reuse

This document addresses the questions in [Issue 21](https://github.com/input-output-hk/pubsub/issues/21#issue-4262084651)
of the PubSub repository. For self-containedness, we reproduce the description 
and goals (deliverables) next.

## Description

> Analyse whether existing SPO key material (KES keys, VRF keys, cold/operational  keys, stake credentials) can be safely reused for PubSub message authentication. This is the base use case and can proceed independently of product direction.

> Once Dana confirms the MVP use case(s), extend the analysis to the relevant credential types (governance DRep/CC keys, wallet stake keys, DeFi minting  policy keys, Identus did:prism credentials).

## Deliverables

> - SPO credential analysis: what's safe to reuse, what's not, and why
> - Recommendation: reuse, derive, or separate keys for SPO use case
> - (After product direction) Extended analysis for additional credential types if applicable

## SPO Credential Analysis

As overviewed in the [Identity Anchoring](https://github.com/input-output-hk/pubsub/blob/doc-keys/docs/cryptography/identity-anchoring.md#31-spo-credentials)
document, SPOs have currently three types of keys in the Cardano ecosystem:

- Cold key pair.
- VRF key pair. 
- KES key pair.

The cold keys are Ed25519 key pairs, and VRF keys also use Curve25519 and while
they are not really used for signing, can be used to authenticate the SPO. The 
KES key pair is more complex, although also builds on Ed25519 keys. The cold key 
pair is registered on chain by specifying the verification key (`cold_vk`) hash 
(`blake2b_224(cold_vk)`) in the pool registration certificate. The VRF key is 
similarly registered via `vrf_keyhash`. See next:


```
stake_pool_registration = (3, pool_params)

pool_params = ( operator       : pool_keyhash      -- blake2b_224(cold_vk)
              , vrf_keyhash    : vrf_keyhash        -- blake2b_224(vrf_vk)
              , pledge         : coin
              , cost           : coin
              , margin         : unit_interval
              , reward_account : reward_account
              , pool_owners    : set<addr_keyhash>
              , relays         : [* relay]
              , pool_metadata  : pool_metadata / null
              )
```

KES keys are registered via operational certificates (op certs), signed by the
cold key (and thus verifiable under `cold_vk`) and posted on chain every 90 
days.

```
operational_cert =
    ( hot_vkey        : kes_vkey        -- bytes .size 32
    , sequence_number : uint            -- monotonically increasing counter
    , kes_period      : uint            -- KES period at which hot_vkey becomes valid
    , sigma           : signature       -- bytes .size 64
    )
```

In KES, time is divided in intervals, and signatures offer forward secrecy, 
meaning that if the key used at interval `t` is compromised, the signatures
produced at intervals `t' < t` cannot be suspected to be fraudulent. This 
property is not achieved by the cold or VRF keys. However, KES signatures and
keys are larger than their Ed25519 counterparts.

## Existing Mechanisms

The mentioned keys have the following main usage:

- Cold keys are the root of trust, used to sign pool registration, update and
retirement certificates, and operational certificates. These operations are 
expected to be infrequent, and thus the signing key is (usually) stored 
somewhere without Internet access.

- VRFs are used to prove eligibility to be a slot leader, and to contribute
to the epoch nonce. SPO nodes use these keys very frequently.

- KES keys are used to sign blocks. SPOs use these keys whenever they are 
elected to produce a block. A new KES key is produced every 90 days, and 
registered in the operational certificate signed by the cold key.

Next, we review known alternative use cases for these keys.

### CIP-22 and CIP-151

[CIP-22](https://cips.cardano.org/cip/CIP-22) proposes a mechanism to allow web
sites (assumed to have trusted access to a Cardano node) to authenticate SPOs. 
For instance, web sites that show pool information and want to allow SPOs to 
update it. In a nutshell, the SPO sends its `pool_id` and `vrf_key`, the
website checks it matches the one in the pool registration certificate, sends
back a random challenge, and the SPO returns it signed under the VRF key
(that is, derives a nonce from it, and proves it was correctly derived).

### CIP-94

[CIP-94](https://cips.cardano.org/cip/CIP-94) describes a mechanism for polling
SPOs on governance matters. SPOs sign their poll answers with their cold key.

## What's safe to reuse, what's not, and why

All keys are in principle safe to use. However, cold keys are expected to be 
"air gapped". As such, expecting them to be used frequently is not reasonable, 
and KES is an overkill for publishing, as we do not care about forward secrecy 
(see [publishing](./publishing.md)).

This leaves two choices:

1. Use the VRF key.
2. Use a new key.

Option 1 is what CIP-22 and CIP-151 opted for. This option would not require 
identity anchoring, as the VRF key is already bound to the SPO via the pool 
registration certificate. Yet, note that this employs a VRF key for a usage 
other than the originally intended one. At first sight it seems safe as long as 
there is proper domain separation (from CIP-22, CIP-151, and other usages
of the VRF, like lottery check and nonce contribution). However, there does not
seem to exist formal proofs demonstrating that the security properties of a VRF
(typically: uniqueness, pseudorandomness, and collision resistance) when used
as a signature scheme (that is, requiring some variant of unforgeability). While
it seems natural to expect some relation, it is also uncertain what type of 
unforgeability we could expect. Finally, the "signatures" produced by the VRF 
(again, note that they are not really a signature) are about 112 bytes (80 bytes
of proof, and 32 bytes of VRF output), about twice the sice of a normal Ed25519
signature (64 bytes).

Option 2 establishes a clearer role separation and natively prevents attacks 
due to sloppy domain separation or unclear unforgeability properties. However, 
it requires binding this new key to the SPO. This could be done by adding into 
the pool registration or the  operational certificates a `pubsub_keyhash` akin 
to the `pool_keyhash` and `vrf_keyhash`, or by having the `pubsub_keyhash` 
signed by either the VRF key or the KES key. In the former case, we would need 
to extend the pool or operational certificate data structures. In the latter, 
this is not needed, but we would again incur in domain separation needs, which 
may be more error prone -- and is one of the points in favor of Option 2 vs 
Option 1. Regarding where to include it: the operational certificate seems the 
best choice, as it allows easier key rotation in case of need (e.g., key loss or
compromise). Additionally, note that there are ongoing conversations to do the 
same with the VRF keys (rotate them via the operational certificate), as part of
the Leios deployment -- hence, this may be a good moment to add a new key type 
in the operational certificates.

## Recommendation

Given the options analyzed above, using a new key, and specifying it in the
operational certificate, seems the most appropriate:

- It prevents attacks due to sloppy domain separation, or unclear unforgeability
properties.
- It anchors a new key type directly to the root of trust for SPOs.
- It can be naturally rotated every 90 days via operational certificates.

As a conventional strongly unforgeable scheme would meet our security needs
(see [publishing](./publishing.md)), the natural target is Ed25519.

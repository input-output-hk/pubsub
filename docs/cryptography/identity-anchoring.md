# Identity Anchoring

This document overviews the identification needs that arise in the different 
layers, which we divide here in [gossiping](gossiping.md) and [publishing](publishing.md), 
independently on their internal structure. Broadly, gossiping refers to the 
components whose main purpose is to create a "densely" connected network among 
the peers --just so that they know about each other--, whereas publishing refers
to the components enabling the information posted by publishers to reach their 
subscribers. The security needs are different in each component. In gossiping, 
Sybil and grinding attacks are a [real concern](gossiping.md), as well as 
forward security; in publishing, standard SUF (Strong Unforgeability) appears 
enough, and Sybil or grinding attacks may be already addressed by a combination 
of mechanisms to avoid them at the gossiping layer, plus the topic registry.

However, both layers may benefit from having a common key management identity 
infrastructure whenever possible, anchoring public keys to somewhat/somehow 
trusted entities. Next, we summarize: threat model for each layer (gossiping and 
publishing), trust anchors existing in Cardano (e.g., SPOs and their op certs, 
DReps and their DRep certs), and the types of entities described in the 
[Synthesis proposal](../staged-design-synthesis.md) for each use case. Then, we
map each of the types of entities to existing Cardano trust anchors that we deem 
appropriate for identity anchoring, or highlight the gap where we find none.

## 2. Threat Models

Next, we summarize the threat models of each layer. For details, we refer to 
the respective documents.

### 2.1 Gossiping Layer

From [gossiping.md](gossiping.md):
- Sybil attacks attacks.
- Grinding attacks (descriptor-position manipulation) in the case of 
descriptor-dependent topologies.
- Forward security (key compromise and retroactive forgery) in the case of
Secure Cyclon.
- Undetected-compromise window (proactive security).

### 2.2 Publishing Layer

From [publishing.md](publishing.md):
- Forgery
- Replay and cross-topic replay
- Equivocation
- Key compromise and recovery

## 3. Existing Trust Anchors in Cardano

We overview the existing key material in Cardano, and how each is bound to 
relevant roles in the ecosystem (dReps, SPOs, stake owners). For each, we 
briefly overview how each credential is created and registered.

### 3.1 SPO Credentials

Each SPO maintains three key pairs [[shelley-ledger-spec], [node-wiki-kes]]:

- **Cold key** (`StakePoolVerificationKey_ed25519`): a long-term Ed25519 key pair
  that never touches the block-producing node. Its hash appears on-chain in the
  pool registration certificate.
- **VRF key** (`VrfVerificationKey_PraosVRF`): an
  `ECVRF-ED25519-SHA512-Elligator2` key pair used for leader election. The VRF
  verification key is registered on-chain in the pool registration certificate
  alongside the cold key hash.
- **KES key** (`KesVerificationKey_ed25519_kes_2^6`): a forward-secure key pair
  using the iterated sum construction (Sum6KES, 2^6 = 64 evolutions
  [[MMM01]]). The underlying leaf signature scheme is Ed25519. The KES key
  evolves roughly every 90 days on mainnet.

**Pool registration certificate.** Before a node can produce blocks, the SPO
must register on-chain by submitting a transaction carrying a
`stake_pool_registration` certificate (cert type 3, [[shelley-ledger-spec],
Appendix A]):

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

The security-critical fields are `operator` (`blake2b_224(cold_vk)`) and
`vrf_keyhash` (`blake2b_224(vrf_vk)`): they bind the pool's on-chain identity
to its cold and VRF key material. The remaining fields are economic and routing
parameters.

The registering transaction must carry a cold-key Ed25519 witness; the ledger
verifies it against `operator`. On acceptance the ledger adds the pool to its
`pParams` map (keyed by `pool_keyhash`) and initialises the op-cert counter for
that pool to 0. Pool re-registration (same `pool_keyhash`, updated fields)
follows the same procedure. Pool retirement is registered via a
`stake_pool_deregistration` certificate (cert type 4):
`(4, pool_keyhash, epoch)`, again witnessed by the cold key; the ledger
schedules retirement at the given epoch.

The **operational certificate** (op cert) is the binding artifact. Its structure
is [[shelley-ledger-spec], Section 12.8]:

```
OCert = {
  vkhot  : KES verification key   -- hot operational key
  n      : N                       -- certificate issue counter
  c0     : KESPeriod               -- KES period when the cert was issued
  σ      : Ed25519 signature       -- cold key signature over (vkhot, n, c0)
}
```

The cold key signs the KES verification key together with a counter `n` and a
KES start period `c0`. Nodes verify the cold-key signature and then use the KES
key to verify the block header body signature at the current evolution step `t =
kesPeriod(slot) − c0`.

The counter is the ledger's anti-rollback mechanism. The ledger stores, per pool,
the last accepted counter value. A presented op cert is valid only if its counter
is at least the stored value (post-Vasil: exactly one greater). An attacker who
compromises a KES key cannot retroactively forge blocks for earlier periods (KES
forward-security), and cannot extend beyond the current cert without the cold key
(counter prevents cert rollback).

The cold key is thus the stable, long-term identity of an SPO: it signs pool
registration, every successive op cert, and pool retirement. The on-chain pool
registration provides the public anchor (cold key hash and VRF key) against
which anyone can verify ownership of an op cert.

**References**: Shelley ledger formal specification [[shelley-ledger-spec]] (Sections
3, 12.8, Appendix A); cardano-node-wiki on KES periods [[node-wiki-kes]]; `kes`
Rust crate [[kes-crate]] (Sum6KES implementation).

### 3.2 DRep Credentials

DRep credentials are defined in CIP-0105 [[cip-0105]] within the governance
framework introduced by CIP-1694 [[cip-1694]].

**Key derivation.** DRep keys are derived from the standard CIP-1852 HD wallet
hierarchy [[cip-1852]] using role index 3:

```
m / 1852' / 1815' / account' / 3 / address_index
```

The key material is a standard Ed25519 key pair. Address index 0 is recommended
(one DRep key per account).

**Credential derivation.** The `drep_credential` is the BLAKE2b-224 hash of the
DRep's Ed25519 verification key (without chain code):

```
drep_credential = blake2b_224(drep_verification_key)
```

This is a key-hash credential (type tag `0` in the Conway CDDL [[conway-cddl]]):

```
drep_credential = [0, blake2b_224(drep_vk)]
```

**Public verification.** The credential is registered on-chain in a
`reg_drep_cert` certificate (Conway era) that records the `drep_credential`, a
deposit, and an optional anchor. Anyone can verify a DRep's identity by computing
`blake2b_224` of the claimed Ed25519 public key and checking it matches the
on-chain credential. Votes cast by the DRep carry an Ed25519 signature witnessed
against the registered credential; the ledger verifies the witness in the standard
way.

**Bech32 encoding.** The verification key hashes are encoded with the `drep_vkh`
prefix (raw) or the `drep` prefix under CIP-0129 [[cip-0129]], which prepends a
header byte encoding the credential type.

**References**: CIP-0105 [[cip-0105]]; CIP-1694 [[cip-1694]]; CIP-1852 [[cip-1852]];
CIP-0129 [[cip-0129]]; Conway ledger formal specification [[conway-ledger-spec]].

### 3.3 CC Credentials

Constitutional Committee (CC) members use a cold/hot key split analogous to the
SPO cold/KES separation but without forward-security requirements [[cip-0105],
[conway-ledger-spec]].

**Key derivation.** CIP-0105 [[cip-0105]] assigns two new role indices within the
CIP-1852 HD hierarchy:

- **CC cold key** (role 4): `m / 1852' / 1815' / account' / 4 / address_index`
- **CC hot key** (role 5): `m / 1852' / 1815' / account' / 5 / address_index`

Both are standard Ed25519 key pairs.

**Credential derivation.** Both credentials follow the same pattern:

```
committee_cold_credential = blake2b_224(cc_cold_verification_key)
committee_hot_credential  = blake2b_224(cc_hot_verification_key)
```

**On-chain lifecycle.** CC membership is established by an `UpdateCommittee`
governance action that lists cold credentials. A member then authorizes their hot
key on-chain via an `auth_committee_hot_cert` certificate:

```
auth_committee_hot_cert = (cold_credential, hot_credential)
```

The ledger's `ccHotKeys` map records the cold → hot credential binding. All votes
are cast using the hot credential; the ledger verifies the corresponding Ed25519
witness. If the hot key is compromised, the cold key issues a new
`auth_committee_hot_cert` replacing it without requiring a governance action.
Resignation is recorded by mapping the cold credential to `Nothing` (via
`ccreghot(cold_credential, None)`).

**Public verification.** Anyone can verify a CC member's hot key by computing
`blake2b_224` of the claimed Ed25519 public key and checking the on-chain
`ccHotKeys` map. The cold credential can similarly be cross-checked against the
current committee set from the governance state.

**Bech32 encoding.** `cc_cold_vkh` and `cc_hot_vkh` prefixes for raw key hashes;
`cc_cold` and `cc_hot` prefixes under CIP-0129 [[cip-0129]].

**References**: CIP-0105 [[cip-0105]]; CIP-1694 [[cip-1694]]; Conway ledger formal
specification [[conway-ledger-spec]].

### 3.4 Staking Key

The staking key is the standard delegation and reward credential for any ADA
holder. Unlike SPO, DRep, and CC credentials — which require an active on-chain
role — a staking key can be registered by any wallet holder, making it a
candidate generic anchor for participants that do not hold an established Cardano
role.

**Key derivation.** Staking keys are derived from the CIP-1852 HD hierarchy
[[cip-1852]] using role index 2:

```
m / 1852' / 1815' / account' / 2 / 0
```

The key material is a BIP32-Ed25519 key pair. Index 0 is recommended; sequential
indexing without gaps is used for multi-staking-key wallets [[cip-0011]].

**Credential derivation.** The `stake_credential` is the BLAKE2b-224 hash of the
staking verification key (without chain code), encoded as a key-hash credential
(type tag `0` in the Conway CDDL [[conway-cddl]]):

```
stake_credential = [0, blake2b_224(stake_vk)]
```

**On-chain lifecycle.** A stake credential is registered with
`account_registration_deposit_cert` (Conway preferred) or
`account_registration_cert` (Shelley legacy), locking a deposit. Once
registered, it can be independently delegated:

- **To an SPO** via `delegation_to_stake_pool_cert = (2, stake_credential,
  pool_keyhash)`: routes stake weight and staking rewards to the chosen pool.
- **To a DRep** via `delegation_to_drep_cert = (9, stake_credential, drep)`:
  routes voting weight to the chosen DRep (key hash, script hash, or
  `abstain`/`no-confidence` sentinels). Introduced by CIP-1694 [[cip-1694]].
- **To both simultaneously** via `delegation_to_stake_pool_and_drep_cert = (10,
  stake_credential, pool_keyhash, drep)`.

All delegation certificates carry an Ed25519 witness from the staking key.

**Public verification.** Anyone can verify a stake credential by computing
`blake2b_224` of the claimed Ed25519 public key and checking it against the
on-chain registered credential. Both staking and voting delegation state are
readable from the ledger.

**Bech32 encoding.** The verification key is encoded with the `stake_vk` prefix;
the key hash with `stake_vkh`; reward addresses use `stake` (mainnet) or
`stake_test` (testnet) [[cip-0011]].

**References**: CIP-0011 [[cip-0011]]; CIP-1694 [[cip-1694]]; CIP-1852
[[cip-1852]]; Conway ledger formal specification [[conway-ledger-spec]]; Conway
CDDL [[conway-cddl]].

## 4. Entity Types and Identity Needs

The use cases in the [Synthesis proposal](../staged-design-synthesis.md) and in 
[section 4.2 of gossiping.md](gossiping.md#trust-anchors-in-the-vertical-approach)
involve the following real-world entity types, each appearing as publishers, 
direct subscriber nodes, or both across different use cases.

### 4.1 IOG (and similar protocol developer teams)

- Role: sole publisher in the IOG → SPOs use case (~1 publisher, ~3000 direct 
subscribers).
- Existing Cardano identity: none that is directly suitable; IOG has no on-chain
credential analogous to an SPO or DRep.
- Identity need (publishing): unforgeability; the small publisher count makes 
registry-listing with off-chain curation sufficient.
- Identity need (gossiping): n/a — IOG publishes but does not run gossip nodes 
in this use case.

### 4.2 SPOs (Stake Pool Operators)

- Role: direct subscriber nodes (~3000) in the IOG → SPOs use case; publishers
(500–800 active) in the SPOs → delegators use case.
- Existing Cardano identity: SPO cold key (pool registration certificate) or
operational certificates (op-certs).
- Identity need (publishing): binding to op-cert or pool registration cert 
provides a natural trust anchor; self-signed binding from pubsub key to cold key
suffices.
- Identity need (gossiping): SPOs are the canonical candidate for vertical trust
anchoring. Cold key certifies the gossip key (opcert-like), providing Sybil 
resistance.

### 4.3 DReps (Delegated Representatives)

- Role: publishers (~150 active) in the DReps → delegators use case.
- Existing Cardano identity: DRep registration certificates, on-chain.
- Identity need (publishing): DRep cert provides a natural binding for the 
pubsub signing key; analogous to the SPO case.
- Identity need (gossiping): DReps are not expected to run gossip nodes
directly; delivery goes through wallet backends (see §4.4).

### 4.4 Wallet Backends

- Role: direct subscriber nodes (10–50) in the DReps → delegators,
SPOs → delegators, and dApps → users use cases; relay infrastructure for 
end-user fanout.
- Existing Cardano identity: none; wallet backends are off-chain commercial or 
community services.
- Identity need (publishing): n/a — wallet backends do not publish.
- Identity need (gossiping): not anchored in on-chain credentials. With ~10–50 
nodes per topic, storing their roots of trust directly in the 
[topic registry](../staged-design-synthesis.md#21-topic-registry-on-chain) is 
feasible (as noted in 
[gossiping.md](gossiping.md#trust-anchors-in-the-vertical-approach)).

### 4.5 dApps

- Role: publishers (10–50 active) in the dApps → users use case.
- Existing Cardano identity: none that is universally available; some dApps have
on-chain governance tokens or script addresses, but there is no standard 
credential.
- Identity need (publishing): registry listing with off-chain curation is the 
only available mechanism ([gap S-13](../aueb-gap-analysis-final.md#s-13--the-topic-registry-provides-coordination-not-identity-trust)).
- Identity need (gossiping): dApps do not run gossip nodes.

Note: dApps may be in the order of 10-50 now (?), but it would be good to plan
for the case in which there are hundreds or thousands of them. In that case,
listing them in the topic registry would not scale, and we may need a separate
layer for handling identity here (with DID-based approaches as stated i
[Synthesis proposal](../staged-design-synthesis.md) being a natural candidate).

## 5. Mapping: Entity Types to Cardano Trust Anchors

| Use case | Entity | Role | Candidate trust anchor | Binding mechanism | Open issues |
|---|---|---|---|---|---|
| IOG → SPOs | IOG | Publisher | Topic registry (off-chain curation) | Self-registration; no on-chain witness | No on-chain IOG credential (Gap S-13) |
| IOG → SPOs | SPOs | Subscriber / gossip node | Pool registration cert (`pool_keyhash = blake2b_224(cold_vk)`) | Op-cert-like: cold-key Ed25519 sig over `(vk_gossip, n, c0)`; counter prevents rollback | - |
| DReps → delegators | DReps | Publisher | `reg_drep_cert` (`drep_credential = blake2b_224(drep_vk)`) | Self-signed binding: `drep_vk` sig over pubsub key; verifier checks `blake2b_224(drep_vk)` against on-chain credential | — |
| DReps → delegators | Wallet backends | Relay / subscriber | Topic registry (direct root-of-trust listing) | Owner-attested listing | No on-chain credential |
| SPOs → delegators | SPOs | Publisher | Pool registration cert (`pool_keyhash = blake2b_224(cold_vk)`) | Self-signed binding: cold-key Ed25519 sig over pubsub key | — |
| SPOs → delegators | Wallet backends | Relay / subscriber | Topic registry (direct root-of-trust listing) | Owner-attested listing | No on-chain credential |
| dApps → users | dApps | Publisher | Topic registry (off-chain curation) | Self-registration; no on-chain witness | No standard dApp credential (Gap S-13); topic-registry listing does not scale beyond ~50 dApps |
| dApps → users | Wallet backends | Relay / subscriber | Topic registry (direct root-of-trust listing) | Owner-attested listing | No on-chain credential |

Vertical trust anchors cover every entity that holds an established Cardano
on-chain role. SPOs are anchored via the pool registration certificate
(`pool_keyhash = blake2b_224(cold_vk)`) in both their gossip-node and publisher
roles: an op-cert-like construction provides Sybil resistance and anti-rollback
for gossiping, and a direct cold-key self-signed binding suffices for publishing.
DReps are anchored via `reg_drep_cert` (`drep_credential = blake2b_224(drep_vk)`)
in their publisher role. In all three cases identity is verifiable against
on-chain state with no additional infrastructure.

Three entity types fall outside this coverage because they hold no Cardano
on-chain credential: IOG (publisher), wallet backends (relay/subscriber across
all use cases), and dApps (publisher). For use cases that require few of these
entities, listing their root public keys directly in the topic registry is 
feasible. Otherwise, they are listed as open questions in Section 6.

## 6. Open Questions

**Entities without vertical trust anchors.** The following roles cannot be
anchored to existing Cardano on-chain credentials and require an alternative
identity layer. A DID-based approach (as outlined in the
[Synthesis proposal](../staged-design-synthesis.md)) is a natural candidate in
each case, since it decouples verifiable identity from Cardano governance roles.

- **IOG as publisher (IOG → SPOs).** IOG holds no on-chain credential analogous
  to an SPO or DRep. Topic-registry listing with off-chain curation is the only
  current fallback (Gap S-13), but it provides no cryptographic binding between
  the publisher identity and the registered key. A DID for IOG would provide a
  stable, externally verifiable anchor.
- **Wallet backends as relay/subscriber (all use cases).** Wallet backends are
  off-chain services with no Cardano on-chain credential. At current scale
  (~10–50 nodes per topic) direct root-of-trust listing in the topic registry is
  operationally feasible, but the listing is owner-attested with no
  cryptographic proof of ownership and does not generalise. A DID per wallet
  backend would enable verifiable identity without requiring an on-chain role.
- **dApps as publishers (dApps → users).** No standard on-chain dApp credential
  exists. Topic-registry listing does not scale beyond ~50 dApps and provides no
  cryptographic binding (Gap S-13). A DID per dApp (tied to its on-chain script
  address or off-chain presence) is the natural candidate, as noted in the
  Synthesis proposal.

**Key derivation.**

- Key derivation for gossip and publication keys: CIP-1852-compatible only if
  Ed25519 suffices for all roles; blocked on the KES decision for gossip keys.
- Whether a shared key derivation path (common HD subtree) can serve both
  gossiping and publishing identity, or whether the two require separate key
  hierarchies.

## References

- [shelley-ledger-spec] Corduan, Vinogradova, Güdemann. *Formal Specification of
  the Cardano Ledger*. IOHK, 2019.
  https://github.com/intersectmbo/cardano-ledger/releases/latest/download/shelley-ledger.pdf
  (Sections 3, 12.8, Appendix A.)

- [conway-ledger-spec] Knispel, DeMeo, Jääger, Tomé Cortiñas. *Formal
  Specification of the Cardano Ledger for the Conway era*. IOHK.
  https://intersectmbo.github.io/formal-ledger-specifications/conway-ledger.pdf

- [cip-0011] Guillemot, Benkort. *Staking key chain for HD wallets*. CIP-0011.
  https://github.com/cardano-foundation/CIPs/blob/master/CIP-0011/README.md

- [cip-0105] Ledger/CIP authors. *Conway era Key Chains for HD Wallets*. CIP-0105.
  https://github.com/cardano-foundation/CIPs/blob/master/CIP-0105/README.md

- [cip-0129] Ledger/CIP authors. *Governance Identifiers*. CIP-0129.
  https://github.com/cardano-foundation/CIPs/blob/master/CIP-0129/README.md

- [cip-1694] Ledger/CIP authors. *A First Step Towards On-Chain Decentralized
  Governance*. CIP-1694.
  https://github.com/cardano-foundation/CIPs/blob/master/CIP-1694/README.md

- [cip-1852] Ledger/CIP authors. *HD (Hierarchy for Deterministic) Wallets for
  Cardano*. CIP-1852.
  https://github.com/cardano-foundation/CIPs/blob/master/CIP-1852/README.md

- [conway-cddl] IntersectMBO. *Conway CDDL*.
  https://github.com/IntersectMBO/cardano-ledger/blob/master/eras/conway/impl/cddl/data/conway.cddl

- [node-wiki-kes] IOHK. *KES period and operational certificates*. cardano-node
  wiki.
  https://github.com/input-output-hk/cardano-node-wiki/blob/main/docs/stake-pool-operations/7_KES_period.md

- [kes-crate] IOHK. *`kes` Rust crate — Sum6KES implementation*.
  https://github.com/input-output-hk/kes

- [MMM01] Malkin, Micciancio, Miner. *Composition and Efficiency Tradeoffs for
  Forward-Secure Digital Signatures*. EUROCRYPT 2001.

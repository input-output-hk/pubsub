# Build vs Buy Decisions

Strategic technology adoption decisions for the Cardano PubSub architecture.

## Decision Matrix

| Component | Decision | Rationale |
|-----------|----------|-----------|
| **Networking** | ADOPT + BUILD | Adopt [IOG Research protocols](research-foundation.md) (SecureCyclon, Vicinity, Hybrid Dissemination), BUILD Ouroboros compatibility adapters |
| **Identity** | ADOPT (Identus) | Strictly integrate Identus for all DID and Credential operations rather than building custom identity solution |
| **Encryption** | BUY (Adopt) | Implement IETF standard MLS (RFC 9420) rather than rolling custom crypto |
| **Database** | BUY | Embedded RocksDB or Sled for local node storage |
| **Verification** | BUILD | Logic to check Cardano L1 state or Midnight proofs is unique to our ecosystem |

---

## Detailed Rationale

### Networking: ADOPT + BUILD

```
┌─────────────────────────────────────────────────┐
│              Networking Stack                    │
├─────────────────────────────────────────────────┤
│  ADOPT: SecureCyclon (peer sampling)            │
│  ADOPT: Vicinity (topic navigation)             │
│  ADOPT: Hybrid Dissemination (Harary + random)  │
│  BUILD: Ouroboros compatibility adapters        │
│  BUILD: Topic Registry smart contract           │
└─────────────────────────────────────────────────┘
```

**Why this approach?**
- IOG Research protocols are purpose-built for Cardano Pub/Sub
- SecureCyclon developed for IOG's eclipse-resistance project (2020-2022)
- SPO adoption requires Ouroboros compatibility
- Harary Graph + random links provide both speed and reliability

---

### Identity: ADOPT (Identus)

!!! success "Strategic Alignment"
    Identus is the Cardano ecosystem's native identity solution. Using it ensures:
    
    - Ecosystem alignment and support
    - Portable reputation across Cardano dApps
    - Verifiable Credentials support out of the box
    - Maintenance burden on Identus team, not Cardano PubSub team

---

### Encryption: BUY (MLS)

!!! warning "Never Roll Your Own Crypto"
    MLS (RFC 9420) is the IETF standard for secure group messaging. It provides:
    
    - Forward secrecy
    - Post-compromise security
    - Efficient key rotation for large groups
    - Formal security proofs

---

### Database: BUY (RocksDB/Sled)

Embedded databases are mature, well-tested, and provide:

- ACID guarantees
- Efficient key-value storage
- Proven performance at scale
- Active maintenance

---

### Verification: BUILD

The logic to verify:

- Cardano L1 UTXO state
- Stake snapshots
- Midnight proofs
- Partner chain state

...is **unique to our ecosystem**. No off-the-shelf solution exists.

This is where we create **technical moat**.

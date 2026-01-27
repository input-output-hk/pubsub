# Build vs Buy Decisions

Strategic technology adoption decisions for the Agora architecture.

## Decision Matrix

| Component | Decision | Rationale |
|-----------|----------|-----------|
| **Networking** | HYBRID | Adopt libp2p components where efficient, BUILD necessary adapters for Ouroboros compatibility, BUILD custom Harary Graph overlay logic |
| **Identity** | ADOPT (Identus) | Strictly integrate Identus for all DID and Credential operations rather than building custom identity solution |
| **Encryption** | BUY (Adopt) | Implement IETF standard MLS (RFC 9420) rather than rolling custom crypto |
| **Database** | BUY | Embedded RocksDB or Sled for local node storage |
| **Verification** | BUILD | Logic to check Cardano L1 state or Midnight proofs is unique to our ecosystem |

---

## Detailed Rationale

### Networking: HYBRID

```
┌─────────────────────────────────────────────────┐
│              Networking Stack                    │
├─────────────────────────────────────────────────┤
│  BUILD: Ouroboros adapters                      │
│  BUILD: Harary Graph overlay                    │
│  ADOPT: libp2p GossipSub                        │
│  ADOPT: libp2p connection management            │
└─────────────────────────────────────────────────┘
```

**Why Hybrid?**
- libp2p provides battle-tested gossip and connection primitives
- But SPO adoption requires Ouroboros compatibility
- Harary Graph overlay is unique to our reliability requirements

---

### Identity: ADOPT (Identus)

!!! success "Strategic Alignment"
    Identus is the Cardano ecosystem's native identity solution. Using it ensures:
    
    - Ecosystem alignment and support
    - Portable reputation across Cardano dApps
    - Verifiable Credentials support out of the box
    - Maintenance burden on Identus team, not Agora team

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

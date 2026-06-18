# Byzantine-Resilient, Brokerless Pub/Sub Protocols

Pubsub protocol inclusion criteria:

1. **Byzantine resilient** — designed with malicious node behavior in mind, not just crashes.
2. **Not broker based** — peer-to-peer / gossip dissemination, no dedicated broker overlay.
3. **Peer-reviewed publication** — appears in a reputable conference or journal.


## Pub/sub and dissemination systems

| Protocol | Byzantine resilient? | Brokerless? | Peer-reviewed? | Verdict |
|---|---|---|---|---|
| **PubSubChk**  | ? | ? | ? | **?** |
| **GossipSub** (Vyzovitis et al.) | Yes — peer scoring, mesh hardening, flood publishing | Yes — libp2p gossip mesh | Yes — PL TechRep 2020; later formal analysis (arXiv/ACM) | **INCLUDED** |
| **BAR Gossip** (Li et al.) | Yes — BAR model, resilient up to ~20% Byzantine | Yes — P2P gossip streaming | Yes — OSDI 2006 | **INCLUDED** |
| **Fireflies** (Johansen, Allavena, van Renesse) | Yes — correct nodes cannot be eclipsed | Yes — gossip mesh + membership | Yes — EuroSys 2006; TOCS 2015 | **INCLUDED** |
| **Drum** (Badishi, Keidar, Sasson) | Yes — DoS-resistant multicast | Yes — gossip, randomized UDP ports | Yes — DSN / IEEE TDSC | **INCLUDED** |
| **FlightPath** (Li, Clement, Marchetti, Kapritsos, Robison, Alvisi, Dahlin) | Yes — BAR/ε-Nash model, tolerates Byzantine + rational peers | Yes — P2P gossip streaming | Yes — OSDI 2008 | **INCLUDED** |
| **Trinity** (USC ANRG) | Yes — BFT + blockchain persistence | **No** — distributed brokers + MQTT | Yes — IEEE | **EXCLUDED** — broker-based |
| **Chios** | Yes — BFT brokers + threshold crypto | **No** — broker set running BFT | Yes | **EXCLUDED** — broker-based |
| **P2S** (Kazemzadeh & Jacobsen) | Yes — configurable failure count | **No** — tree-based broker overlay | Yes | **EXCLUDED** — broker-based |
| **Secret Forwarding / Secure Event Dissemination** (Choi et al.) | Partial — tolerates Byzantine *brokers* via replication | **No** — broker overlay + replicas | Yes — PLoS ONE | **EXCLUDED** — broker-based |
| **Subscription Subgrouping** (structured overlay) | No | **No** — broker overlay | Yes — arXiv | **EXCLUDED** — not Byzantine; broker-based |
| **BFT Pub/Sub for Cloud** (Kazemzadeh & Jacobsen) | Yes — but a directions/position paper | **No** — broker paradigm | Yes — IEEE | **EXCLUDED** — broker-based; not a concrete protocol |
| **PSVR** (self-stabilizing ad-hoc pub/sub) | No — crash/self-stabilizing only | Yes — brokerless WSN | Yes — arXiv | **EXCLUDED** — not Byzantine |
| **Scribe** (Castro, Druschel, Kermarrec, Rowstron) | No — crash recovery only | Yes — P2P over Pastry | Yes — NGC 2001 / JSAC 2002 | **EXCLUDED** — not Byzantine |
| **lpbcast** (Eugster, Guerraoui, Handurukande, Kouznetsov, Kermarrec) | No — crash/probabilistic only | Yes — gossip, partial views | Yes — DSN 2001; TOCS 2003 | **EXCLUDED** — not Byzantine |
| **Probabilistic Multicast / pmcast** (Eugster & Guerraoui) | No — crash only | Yes — gossip, interest-based | Yes — DSN 2001 | **EXCLUDED** — not Byzantine |
| **Bayeux** (Zhuang, Zhao, Joseph, Katz, Kubiatowicz) | No — crash/redundant-tree fault tolerance | Yes — P2P over Tapestry | Yes — NOSSDAV 2001 | **EXCLUDED** — not Byzantine |
| **CAN application-level multicast** (Ratnasamy, Handley, Karp, Shenker) | No — crash only | Yes — CAN overlay | Yes — NGC 2001 | **EXCLUDED** — not Byzantine |
| **Tera** (Baldoni, Beraldi, Quéma, Querzoni, Tucci-Piergiovanni) | No — crash only | Yes — P2P topic routing | Yes — DEBS 2007 | **EXCLUDED** — not Byzantine |
| **Spidercast** (Chockler, Melamed, Tock, Vitenberg) | No — crash only | Yes — interest-aware overlay | Yes — DEBS 2007 | **EXCLUDED** — not Byzantine |
| **Vitis** (Rahimian, Girdzijauskas, Payberah, Haridi) | No — crash only | Yes — gossip hybrid overlay | Yes — IPDPS 2011 | **EXCLUDED** — not Byzantine |
| **BFT Pub/Sub: A State Machine Approach** (Jehl & Meling) | Yes — SMR-replicated brokers | **No** — broker replicas | Yes — DAIS/PRDC 2013 | **EXCLUDED** — broker-based |
| **Topiary** (Mao & Bojja Venkatakrishnan) | No — bandit-based topology learning, no adversary model | Yes — P2P dapp overlay | **No** — arXiv 2023 preprint | **EXCLUDED** — not Byzantine; not peer-reviewed |
| **EpiSub** (libp2p) | No — epidemic active/passive partitioning | Yes — libp2p overlay | **No** — libp2p spec/design only | **EXCLUDED** — not Byzantine; no peer-reviewed publication |
| **LiFTinG** (Guerraoui, Huguenin, Kermarrec, Monod, et al.) | Partial — detects/tracks free-riders (rational), not full Byzantine | Yes — gossip overlay | Yes — peer-reviewed | **EXCLUDED** — addresses free-riding, not arbitrary Byzantine faults |
| **DHT / structured-overlay content pub/sub family** — Meghdoot, PastryStrings, Hermes, Sub-2-Sub, DPS (Anceaume et al.), Baldoni structured-overlay pub/sub, MFT-PubSub, Ferry, Willow | No — crash/churn tolerance only | Yes — DHT overlays (CAN/Pastry/Chord) | Yes — ICDCS / IPTPS / Middleware etc. | **EXCLUDED** — none claims Byzantine resilience |

## Byzantine reliable broadcast primitives

These expose a broadcast/dissemination API rather than a full topic/content pub/sub interface. Include them only if the underlying broadcast layer counts for your purposes.

| Protocol | Byzantine resilient? | Brokerless? | Peer-reviewed? | Verdict |
|---|---|---|---|---|
| **Murmur / Sieve / Contagion — Scalable BRB** (Guerraoui et al.) | Yes — probabilistic BRB via stochastic samples | Yes — gossip, logarithmic fan-out | Yes — DISC 2019 | **INCLUDED** (broadcast primitive) |
| **Byzantine update diffusion** (Malkhi, Mansour, Reiter; Malkhi, Reiter, Rodeh, Sella) | Yes — epidemic diffusion, b+1 acceptance rule | Yes — replica gossip | Yes — SRDS 1999/2001; TCS 2003 | **INCLUDED** (foundational diffusion) |
| **Optimal Unconditional Information Diffusion** (Malkhi, Pavlov, Sella) | Yes | Yes | Yes — DISC 2001 | **INCLUDED** (diffusion primitive) |
| **Practical BRB on Partially Connected Networks** (optimized Bracha + Dolev) | Yes | Yes — point-to-point | Yes — peer-reviewed | **INCLUDED** (broadcast primitive) |
| **Dynamic Byzantine Reliable Broadcast (DBRB)** (Guerraoui, Komatovic, Kuznetsov, Pignolet, Seredinschi, Tonkikh) | Yes — BRB with dynamic membership/reconfiguration | Yes — quorum-based, no brokers | Yes — OPODIS 2020 | **INCLUDED** (broadcast primitive) |
| **Dynamic Probabilistic Reliable Broadcast** (Albouy, Anceaume et al.) | Yes — probabilistic BRB under churn | Yes — sampling-based, no brokers | Yes — OPODIS 2024 | **INCLUDED** (broadcast primitive) |
| **Secure & Efficient Asynchronous Broadcast** (Cachin, Kursawe, Petzold, Shoup) | Yes — Byzantine consistent/reliable broadcast, threshold crypto | Yes — among n peers, no broker overlay | Yes — CRYPTO 2001 | **INCLUDED** (broadcast primitive) |
| **OptimumP2P** (Nicolaou et al.) | Yes — RLNC coded shards resist corruption | Yes — libp2p replacement | **No** — arXiv 2025 preprint only | **EXCLUDED** — no peer-reviewed publication yet |
| **Abortable Broadcast / Slot Table** (Drijvers et al., DFINITY) | Yes — bounded-memory delivery under Byzantine peers, DoS-resistant; runs on the Internet Computer | Yes — authenticated P2P, no brokers | **No** — arXiv 2024 preprint (benchmarked against GossipSub) | **EXCLUDED** — no peer-reviewed publication confirmed yet |
| **HyParView / Plumtree** (Leitão et al.) | No — crash/churn resilience only | Yes — gossip overlay | Yes — DSN 2007 | **EXCLUDED** — not Byzantine |
| **CYCLON / SCAMP / Newscast** | No — fragile under Byzantine faults | Yes | Yes | **EXCLUDED** — not Byzantine |


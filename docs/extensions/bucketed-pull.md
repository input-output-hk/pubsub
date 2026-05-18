# Bucketed Pull — Gist

**Date:** May 2026

## The attack we're defending against

An attacker controls K Sybil identities. They attempt to **fill victim v's outbound serving slots** in a single round, denying honest peers' pull requests. The victim has a hardware-imposed serving cap **OC** (outbound connections per round). The attack succeeds when attacker requests at v exceed OC; the goal is to keep concentration ≤ OC.

A bookkeeping rule sits underneath everything: each server accepts at most one request from a given (identity, round). This kills trivial single-identity flooding; what remains is the **multi-identity attack**.

## Approach — two pieces

### Piece 1 — identity cost

Each identity locks **D ADA** on chain (slashable, withdrawal-delayed; distinct from delegation stake). An attacker with budget β can sustain at most **K ≤ β/D** Sybils.

### Piece 2 — per-round hash-based bucketing

For each (round R, topic T), define a directional pull-permission predicate over an ordered (puller, server) pair:

```
allow(Alice → Bob, R, T)  ≡  H(nonce_R, T, node_id_Alice, node_id_Bob) mod B  ==  0
```

**Pull rule:** Alice may pull from Bob iff `allow(Alice → Bob, R, T)` holds.

**Verification:** Bob recomputes one hash and one comparison.

## Notation

| Symbol | Meaning |
|---|---|
| N | Global registered identities, across all topics |
| D | Deposit per identity |
| β | Attacker budget |
| K = β/D | Bound on attacker Sybil count |
| H_v | Per-peer view size |
| RF | Pull fanout per peer per round |
| OC | Cap of outbound connections an honest peer serves per round (hardware-set) |
| B | Buckets per (round, topic) |

## Probability — concentration on victim v per round

The attacker is **not constrained to honest-sampled views**. Once they learn v's descriptor, they share it across all K Sybils and persist it across rounds. An attacker Sybil J therefore targets v whenever its bucket aligns with v's. Under random-oracle H, the per-Sybil bucket alignment probability is 1/B (uniform), so:

| | Without bucketing | With bucketing |
|---|---|---|
| Per-Sybil targeting probability | 1 (attacker picks freely) | 1/B |
| Expected attacker requests at v per round | **K** | **K / B** |

**Reduction factor: B.** Independent of H_v and N.

## Deposit calibration — the practical impact

Setting `concentration ≤ OC` and solving for D:

```
Without bucketing:  K        ≤ OC   ⇒   D ≥ β / OC
With bucketing:     K / B    ≤ OC   ⇒   D ≥ β / (B · OC)
```

## Parameter fine-tuning

The protocol has two independent attack-resistance constraints. Each sets a maximum tolerable Sybil count K; the deposit must satisfy *both*:

```
Eclipse target:   K  ≤  k_max  =  N · ε_e^(1/RF)            (so (k/N)^RF ≤ ε_e)
Flood target:     K  ≤  K_max  =  OC · B                    (so K/B ≤ OC)
Deposit floor:    D  ≥  β / min(k_max, K_max)
```

The **smaller** of the two bounds is binding — whichever attack is "easier" (fewer Sybils needed) sets the deposit floor. Using max would let the easier attack succeed.

### Per-parameter notes

| Parameter | Constraints | Tuning levers |
|---|---|---|
| **N** | Observed on chain | Grows over time; effective adversary fraction k/N shrinks as more participants register |
| **S_T** | S_T ≤ N (per-topic; not directly observable) | Bounds H_v: H_v ≤ S_T − 1 |
| **β** | Threat-model assumption | Sets D floor |
| **D** | D ≥ β / min(k_max, K_max) | Tunable upward to harden against larger β |
| **RF** | M2 fanout, sets eclipse exponent | Doubling RF takes √ of the previous adversary fraction |
| **B** | B ≤ H_v / RF (feasibility); balanced at H_v / RF | Larger B → stronger flood protection (smaller K/B); smaller B → stronger eclipse resistance |
| **OC** | OC ≥ RF + c·√RF (variance buffer); typically 2–3 · RF | Hardware-set per peer |
| **H_v** | H_v ≥ B · RF (feasibility); H_v ≤ S_T − 1 | Set by discovery layer; raising H_v relaxes the B ceiling |

OC variance buffer at common RF values:

| RF | OC ≈ RF + 3√RF |
|---|---|
| 3 | 8 |
| 5 | 11 |
| 10 | 19 |

### Worked example — varying RF at fixed ε_e = 10⁻⁶

Fixed: β = $500 000, N = 10 000, H_v = 1 000, ε_e = 10⁻⁶. OC follows `RF + 3√RF` (rounded). B = H_v/RF (balanced).

| RF | OC | B | k_max | K_max | min | binding | min/N | D ≥ |
|---|---|---|---|---|---|---|---|---|
| 3 | 8 | 333 | 100 | 2 666 | 100 | eclipse | 1.0% | $5 000 |
| 5 | 11 | 200 | 631 | 2 200 | 631 | eclipse | 6.3% | $792 |
| 7 | 15 | 143 | 1 389 | 2 143 | 1 389 | eclipse | 13.9% | $360 |
| 10 | 19 | 100 | 2 512 | 1 900 | 1 900 | **flood** | 19.0% | $263 |
| 15 | 27 | 67 | 3 981 | 1 800 | 1 800 | flood | 18.0% | $278 |

Two patterns to read out of the table:

1. **The binding constraint flips around RF ≈ 8–10.** At low RF, eclipse is the easier attack (eclipse k_max < flood K_max); raising RF helps a lot because k_max grows like ε_e^(1/RF). At high RF, flood becomes the easier attack because K_max decreases — at fixed H_v, K_max = H_v · (1 + 3/√RF) shrinks as RF grows (OC grows slower than B shrinks). The optimum sits where the two curves cross.
2. **min/N — the maximum tolerable adversary fraction — is a useful lens.** It tells you what fraction of the network the protocol resists at a given configuration. For ε_e = 10⁻⁶ and these parameters, the system tolerates roughly **19% adversary fraction** at the optimal RF before D starts climbing again. Below the optimum, you're paying for unnecessarily strict eclipse resistance; above it, you're paying for diminishing flood capacity.

The minimum D for these inputs is around RF ≈ 10 at D ≈ $263. Outside that neighbourhood, D climbs in either direction.

### Small-topic regime caveat

When S_T is small (≤ a few times RF):

- H_v is bounded by S_T, so it can't be raised to relax the feasibility constraint.
- B is forced toward 1 (no concentration protection).
- The random-sampling layer is structurally inadequate.

The bucketed-pull mechanism degrades gracefully but does not provide meaningful security in this regime. Delivery is carried by the **relay-tier** (golden-tier push) and **local-relays** (mutual-trust links) extensions for those topics.

## Slashing and enforcement

The protocol exposes several structural violations that can trigger slashing without complex coordination:

1. **Bucket mismatch.** A pull request signed by the puller for which `H(nonce_R, T, puller, server) mod B ≠ 0` is direct evidence of violation. Any peer (or the server itself) can submit; the violator's deposit is slashed.
2. **Duplicate (identity, round) at one server.** Already enforced by the per-round bookkeeping (rejected at the server). Two valid signed requests from the same identity to the same server in one round are evidence of misbehaviour.
3. **Same-round, multiple-server duplication.** If pull requests carry an explicit `seq ∈ {1, …, RF}` signed by the puller, two requests with the same `(identity, round, seq)` from different servers are evidence the puller exceeded RF total. 
4. **Server overcapacity reports.** When a server receives more than OC requests in a round — probability ≈ 10⁻³ in the honest case — it can sign and broadcast the identity list. Identities appearing across many overcapacity reports are statistically attackers; threshold-based slashing without per-round per-server broadcasts in the common case.

The combination keeps bandwidth low (rules 1–2 require no extra messaging; rules 3–4 only fire during attack rounds) while making the obvious violations directly punishable.

## Probability — eclipse of an honest victim v per round

A honest victim v is *eclipsed* in a round if every one of its pull targets is adversarial.

Under honest discovery (views are roughly uniform random samples of the registry), the adversary fraction in v's view is ≈ k/N where k is the global adversary count.

**Without bucketing (M2 baseline):** v picks RF random peers from V_v. Eclipse requires all RF to be adversarial:

```
P_eclipse(no bucketing)  ≈  (k / N)^RF
```

**With bucketing:** v pulls from all same-bucket peers in V_v. Pull-set size ≈ H_v / B. The bucketing is a uniform random partition, so the adversary *fraction* inside v's pull set is the same as in v's view — still k/N, not lowered. Eclipse requires all H_v/B pull targets to be adversarial:

```
P_eclipse(bucketing)  ≈  (k / N)^(H_v / B)
```

**At the balanced point B = H_v/RF, both formulas equal `(k/N)^RF`.** Bucketing does **not** directly reduce eclipse probability at the balanced point — what it reduces is *concentration* (the count of attacker requests v receives per round, K/B), not the *fraction* of adversaries in v's pull set.

The intuition that "an attacker can be picked at most K/B times from v's view" is correct as a count bound, but eclipse depends on adversary *fraction* in the pull set, which uniform-random bucketing leaves unchanged.

### Eclipse via the choice of B (tradeoff with concentration)

Bucketing *can* reduce eclipse probability if we deliberately choose B smaller than the balanced point — that gives v a larger pull set, exponentiating eclipse harder — but at the cost of weaker concentration protection:

| B | Pull set size H_v/B | Eclipse probability | Concentration K/B |
|---|---|---|---|
| H_v / (2·RF) | 2·RF | (k/N)^(2RF) | 2·K·RF / H_v |
| **H_v / RF** (balanced) | RF | (k/N)^RF | K·RF / H_v |
| 2·H_v / RF | RF/2 | (k/N)^(RF/2) | K·RF / (2·H_v) |

Smaller B → stronger eclipse resistance, weaker concentration protection. Choose along this curve based on which threat dominates.

## What this depends on

- **Beacon liveness.** nonce_R must be timely, public, and unbiasable (block hash, slot-leader VRF aggregate).
- **Identity registry availability.** Nodes need on-chain N and the identity set.
- **Discovery layer integrity** (views are roughly uniform random samples of the registry). Required for the eclipse formula; the concentration bound holds even without it, since it does not invoke any view-attenuation.

## Composition with other extensions

- Owner-attested **relays** (relay-tier extension) push directly, bypassing bucketing — deterministic delivery channel.
- **Mutual-trust links** (local-relays extension) bypass bucketing by bilateral consent — high-trust additive edges.

Bucketed pull is the random-sampling layer underneath both. The three layers compose additively.

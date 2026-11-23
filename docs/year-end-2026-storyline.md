# Year-end 2026 storyline

The narrative behind the [year-end deck](../web/slides/year-end-2026.draft.html), built from the [logbook](../logbook.md), the [biweekly reports](../biweekly-reports/), the [CPS](cps/README.md), the [CIP](cip/README.md), the March technical review, the AUEB gap analysis and the CIP repository pull requests ([CPS #1270](https://github.com/cardano-foundation/CIPs/pull/1270), [CIP #1271](https://github.com/cardano-foundation/CIPs/pull/1271)).

## The shape of the talk

The problem first, then the outcome, then how we got there. The audience sees CIP-0201 on slide 4, so every later slide answers one question: why does the protocol look like this? The deck has 21 slides.

1. **The problem** (slide 3)
2. **The outcome** (4)
3. **Replace the foundation** (5–9)
4. **Make it measurable** (10–15): Mauro's formalization and models (10–11), the live compare-designs demo (12), Ezequiel's implementation slides (13–15)
5. **Let the evidence choose** (16–17): the M3 vs M4 comparison (16, presented by Ezequiel) and why the protocol looks like this (17)
6. **What's open** (18–19)
7. **How we worked: lessons and open questions** (20), then a summary (21)

The through-line is that evidence changed our minds three times: we set SecureCyclon aside, we moved from M3 to M4, and we re-scoped the CIP into a report that states its own limits.

## 1 · The problem

Cardano participants need messages that must arrive, off chain: incident alerts from protocol teams to SPOs, notices from stake pools to delegators, deadlines from governance bodies to voters, alerts from dApps to their users. Today these travel through chat, mailing lists and backends; nothing standard proves who sent them or makes them hard to suppress. The CPS asks for messages that are authentic, unaltered and hard to silence.

## 2 · The outcome

CIP-0201 is a topic-based pub/sub layer anchored on Cardano, with four mechanisms:

- **Registered nodes:** keys on chain, each with a refundable deposit.
- **A verifiable gate:** public randomness fixes which pairs may connect each epoch; each node picks privately from that set.
- **Symmetric links:** one link per pair carries messages both ways, so either end can supply an honest path.
- **Signed messages:** every subscriber verifies, relays and caches for recovery.

The guarantee, stated with its conditions (the slide opens with "Predicted:"): at 20,000 nodes with 20 % silent, at the reference configuration (k = 10, B = 500, C = 23), the models predict that some honest publisher fails to reach every honest subscriber in about 1 of 200,000 epochs (p_bad 5.1 × 10⁻⁶), redrawn every epoch. This is a prediction, not a measurement.

Status: CPS-0038 and CIP-0201 were submitted on 16 Sep, confirmed by the editors on 29 Sep, and are in review. Merging as Proposed comes after we reply to the open review comments.

## 3 · Replace the foundation

**What we were given.** Four inputs:

- AUEB's deliverables D1 and D2: a three-layer overlay (peer sampling, navigation, dissemination), replication servers and an on-chain topic registry.
- SecureCyclon (ICDCS 2023) as the peer-sampling layer that every guarantee above it inherited from.
- The product documentation: five use cases with requirements.
- A direct line to Spyros Voulgaris, SecureCyclon's co-author, in our sessions from 30 Apr.

The March review reduced the five use cases to one primitive: identifiable parties send verifiable events to interested subscribers. On 28 Apr we scoped to one-way broadcasts and deprioritised the bidirectional multi-sig branch.

**Reviewing the architecture.** The March review and the gap analysis found no security model, no anti-spam protection and no incentives. The framing finding came on 30 Apr: the threat is silencing, not forgery. SecureCyclon gives non-bias (X % malicious nodes yield X % malicious links), which does not stop silencing, and Spyros agreed the design did not address it (gap analysis S-02, S-04). On 5 May we stopped generating ideas and turned to SecureCyclon.

**SecureCyclon findings.** A silent adversary forges nothing and breaks no rule; it only chooses whom it contacts and what it withholds. The team's silent-attack report (28 Jun; N = 200, view size 20, 3 seeds) found that at 20 % adversarial nodes a targeted victim's view is 72.5–80.3 % malicious, and at least 80 % malicious in 37–68 % of cycles. At 30 % it is 98.6–99.3 %. No detection fired. The finding holds for the tested configurations, not for every sampler. Earlier, on 28 May with Spyros, we revisited the result that 15 % malicious nodes reach about 8.9× amplification.

**A deposit implies a list, and a list replaces sampling.** Sybil resistance needs a cost per identity (23 Jun). A deposit implies an on-chain list, and once every node is known, a sampling layer adds attack surface without adding security (25 Jun). A known list invites targeted flooding, hence the verifiable gate (bucketed hash gating, 8 May; restated 25 Jun). Timeline: 8 May bucketed gating, 19 May list-based design, 25 Jun on-chain registry with rejection sampling, 28 Jun silent-attack report, 30 Jun phase 2 moves to experiments, 7 Jul SecureCyclon formally retired. The slide strikes both navigation and peer sampling, since the list-based design replaced both.

**The question changes** (slide 9). With every node on chain, nodes can pick peers uniformly; the question becomes whether every honest publisher reaches every honest subscriber when some peers stay silent. A good topology and a bad one, where two silent neighbours cut a node off, set up the formalization.

## 4 · Make it measurable (Mauro, Ezequiel)

The live demo (slide 12) embeds the compare-designs tool between Mauro's and Ezequiel's slides: drag the adversarial share and M3 falls below the target near 0.22, M4 near 0.26. Their slides define the problem (good topology, silent fraction μ, p_bad ≤ δ, downtime as a shift in μ), give the five models' leading coverage laws, and show one protocol core run two ways (prototype node and deterministic simulator), AI-assisted development with structure, and predictions checked against execution.

Background for questions: the ungated laws agree with the simulator in 38 of 40 downtime predictions within 95 % intervals; the gated closed forms were derived after the measurements on one instrument and reproduced in review, without a second implementation. The 2 Jul logbook records why the hand-derived laws matter: Claude helped write both the node and the simulator, so correlated errors were possible.

## 5 · Let the evidence choose

**Evidence moved us from M3 to M4.** M3 was the implementation base in July (7 and 15 Jul), chosen from comparisons without the gate or cap. With them, and with security weighted ahead of efficiency (13 Aug), M4 won and was selected on 20 Aug.

**Before gating — costs and downtime margin.** The slide compares M3 at RF = 13, s = 7 with M4 at RF = 9, at N = 20,000 and μ = 0.2. Both meet the 10⁻⁴ per-epoch topology-failure target. Costs and forwarding depths are measured; downtime tolerance is predicted.

| | M3 | M4 | Source |
| --- | ---: | ---: | --- |
| Received copies per publication per honest node | 10.4 | 13.4 | Companion Table 4 |
| Mean logical links held | 38.0 | 18.0 | Companion Table 4 |
| Mean full-coverage hops | 5.5 | 5.0 | Companion Table 4 |
| Predicted independent honest downtime absorbed | 2.17 % | 7.43 % | Companion Table 4 |

M3 keeps the bandwidth edge: about 22 % fewer received copies at the same message size and publication rate. M4 holds fewer logical links and has more predicted downtime margin at these configurations. Full-coverage hops measure forwarding depth to the last honest node, not elapsed time or a worst-case bound.

**With gating — equal expected receiving pools.** The second table uses B = 500 for both models, giving approximately 40 eligible sources per node. M3 uses RF = 13, s = 7 and B_relay = B_publisher = 500; M4 uses k = 10, B = 500 and C = 23. These are different configurations from the ungated table above.

| | M3 | M4 | Source |
| --- | ---: | ---: | --- |
| Predicted topology failure per epoch | 4.4 × 10⁻⁵ | 5.1 × 10⁻⁶ | [E20 synthesis §7](../pubsub-node/docs/experiments/m4-synthesis.md#7-m3-under-the-same-armor), equal deafen-cost row and M4 reference |

Both meet the target, so the slide omits a pass/fail column. The M3 value is the synthesis's derived B = 500 prediction, rather than the B = 769 reference in CIP Table 10. This comparison holds receiving-pool size equal, not total eligibility across both directions. It concerns network-wide topology failure, not the cost of adaptively eclipsing a chosen victim; the probabilities are predictions, not measured rare-event rates. It does not establish that M3 cannot match M4 with further parameter changes.

**Symmetry rescues.** In M4, an honest eligible peer selecting a node can provide connectivity even when that node's own selections all land on adversaries. M3's relay-source selections do not offer the same reciprocal rescue. Neither design can recover an honest source from an eligible receiving pool containing no honest peers.

**Background, not shown on the slide.** A separate comparison at equal expected either-direction eligible reach of 32 peers per identity observed 17 failed runs out of 400 for M3 and none out of 400 for M4. That uses different configurations and a different normalization from the slide. Zero failures does not establish zero failure probability. See the CIP's Alternatives considered and E20 synthesis §7.

**Why it looks like this.** Each mechanism answers a finding. The slide shows each row as a short phrase; the full wording and dates are here:

| Finding | Date | What CIP-0201 does |
| --- | --- | --- |
| The threat is silencing, not forgery | 30 Apr | Signed messages settle forgery; one bar for the rest: every honest publisher reaches every honest subscriber |
| Identities must cost something | S-04 · 23 Jun | Refundable deposit and an on-chain node registry |
| A full list makes peer sampling unnecessary | 25 and 28 Jun | Nodes select locally from the registry |
| A known list invites targeted flooding | 8 May · 25 Jun | A verifiable gate from epoch randomness, plus an admissions budget (ADR 0042) |
| One link serving both directions needs the fewest picks and leaves the most margin | 11–20 Aug | The symmetric relay link, gated on the identity-sorted pair (13 Aug) |
| A bad draw must not last | 13 Aug | Epoch rotation and recovery caches |

## 6 · What's open

**Where review stands** (slide 18). Submitted 16 Sep, confirmed 29 Sep as CPS-0038 and CIP-0201 (Network category), review open: we are responding to review feedback. Next comes a merge as Proposed, after we reply and the editors bring it back to the CIP meeting; then Path to Active. The slide shows only the status, in three steps: submitted and numbered; reviews answered, leaving time for more feedback to accumulate; and a merge as Proposed targeted by the end of the year. Background for questions: reviewers asked whether the problem is real given existing channels and on-chain data, how large the deposit must be (we stated the outlay, 4,000 simultaneous deposits to hold μ = 0.2 of 20,000), whether proof of work or reputation could complement the deposit, and whether this belongs in the node or in tooling, and how it compares with Nostr.

**Gaps** (slide 19). The CIP states each of these itself, under Limits of this evidence or Path to Active. The slide shows them as short cards in four columns (shown, open evidence, open spec, open deployment), adds that the 20 % adversary is assumed rather than derived, and leaves correlated failure, the beacon and epoch details, the full profile parameters, the two interoperating implementations and the operator pilot out of the slide.

- *Evidence:* tail probabilities extrapolated; rotation argued, not simulated; gated laws not formally derived; small topics outside the analysis (the gate is off up to 40 registrations at k = 10); correlated failure out of scope.
- *Specification:* the beacon source and the final gate construction (the hash gate is the evaluated baseline; no private construction is selected); handshake recipient and deployment binding, replay handling and the full state machine (the draft signed preimage must not be deployed); epoch numbering and boundaries; recovery and catch-up exchanges; behaviour through a chain halt or fork; the relationship to CIP-0137.
- *Deployment:* μ and deposit economics; rerun of the candidate profile B = 512, k = 10, C = 24; two interoperating implementations; an operator pilot and adoption threshold.

## 7 · Lessons learned and summary

Lessons and open questions (slide 20).

**What worked**

- *Decisions written down.* The logbook and ADRs let Mauro join on 6 Aug and correct M4's eclipse-cost figure by 11 Aug.
- *Two instruments, cross-checked.* Hand-derived laws checked against a deterministic simulator running the reference implementation's code, without the async runtime or real transport (2 Jul, 13 Aug).

**What we would reconsider** (framed as open questions)

- *Define the scope gap early?* What product envisioned and what the research offered overlapped little; we estimate close to half our time went into bridging it (18 Nov review session). Defining that gap sooner might have meant handing the problem back or choosing a new approach earlier.
- *When to close a work stream?* When SecureCyclon didn't hold, we redesigned within the same stream rather than stopping, reporting and restarting with new goals (18 Nov review session).
- *The right stage for a CIP?* CIP-0201 specifies a solution with known gaps and their requirements, not one ready to implement end to end.

Comparing designs under the real rules (M3 led until the gate and cap were included) is covered on slide 16 and no longer listed here.

**Summary** (slide 21): SecureCyclon does not give a uniform view under a silent adversary, in our tests; an on-chain registry with deposits replaces peer sampling; five designs, formalised and simulated with our reference implementation; M4 with a verifiable gate and symmetric links; CPS-0038 and CIP-0201 in review. It summarises only what we proposed; the open items stay on slide 19.

## Tools on the website worth showing

- [Compare designs](../web/experiments/compare-designs/): ungated M1–M5 estimates with population, adversarial share and pick count as controls. Embedded live on slide 12.
- [Parameter surface](../web/experiments/parameters/): exploratory M4 isolation estimates, bucket-count ceilings and an admissions-budget recipe. Not linked from the deck; useful for questions.
- [Model simulator](../web/experiments/models/): watch each model draw its links and a message propagate, at toy scale.
- [How the design was derived](../web/experiments/): the comparisons behind selecting M4.

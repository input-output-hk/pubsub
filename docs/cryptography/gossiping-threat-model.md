# Threat Model: Signature Scheme Choice and System Recoverability

This document extends the analysis in [gossiping.md](gossiping.md), focusing on
how the choice of signature scheme affects the system's ability to recover after
a node compromise.

## The Core Question

When an honest node is compromised, how much damage can the attacker cause, and
how quickly and completely can the system recover? The answer depends critically
on whether the gossip authentication scheme provides forward security, and
whether proactive security is additionally targeted.

## Without Forward Security (e.g., plain Ed25519)

At compromise time T, the attacker obtains the full signing key — valid for
every period, past and future. In the SecureCyclon model, where blacklisting is
triggered by signed evidence of rule violations, this has an immediate and
severe consequence: the attacker can manufacture signed evidence of misbehavior
attributed to any past period, including ones in which the honest node behaved
correctly.

Recovery is therefore not simply a matter of re-keying. The attacker still holds
the original key and can keep producing fake past-period signatures indefinitely.
Full recovery requires replacing the node's identity entirely and rebuilding its
reputation from scratch — a cost that is unbounded, depending on how blacklisting
interacts with re-registration in the protocol.

## With Forward Security (KES)

At compromise period i, the attacker obtains only the key material for period i
onward. Signatures for periods 0 through i−1 are computationally inaccessible.
The honest node's history up to the compromise is cryptographically intact and
cannot be forged.

Recovery is then bounded and local:

1. Detect the compromise.
2. Issue a fresh KES key, certified by the same long-term root credential
   (analogously to how opcerts work in Cardano).
3. Re-enter the network with the past clean record intact.

Only behavior during the window [i, T_rekey] is at risk. Recovery time is
detection_latency + re_registration_latency — both of which can in principle be
engineered to be short and finite.

## The Residual Problem: Undetected Compromise

Even with KES, if the compromise is never detected, the attacker can impersonate
the node for all remaining KES periods, and eventually force a re-keying when the
key schedule is exhausted. The damage window is bounded by the total number of
remaining periods at the time of compromise, which can be large.

Proactive security closes this gap: by periodically refreshing keys regardless
of whether a compromise has been detected, the attacker's effective window is
bounded by the refresh interval even in the absence of detection. The trade-off
is significantly higher implementation complexity, as no proactive security
scheme is currently supported in Cardano's stack.

## Recoverability Summary

| Scheme | Past history | Future impersonation window | Recovery requires |
|---|---|---|---|
| SUF only | Forgeable retroactively | Unbounded | Full identity replacement |
| KES (forward-secure) | Cryptographically intact | Detection + re-key latency | Re-key under same root credential |
| Proactive security | Cryptographically intact | At most one refresh interval | Re-key; detection not required |

## Implication for the Next Steps

The practical question this analysis raises is: **what is the acceptable bound on
the compromise window, given the expected detection and re-keying latency in the
target deployment?** If detection is fast and re-keying is operationally
straightforward (as it can be given the opcert parallel), KES may provide
sufficient recovery guarantees. If detection is unreliable or slow, proactive
security may be necessary despite the implementation cost.

This should inform the decision on signature scheme before key derivation and
trust anchoring options are evaluated (see the dependency chain in
[gossiping.md](gossiping.md)).

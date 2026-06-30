# Drum — DoS-Resistant Gossip Multicast

*Badishi, Keidar, Sasson — DSN 2004; extended IEEE TDSC 2006 (extended TR + text in this folder).*

## What it is
A gossip-based **multicast** protocol — single source, probabilistically-reliable delivery to a static group — hardened against **application-level DoS attacks**. Plain push- or pull-only gossip can be crippled by a *targeted* attacker flooding a victim's one channel; Drum removes that vulnerability.

## Model
- **Static group**; each message has one source. (Dynamic membership only sketched, via an optional CA.)
- **Asynchronous**, fully-connected network; constant, independent link loss.
- Message **sources are authenticated by signatures**; the protocol's random ports are encrypted (PKI).
- **Adversary** can fabricate messages, snoop, and flood (all cost resources); malicious members also refuse to forward.
- **Application-level DoS only** — network-level DoS defenses are assumed already in place (a flood not aimed at the random ports doesn't affect them).

## How it works (three measures)
Each node runs in rounds; per round it picks two small random peer sets, `viewpush` and `viewpull`:
1. **Push + pull combined.** A node both *pushes* messages to `viewpush` and *pulls* missing ones from `viewpull`. Receiving still works even if one direction is attacked.
2. **Separate resource bounds per operation.** Push and pull have independent caps (bounded messages accepted per port per round); a flood on one can't consume the other's capacity. Unread messages are discarded at round end (rounds vary locally, so the attacker can't aim for the round start).
3. **Random, encrypted ports.** The reply/data ports are freshly chosen each round and sent encrypted, so an attacker cannot predict or target them.

*(Push: `push-offer → push-reply(digest) → data`; Pull: `pull-request(digest) → pull-reply`, each over a random reply port.)*

## Guarantees (closed-form analysis + simulation + Java implementation)
- **Lemma 1.** For any fixed fraction `α < 1` of *targeted* processes, expected propagation time is **bounded by a constant independent of attack strength `x`** — ramping up a focused attack gains the adversary nothing. (A non-attacked channel accepts a valid message with probability `pu > 0.6`.)
- **Lemma 2.** The adversary's best strategy is to **spread** the attack over all processes (increase `α`), not concentrate it.
- Under a *broad* attack on **all** processes, every protocol (push-only / pull-only / Drum) degrades **equally** — unavoidable. Drum's advantage is specifically against **targeted/focused** DoS, where push-only and pull-only collapse.

## Scope
Handles DoS + message forgery (blocked by signatures) + non-forwarding by malicious members. It is **not** a consensus / agreement / ordering primitive — it is a DoS-hardened dissemination layer; network-level DoS is out of scope.

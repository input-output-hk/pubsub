# Topic Registry – Quint Specification

Formal spec of the on-chain Topic Registry smart contract. Models topic
lifecycle, role-based access control (owners / admins / publishers), and
the authorization matrix from the architecture report (Table 2.1).

Written in [Quint](https://quint-lang.org/).

## Files

| File | What it does |
|---|---|
| `types.qnt` | Data model — `Topic`, `TopicRegistry`, `Message` variants |
| `topic_registry.qnt` | Pure contract logic — all 10 operations + auth checks |
| `topic_registry_env.qnt` | Closed-loop environment for simulation & model checking |
| `topic_registry_props.qnt` | 15 invariants + 1 temporal liveness property |
| `topic_registry_test.qnt` | Unit tests (regression coverage for known bugs) |
| `spells/` | Quint standard library helpers |
| `check_with_tlc.sh` | Run TLC model checking (requires Java) |

## Install Quint

See [Getting Started](https://quint-lang.org/docs/getting-started) or just:

```
npm install -g @informalsystems/quint
```

Requires Node.js 18+.

## Run tests

```
quint test topic_registry_test.qnt
```

If the Rust evaluator fails to download, use the TypeScript backend:

```
quint test topic_registry_test.qnt --backend typescript
```

## Model checking

Exhaustively verify invariants with TLC. Requires Java and
[Apalache](https://apalache-mc.org/) installed at
`~/.quint/apalache-dist-0.51.1/`.

```
# Check a single invariant
./check_with_tlc.sh topic_registry_props.qnt \
  --main topic_registry_props \
  --step env_next \
  --invariant inv_aliveTopicHasOwner

# Check a temporal property
./check_with_tlc.sh topic_registry_props.qnt \
  --main topic_registry_props \
  --step env_next \
  --temporal temp_inboxEventuallyDrained
```

See `./check_with_tlc.sh --help` for all options.

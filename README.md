# Cardano PubSub

A native publish/subscribe communication layer for the Cardano ecosystem: nodes subscribe to topics and receive every message honest publishers send on them, under an adversary model with silent Byzantine participants.

The project is currently in **Phase 2 — empirically-driven architecture** ([#46](https://github.com/input-output-hk/pubsub/issues/46)): candidate dissemination topologies (the *M models*) are analysed formally and validated experimentally, converging on a design proposal by mid-August 2026.

## Repository map

| Path | What it holds |
|------|---------------|
| [`site/`](site/) + [`mkdocs.yml`](mkdocs.yml) | Documentation site: product vision, use cases, architecture, economics |
| [`docs/`](docs/) | Design documents: technical reviews, gap analyses, design synthesis, extension proposals |
| [`formal_spec/`](formal_spec/) | Formal models (Quint, PRISM): peer sampling, hybrid dissemination (the M models), topic registry |
| [`pubsub-node/`](pubsub-node/) | Rust prototype of the PubSub node — see its [README](pubsub-node/README.md) for the spec-driven workflow |
| [`contracts/`](contracts/) | Aiken on-chain contracts: node registry, topic registry |
| [`CardanoPubSub/`](CardanoPubSub/) | Earlier Java simulation code and latency experiments |
| [`logbook.md`](logbook.md) | Running log of technical decisions and progress, most recent first |
| [`biweekly-reports/`](biweekly-reports/) | Biweekly progress reports |

## Following progress

- **[Logbook](logbook.md)** — decision-level progress notes, updated weekly.
- **[Biweekly reports](biweekly-reports/)** — periodic summaries.
- **[Phase 2 issue (#46)](https://github.com/input-output-hk/pubsub/issues/46)** — goals, outcomes, and the three work tracks: formal analysis ([#76](https://github.com/input-output-hk/pubsub/issues/76)), prototype & experiments ([#79](https://github.com/input-output-hk/pubsub/issues/79)), design proposal ([#91](https://github.com/input-output-hk/pubsub/issues/91)).

## Running the prototype

The Rust node prototype lives in [`pubsub-node/`](pubsub-node/):

```sh
cd pubsub-node
cargo test
```

Protocol design background lives in [`docs/`](docs/) and [`formal_spec/`](formal_spec/); the prototype README explains how specs, plans, and implementation connect.

## Documentation site

Built with MkDocs Material from [`site/`](site/):

```sh
pip install mkdocs-material
mkdocs serve
```

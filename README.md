# Cardano PubSub

A native publish/subscribe communication layer for the Cardano ecosystem: nodes subscribe to topics and receive every message honest publishers send on them, under an adversary model with silent Byzantine participants.

The project is currently in **Phase 2 — empirically-driven architecture** ([#46](https://github.com/input-output-hk/pubsub/issues/46)): candidate dissemination topologies (the [*M models*](formal_spec/hybrid_dissemination/models/README.md)) are analysed formally and validated experimentally, converging on a design proposal by mid-September 2026.

> **Important Disclaimer & Acceptance of Risk**
>
> This is a proof-of-concept implementation that has not undergone security auditing. This code is provided "as is" for research and educational purposes only. It has not been subjected to a formal security review or audit and may contain vulnerabilities. **Do not use this code in production systems or any environment where security is critical without conducting your own thorough security assessment.** By using this code, you acknowledge and accept all associated risks, and our company disclaims any liability for damages or losses.

## Repository map

| Path | What it holds |
|------|---------------|
| [`web/`](web/) | Public [GitHub Pages site](https://pubsub.cardano-scaling.org/): workstream overview and progress presentations |
| [`docs/`](docs/) | Design documents: technical reviews, gap analyses, design synthesis, extension proposals |
| [`formal_spec/`](formal_spec/) | Formal models (Quint, PRISM): peer sampling, hybrid dissemination (the [M models](formal_spec/hybrid_dissemination/models/README.md)), topic registry |
| [`pubsub-node/`](pubsub-node/) | Rust prototype of the PubSub node — see its [README](pubsub-node/README.md) for the spec-driven workflow |
| [`logbook.md`](logbook.md) | Running log of technical decisions and progress, most recent first |
| [`biweekly-reports/`](biweekly-reports/) | Biweekly progress reports |

## Following progress

- **[Website](https://pubsub.cardano-scaling.org/)** — public overview and the latest progress presentation.
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

## Website & documentation

The [public site](https://pubsub.cardano-scaling.org/) is deployed from [`web/`](web/) on every merge to `main`. The earlier MkDocs documentation (product vision, use cases, architecture, economics) is retired and preserved on the [`archive/mkdocs-site`](https://github.com/input-output-hk/pubsub/tree/archive/mkdocs-site/site) branch.

## Contributing & security

See [CONTRIBUTING.md](CONTRIBUTING.md) for how to get involved and [SECURITY.md](SECURITY.md) for reporting vulnerabilities.

## License

Copyright 2025 Input Output Global

Licensed under the Apache License, Version 2.0 (the "License"). You may not use this repository except in compliance
with the License. You may obtain a copy of the License at <http://www.apache.org/licenses/LICENSE-2.0>

Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an
"AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the [License](LICENSE) for
the specific language governing permissions and limitations under the License.

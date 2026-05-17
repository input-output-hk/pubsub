# pubsub-node

Rust implementation of the Cardano PubSub node. Part of the [Cardano PubSub workstream](../) — currently in Architecture & Design phase. Implementation scaffolding is being assembled in this directory; the protocol design itself lives in the parent repo under `docs/` and `formal_spec/`.

## Status

Initial scaffold. Constitution v1.0.0 ratified; first feature spec to follow.

## For contributors

1. Read the [project constitution](.specify/memory/constitution.md) — five principles governing implementation work in this directory. Read this before authoring code or specs.
2. Read [`CLAUDE.md`](CLAUDE.md) for the Spec Kit feature-development workflow and the manual branch step required in this layout.
3. Background on the protocol: [`docs/`](../docs/) holds the design synthesis, gap analysis, and extension proposals; [`formal_spec/`](../formal_spec/) holds the Quint and PRISM models.

## Spec Kit workflow

Features flow through:

    /speckit-specify  →  /speckit-plan  →  /speckit-tasks  →  /speckit-implement

Before each `/speckit-specify`, cut the feature branch manually (Spec Kit can't auto-create it in this nested layout — see [`CLAUDE.md`](CLAUDE.md) for the rationale):

    ls specs/                              # peek at the next available number
    git checkout -b NNN-<short-name>       # e.g. 001-minimal-node-scaffold

# Contributing

Thanks for your interest in Cardano PubSub! This repository is an active research workstream; external contributions are welcome, but note that the protocol design is still converging — opening an issue to discuss an idea before investing in a pull request is usually the best first step.

## Issues

- Use GitHub issues for bugs, questions about the design documents, or proposals.
- For security-relevant findings, follow [SECURITY.md](SECURITY.md) instead of opening a public issue.

## Pull requests

1. Fork the repository (or create a branch, for members) and base your work on `main`.
2. Keep PRs focused — one logical change per PR.
3. `main` is protected: every PR needs at least one approving review, all review conversations must be resolved, and stale approvals are dismissed when new commits are pushed.
4. CI must pass. For changes under `pubsub-node/`, that means:

   ```sh
   cd pubsub-node
   cargo fmt --check
   cargo clippy --all-targets
   cargo test
   ```

## Commit messages

Follow the Conventional Commits style used throughout the history:

```
feat(pubsub-node): short imperative summary
fix(formal_spec): ...
docs: ...
```

## Code and documentation layout

See the [README](README.md) repository map. Rust implementation work in `pubsub-node/` follows the spec-driven workflow described in [`pubsub-node/README.md`](pubsub-node/README.md) — read the project constitution there before authoring code.

## License

By contributing, you agree that your contributions will be licensed under the [Apache License 2.0](LICENSE).

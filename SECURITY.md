# Security Policy

## Reporting a vulnerability

Please do **not** report security vulnerabilities through public GitHub issues.

Instead, report them privately via one of:

- **GitHub private vulnerability reporting**: use the [*Report a vulnerability*](https://github.com/input-output-hk/pubsub/security/advisories/new) form on this repository.
- **Email**: [security@iohk.io](mailto:security@iohk.io)

Please include a description of the issue, steps to reproduce, and the affected component (e.g. `pubsub-node`, formal specifications, or the website).

You should receive an acknowledgement within a few business days. Please allow us reasonable time to investigate and address the issue before any public disclosure.

## Scope

This repository is a research workstream. The Rust node in `pubsub-node/` is a **prototype** used for protocol experiments — it is not production software and makes no security guarantees. Findings about the protocol design itself (dissemination models, peer sampling, registry) are equally welcome and best raised through the channels above if they have security impact, or as regular GitHub issues otherwise.

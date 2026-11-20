# CivicCast Lab

Private, paused research project preserving the alternative Rust broadcast engine
originally built as TownLight Station. It is separate from the working
[CivicCast product](https://github.com/scottconverse/civiccast).

This repository retains the Rust control plane and GStreamer media engine,
TypeScript/React operator interface, Windows installer and recovery tooling,
architecture documents, tests and historical evidence. It is unfinished and is
not a released station product.

- [Provenance, separation and next-agent handoff](SEPARATION.md)
- [Inherited project assessment](PROJECT_STATUS.md)
- [Capability ledger](CAPABILITIES.md)
- [Architecture](docs/architecture/overview.md)

The lightweight Python meeting recorder stays in
[Townlight Station](https://github.com/townlight/station/tree/main/beta-1).
Its removal from this current tree does not rewrite the original history or tags.
GitHub Actions are disabled while the project is paused. No Mac port is implemented
or promised. Existing Windows and Rust identifiers remain unchanged inside the
preserved implementation.

# CivicCast Lab

Private research project developing the Rust broadcast engine
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
Cross-platform work targets Windows, macOS on Apple silicon, and Ubuntu. The
channel worker builds with Cargo and a native GStreamer 1.28 development
runtime. On Linux its playout graph requires the Pango text overlay and the Rust
closed-caption plugin. The ingest loudness analyzer uses the Rust audiofx plugin.
Windows development uses the safe Cargo wrapper in scripts/. The installed
station and its 3-, 6-, and 9-channel capacities still need qualification on
the three reference machines.

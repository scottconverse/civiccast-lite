# CivicCast Lab

CivicCast Lab is a private prerelease for meetings, video podcasts, and scheduled programming. Start with the [private prerelease v0.1.0.12](https://github.com/scottconverse/civiccast-lab/releases/tag/v0.1.0.12). It is a separate product from [CivicCast](https://github.com/scottconverse/civiccast) and the [Townlight Station recorder](https://github.com/townlight/station/tree/main/beta-1).

Independent channels can run scheduled local media or live takes. Each channel publishes to one RTMP or RTMPS target. A cable headend path and an independent web path can run from the same station. The viewer portal serves self-hosted HLS and works independently of YouTube.

Rust runs the station service and channel workers. GStreamer 1.28 handles media, React and TypeScript power the operator console, and SQLite stores station state.

## Status

**macOS arm64.** Short runs passed at 3, 6, and 9 channels for cable and the independent viewer portal.

**Ubuntu 26 amd64.** Local RTMP receiver recovery passed.

**Open checks.** Windows R7 has an unresolved intermittent nine-channel picture freeze. Playback at YouTube and capacity on the physical Ubuntu R9 still need verification.

The 3-, 6-, and 9-channel tiers are evaluation targets. Capacity depends on the host, media, encoder, and outputs.

## Quick start

Read the [user manual](docs/USER-MANUAL.md) for detailed steps. Download only from the tagged private prerelease. Verify artifacts against published `SHA256SUMS.txt`. Windows builds are signed. macOS builds are ad-hoc signed. Open the operator console at `http://127.0.0.1:4070` on Windows or `http://127.0.0.1:8787` on macOS and Ubuntu.

## Source map

- [User manual](docs/USER-MANUAL.md)
- [Architecture overview](docs/architecture/overview.md)
- [Viewer portal setup](web/portal/README.md)
- [Marketing landing page](web/marketing/index.html), a static page you can open from a checkout
- [Contributions](CONTRIBUTING.md)
- [Historical handoff](SEPARATION.md)
- [Code license](LICENSE-CODE) and [documentation license](LICENSE-DOCS)

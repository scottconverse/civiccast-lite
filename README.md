# CivicCast Lite

CivicCast Lite is an open-source broadcast station for local government, community media, and independent producers. Run a council meeting live on one channel while a recorded video podcast plays on another. Each channel can feed a cable headend, one RTMP or RTMPS destination, and an independent viewer website.

[Project website](https://scottconverse.github.io/civiccast-lite/) · [Download v0.1.0.14](https://github.com/scottconverse/civiccast-lite/releases/tag/v0.1.0.14) · [User manual](docs/USER-MANUAL.md) · [Architecture diagrams](docs/architecture/overview.md)

## Get started

1. Download the package and SHA-256 checksum file from the release. Choose Windows x64, macOS on Apple silicon, or Ubuntu 26 amd64.
2. Follow the [new-user setup](docs/USER-MANUAL.md#new-users) to verify the package, create the first operator, and open the local console.
3. Import a recording or configure a live source. Start with one channel and verify picture and sound at each receiving endpoint.

The installers use CivicCast Lite download names. The service names and on-disk paths retain the identifiers shown in the manual.

## How it works

The Rust station service owns scheduling, operator authentication, and SQLite state. It supervises one channel worker per enabled channel. GStreamer handles playout, source switching, and distribution. The local operator console uses React and TypeScript. Viewers can watch the station's own HLS feed over HTTPS without a platform account.

| Path | Use it for |
| --- | --- |
| Cable MPEG-TS | An IP handoff to a compatible cable headend or decoder |
| RTMP or RTMPS | YouTube or another compatible streaming service |
| Independent HLS | Your station's website using the generated viewer assets |

## What has been verified

**v0.1.0.14 (commit `6d03fd4`)** passed clean installation, service startup, and operator console checks on Windows x64, macOS arm64, and Ubuntu 26.04 amd64. The Windows check used the final signed installer. [Hosted build and Unix checks](https://github.com/scottconverse/civiccast-lite/actions/runs/45429657255) · [Signed Windows check](https://github.com/scottconverse/civiccast/actions/runs/45434131285)

Earlier playout and capacity checks:

- **macOS** passed short cable and portal checks at 3, 6, and 9 channels. The cable receiver also recovered picture and sound after a channel-worker restart.
- **Windows** passed a native clean install and a short three-channel cable and portal check with real recordings on Halo. An earlier nine-channel R7 test found an intermittent picture freeze.
- **Ubuntu** passed a local RTMP receiver recovery test. Physical R9 capacity and playback in YouTube's viewer remain unverified.

The 3-, 6-, and 9-channel figures describe test tiers. Capacity depends on the machine, media, encoder, and outputs.

## Project links

- [Technical user guide](docs/USER-MANUAL.md#technical-users)
- [Architecture section](docs/USER-MANUAL.md#architecture) and [full architecture diagrams](docs/architecture/overview.md)
- [Independent viewer portal setup](web/portal/README.md)
- [Build and package from source](docs/BUILDING.md), [contributing](CONTRIBUTING.md), and [project history](SEPARATION.md)
- [Code license](LICENSE-CODE), [documentation license](LICENSE-DOCS), and [third-party notices](THIRD-PARTY-NOTICES.md)

CivicCast Lite developed from the Rust Townlight Station project. [CivicCast](https://github.com/scottconverse/civiccast) and the [Townlight meeting recorder](https://github.com/townlight/station/tree/main/beta-1) are separate projects.

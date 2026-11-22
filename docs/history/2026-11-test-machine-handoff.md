# November 2026 test-machine handoff

The [v0.1.0.12 release](https://github.com/scottconverse/civiccast-lite/releases/tag/v0.1.0.12) is the starting point for future installs. Use the package for the target platform and verify it against the release's SHA256SUMS.txt. The Windows R7 test station was on the older 0.1.10 build from `fc23fc2`. The M4 test station was on the published 0.1.0.12 build from `ebffd381`.

## Windows picture freeze

During a nine-channel run, both the cable MPEG-TS recording and the independent HLS recording contained frozen pictures while audio continued. The corresponding source footage advanced normally. The fault was therefore visible before either viewer's playback path. A separate offline control on the R7 showed that feeding a held image to a warmed Media Foundation H.264 encoder can produce byte-identical GOPs. Repeated encoded bytes alone cannot identify which stage first stopped advancing.

If the issue returns, use a short run with real moving footage. Record synchronized frame fingerprints and timestamps at H.264 ingress, decoder output, selection and compositing, and the encoder input and output. Compare those with the local cable and HLS captures. Start with the historical Windows load profile and isolate changes one at a time. No root cause was confirmed in the original run.

## macOS worker recovery

On the installed 0.1.0.12 M4 build, a supervised channel worker restart during a short prerecorded show returned to the same asset. The channel journal remained contiguous, and a cable receiver captured matching picture and audio before and after the restart. The run did not measure the precise interruption duration.

## Building and installing

- Follow the [contributor setup](../../CONTRIBUTING.md) for the pinned Windows Cargo environment.
- Use the platform packaging sources under [installer](../../installer). The [user manual](../USER-MANUAL.md) covers fresh station setup. A fresh install creates its own operator and trust state.
- Consult the [architecture guide](../architecture/overview.md) for worker supervision and output flow.

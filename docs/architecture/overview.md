# CivicCast Lite architecture

This guide describes the CivicCast Lite v0.1.0.12 prerelease. The station service manages programming and supervises each channel. Channel workers keep picture and sound moving and feed the configured outputs. See the [user manual](../USER-MANUAL.md) for installation and operation.

## Runtime topology

`stationd` is a single Rust daemon and the sole writer to SQLite in WAL mode. It owns the HTTP operator API, authorized configuration, ingest, and durable scheduling. The operator UI is a loopback React/TypeScript application served only to the authenticated local operator. `stationd` launches one supervised `channel-worker` process per enabled channel. Each worker owns a persistent in-process GStreamer 1.28 graph, its own clock, source switching, and a per-channel as-run journal. Worker events correlate to pending requests, and the station projects journal evidence into durable state.

Supervision is platform-native: Windows SCM with protected named pipes and job objects; macOS launchd; Ubuntu systemd. On Unix the worker is launched over an inherited socketpair with peer credential and close-on-exec checks plus process groups. The operator HTTP listener binds to loopback: Windows port 4070, Unix port 8787. Managed media is content-addressed and immutable once stored. The viewer website serves the generated public tree through a separate static HTTPS server.

```mermaid
flowchart LR
  subgraph ADMIN["Authenticated loopback admin boundary"]
    OP["Operator console<br/>web/operator"]
    ST["stationd<br/>HTTP API, scheduler, SQLite writer"]
  end
  OP -->|"Loopback HTTP, authenticated"| ST
  ST --> DB[("SQLite WAL")]
  ST --> MEDIA[("Managed media<br/>content-addressed immutable")]
  subgraph WORKERS["Supervised channel workers: one per enabled channel"]
    W1["channel-worker 1"]
    WN["channel-worker N"]
    J1[("as-run journal 1")]
    JN[("as-run journal N")]
  end
  ST -->|"Versioned commands/events"| W1
  ST -->|"Versioned commands/events"| WN
  MEDIA --> W1
  MEDIA --> WN
  W1 --> J1
  WN --> JN
  J1 -->|"journal evidence"| ST
  JN -->|"journal evidence"| ST
  SUP["SCM / launchd / systemd"] -.->|"supervise"| ST


  W1 --> HLS[("Generated HLS tree")]
  WN --> HLS
  subgraph PUBLIC["Public viewer boundary"]
    VIEW["Static HTTPS viewer<br/>web/portal"]
  end
  HLS --> VIEW
```

## Data and trust boundaries

| Boundary | Data | Trust rule |
|---|---|---|
| Admin loopback | Operator API, configuration, schedule, durable state | Authenticated loopback admin only; `stationd` is sole SQLite writer; operator UI is not exposed to viewers. |
| Worker local | Versioned commands/events, GStreamer graph, per-channel as-run journal | Platform-protected local transport; workers own journals and graphs; no direct SQLite writes. |
| Managed media | Content-addressed media | Immutable after store; station-storage owns persistence; workers read only. |
| Public viewer | Rolling HLS playlists and `.ts` segments | Independent static HTTPS site with embedded `hls.js` recovery; no `stationd` or admin access; only the generated viewer tree is exposed. |
| Secrets | Stream-key URLs | Windows: machine DPAPI. macOS uses a protected credential file. Ubuntu loads the protected credential through systemd. On both platforms the service uses AES-GCM. |

Cable output uses configured MPEG-TS over UDP or SRT. A bounded nonblocking handoff feeds an optional RTMP or RTMPS publisher. The portal branch writes rolling HLS playlists and `.ts` segments; those artifacts are served by the separate static HTTPS viewer site.

## On-air signal flow

Each channel's persistent graph holds the currently selected source: a scheduled Ready asset, a live source after a confirmed take, or the configured fallback. Arming alone leaves the active source in place. The graph processes video and audio against the channel clock.

```mermaid
flowchart LR
  READY["Scheduled Ready asset<br/>recorded, managed media"] --> SEL{"Per-channel source selector"}
  LIVE["Live source after confirmed take"] --> SEL
  FALL["Configured fallback"] --> SEL
  SEL --> AV["Timed video/audio path<br/>channel clock"]
  AV --> CABLE["Cable MPEG-TS<br/>UDP or SRT"]
  AV -->|"bounded non-blocking handoff"| RTMP["Optional RTMP/RTMPS publisher"]
  AV --> HLS["Rolling HLS playlists and .ts segments"]
  HLS --> SITE["Independent static HTTPS viewer<br/>hls.js recovery"]
```

## Scheduled take acknowledgement sequence

An operator drafts and commits a Ready asset under the scheduling rules. The scheduler validates and dispatches `TakeScheduledAsset`. The worker changes the active source and emits `ScheduledAssetOnAir`. The station validates the exact binding and commits the acknowledgement. Receiver playback is verified separately.

```mermaid
sequenceDiagram
  participant OP as Operator UI
  participant ST as stationd
  participant DB as SQLite WAL
  participant WK as channel-worker
  OP->>ST: Commit draft with Ready asset
  ST->>ST: Validate committed revision when due
  ST->>WK: TakeScheduledAsset
  WK->>WK: Switch active source
  WK-->>ST: ScheduledAssetOnAir
  ST->>ST: Validate exact binding
  ST->>DB: Persist acknowledgement
```

## Worker restart, journal, and reconciliation

Journal state persists across worker and station restarts. The station reconciles journal evidence against SQLite and reestablishes eligible sources, with per-channel failure and recovery supervised.

```mermaid
flowchart TD
  A["Worker exit or station restart detected"] --> B["Supervisor restarts channel-worker"]
  B --> C["Worker opens per-channel as-run journal"]
  C --> D["stationd reads journal evidence"]
  D --> E{"Journal agrees with SQLite durable state?"}
  E -- "yes" --> F["Reestablish eligible source and resume"]
  E -- "no or ambiguous" --> G["Reconcile eligible source from journal and SQLite"]
  G --> H{"Reconciled?"}
  H -- "yes" --> F
  H -- "no" --> I["Mark channel failed; surface failure; supervised retry"]
  F --> J["Project evidence into SQLite; continue"]
```

## Source directories

- `bins/stationd` — daemon, operator API, scheduler, SQLite writer.
- `bins/channel-worker` — supervised worker and graph owner.
- `crates/station-media-engine` — media graph and output plumbing.
- `crates/station-storage` — content-addressed store and journal persistence.
- `web/operator` — loopback React/TypeScript operator UI.
- `web/portal` — viewer surface, see [portal README](../../web/portal/README.md).
- `installer` — SCM, launchd, and systemd packaging.

## Decisions and evidence

- [ADR 0001 — media worker protocol](../adr/0001-media-worker-protocol.md)
- [ADR 0002 — durable channel journal](../adr/0002-durable-channel-journal.md)
- [ADR 0004 — persistent GStreamer graph](../adr/0004-persistent-gstreamer-graph.md)
- [ADR 0009 — acknowledged schedule dispatch](../adr/0009-acknowledged-schedule-dispatch.md)
- [ADR 0024 — protected worker bootstrap](../adr/0024-protected-worker-bootstrap.md)

## Testing and limits

Evaluation test tiers are 3, 6, and 9 for this CivicCast Lite v0.1.0.12 prerelease; see the [README](../../README.md) for results and open checks. The measured capacity depends on the selected media and outputs, host, and encoder.

See the [architecture reference at the prerelease commit](https://github.com/scottconverse/civiccast-lite/blob/ebffd381caa0ba9f7d97ff2e750189b39e8346e3/docs/architecture/overview.md) for inherited subsystem detail and the original design record.

# CivicCast Lab: source preservation and handoff

Owner: Scott Converse. Separation date: November 20, 2026.

Scott authorized separating the experimental Rust station into this private repo.

## Provenance

- Source repository: https://github.com/townlight/station
- Pre-separation main: `85774af3505ac9947e10d7b5e484ed25293ef671`
- Original Rust root commit: `0493b61d2f7bc1f1c5cd186c09ced263cfff49c3`
- August handoff: `871ec344c4d867e430c6c026fd00b9f3e67b9d86`
- All original branches and tags were copied before any current-source removal.
- Rust source, Cargo manifests/lock, operator UI, installer, scripts, harnesses,
  architecture, capability ledger and dated proof records are preserved.
- Separation changes organization and documentation, not application behavior.
  Internal crate/service identifiers stay unchanged for reproducibility.

Original history contains the later Python recorder; that recorder stays in
Townlight and is absent from Lab's current tree. Copied historical tags identify
their original contents; they are not CivicCast Lab releases.

## Status and known failures

Development is paused. This is not the current CivicCast product. Inherited
PROJECT_STATUS.md and CAPABILITIES.md retain their original assessment and dates.

Windows run https://github.com/townlight/station/actions/runs/44679958123 failed:

- `station-media-engine --test persistent_udp`: capture paths, fallback input,
  sustained audio.
- `station-media-journal --lib`: generation-manifest paths and multiprocess cases.
- `station-media-journal --test durable_journal`: compaction, replay, corruption.
- `station-storage --test asrun_epg_authority`: compaction and projections.

Original issue: https://github.com/townlight/station/issues/29 . Diagnose these
specific failures without weakening assertions or rerunning unrelated passed work.
No installer, runtime or full suite was executed for this repository separation.
Actions are disabled so the preservation push does not launch historical jobs.

## Possible Mac exploration

Scott expressed interest in assessing a Mac version later. No port is implemented
or authorized by separation. Assess Windows services, named pipes, Win32 process
and filesystem APIs, native media packaging and installer bindings before proposing
one. Rust alone does not establish portability. Keep CivicCast and Townlight's
recorder independent.

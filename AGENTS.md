# CivicCast Lite engineering boundaries

- CivicCast Lite is the public Rust broadcast station project. Read SEPARATION.md for provenance.
- Do not resume development, port to Mac, build/sign/publish installers or enable
  automated jobs without a Scott assignment covering that work.
- Preserve source history and dated evidence. Contracts and inherited receipts
  do not establish a complete installed station.
- Keep this separate from CivicCast production and the Townlight Python recorder.
- Work headlessly and preserve unrelated source, data, credentials and processes.
- Run affected meaningful checks; reuse unaffected evidence.
- On Windows, use scripts/invoke-cargo.ps1 from the workspace root for Cargo.
  Do not run raw Cargo, direct test executables or target artifacts: missing DLLs
  can open persistent modal dialogs. Preserve GStreamer and containment boundaries.
- Internal crate/service identifiers are intentionally unchanged. Renaming and
  porting are not part of repository separation.

# Windows Rust kernel CI

The separate Rust workspace is tested on Windows using the repository-required Cargo launcher and explicit GStreamer runtime/headers. The official 1.28.6 download is SHA-256 checked. This job does not build or publish the Suite MSI and does not replace its pinned beta-1 Python service.

The complete Rust tests run serially on the hosted runner. HTTP fixtures allow 30 seconds for fresh Windows database/bootstrap work; the stalled-request test establishes readiness first and retains its five-second response timeout. Embedded-console checks use the current Townlight spelling. These are test-fixture corrections, not changes to the shipped Station service.

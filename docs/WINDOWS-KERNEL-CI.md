# Windows Rust kernel CI

The separate Rust workspace is tested on Windows using the repository-required Cargo launcher and explicit GStreamer runtime/headers. The official 1.28.6 download is SHA-256 checked. This job does not build or publish the Suite MSI and does not replace its pinned beta-1 Python service.

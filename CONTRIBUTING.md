# Contributing

CivicCast Lite was developed in small, executable vertical slices.

## Setup

1. Install Rust 1.97.1 or newer on Windows.
2. Clone the repository.
3. Read [the architecture](docs/architecture/overview.md) and [project history](SEPARATION.md).

## Change contract

- Write a failing behavior test before changing logic or a public interface.
- Confirm the test fails for the intended assertion.
- Implement the smallest passing change, then refactor under green tests.
- Keep station authority in the control plane and media timing in channel workers.
- Never commit secrets, station data, credentials, signing material, or generated database files.
- Update operator, API, or architecture documentation in the same change as behavior.

For authorized Windows Cargo work, use only the pinned wrapper from the workspace root:

```powershell
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File scripts/invoke-cargo.ps1 test --locked -p <affected-package>
```

Do not invoke raw Cargo or direct test binaries; native loader errors can open
modal dialogs. Start with the named failures in SEPARATION.md and reuse other evidence.

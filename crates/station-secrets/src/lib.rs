#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
pub use unix::{protect_local_machine, unprotect_local_machine};
#[cfg(windows)]
pub use windows::{protect_local_machine, unprotect_local_machine};

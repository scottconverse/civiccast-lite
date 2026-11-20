//! Windows transport for the per-channel supervisor and worker.
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub(crate) use station_windows_ipc::{ContainedProcess, ProcessContainment};
pub use station_windows_ipc::{IpcError, PipeStream};
use station_windows_ipc::{PipeServer, launch_contained_process};

use crate::SupervisorError;

static PIPE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(crate) fn connect_child(
    executable: &Path,
    worker_id: &str,
    channel_id: &str,
    journal_path: &Path,
    started: Instant,
    timeout: Duration,
) -> Result<(ContainedProcess, ProcessContainment, PipeStream), SupervisorError> {
    let suffix = format!(
        "worker-{}-{}",
        std::process::id(),
        PIPE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let server = PipeServer::bind(&suffix).map_err(SupervisorError::Ipc)?;
    let journal_path = journal_path.to_string_lossy();
    let (child, containment) = launch_contained_process(
        executable,
        &[worker_id, channel_id, &journal_path, server.name()],
    )
    .map_err(SupervisorError::Ipc)?;
    match server.accept_timeout(timeout.saturating_sub(started.elapsed())) {
        Ok(pipe) => Ok((child, containment, pipe)),
        Err(error) => Err(abandon_launch(
            child,
            containment,
            SupervisorError::Ipc(error),
        )),
    }
}

pub(super) fn abandon_launch(
    child: ContainedProcess,
    containment: ProcessContainment,
    error: SupervisorError,
) -> SupervisorError {
    // Releasing the kill-on-close job is a bounded kernel request. The direct handle is
    // then dropped without waiting, so every failed launch path returns even
    // if Windows is slow to report process exit.
    drop(containment);
    drop(child);
    error
}

pub(super) fn request_termination(child: &ContainedProcess) -> Result<(), SupervisorError> {
    child.terminate().map_err(SupervisorError::Ipc)
}

pub(super) fn terminate_bounded(
    child: &ContainedProcess,
    containment: Option<ProcessContainment>,
    timeout: Duration,
) -> Result<(), SupervisorError> {
    // Close containment first. This covers descendants that direct termination
    // cannot reach and removes the need for an unbounded detached reaper.
    let job_close_requested_termination = containment.is_some();
    drop(containment);
    match child.try_wait().map_err(SupervisorError::Ipc)? {
        Some(_) => return Ok(()),
        None => match request_termination(child) {
            Ok(()) => {}
            // Closing a job may already have placed its direct process into
            // termination before the immediate zero-time wait observes the
            // signaled state. The owned process handle cannot otherwise be
            // access-denied, so continue the bounded observation in this one
            // proven-contained case.
            Err(SupervisorError::Ipc(IpcError::Os { code: 5, .. }))
                if job_close_requested_termination => {}
            Err(error) => return Err(error),
        },
    }

    let started = Instant::now();
    loop {
        if child.try_wait().map_err(SupervisorError::Ipc)?.is_some() {
            return Ok(());
        }
        if started.elapsed() >= timeout {
            return Err(SupervisorError::ProcessExitTimedOut);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

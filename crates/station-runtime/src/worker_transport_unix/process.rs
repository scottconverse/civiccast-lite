use super::{ENDPOINT, IpcError, PipeStream, RETRY};
use crate::SupervisorError;
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct OwnedGroup {
    child: Option<Child>,
    status: Option<u32>,
}

impl OwnedGroup {
    fn kill_group(&self) -> io::Result<()> {
        if let Some(child) = &self.child {
            // The group leader is still our unreaped child, so its PID cannot
            // have been reassigned to an unrelated process group.
            if unsafe { libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL) } == -1 {
                let e = io::Error::last_os_error();
                #[cfg(target_os = "macos")]
                if e.raw_os_error() == Some(libc::EPERM) && lone_exited_group_leader(child.id()) {
                    return Ok(());
                }
                if e.raw_os_error() != Some(libc::ESRCH) {
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    fn observe(&mut self) -> io::Result<Option<u32>> {
        let Some(child) = self.child.as_ref() else {
            return Ok(self.status);
        };
        // Observe exit without freeing the leader PID until descendants have
        // received termination. Child::try_wait alone would reap too early.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let ret = unsafe {
            libc::waitid(
                libc::P_PID,
                child.id(),
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        if unsafe { info.si_pid() } == 0 {
            return Ok(None);
        }
        self.kill_group()?;
        let status = self.child.as_mut().unwrap().wait()?;
        let code = status
            .code()
            .unwrap_or_else(|| 128 + status.signal().unwrap_or(0)) as u32;
        self.status = Some(code);
        self.child = None;
        Ok(Some(code))
    }
}

// Darwin reports EPERM when the only member of a process group is a zombie.
// Keep the child unreaped throughout both observations so its ID stays owned.
#[cfg(target_os = "macos")]
fn lone_exited_group_leader(pid: u32) -> bool {
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    let observed = unsafe {
        libc::waitid(
            libc::P_PID,
            pid,
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    if observed != 0 || unsafe { info.si_pid() } != pid as libc::pid_t {
        return false;
    }
    const PROC_PGRP_ONLY: u32 = 2; // <sys/proc_info.h>
    let mut members = [0 as libc::pid_t; 8];
    let bytes = unsafe {
        libc::proc_listpids(
            PROC_PGRP_ONLY,
            pid,
            members.as_mut_ptr().cast(),
            std::mem::size_of_val(&members) as libc::c_int,
        )
    };
    bytes == std::mem::size_of::<libc::pid_t>() as libc::c_int && members[0] == pid as libc::pid_t
}

impl Drop for OwnedGroup {
    fn drop(&mut self) {
        let _ = self.kill_group();
        if let Some(mut child) = self.child.take() {
            // Drop can occur while unwinding a protocol failure. Let the OS
            // deliver termination without blocking the orchestration loop.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
}

pub(crate) struct ContainedProcess(Arc<Mutex<OwnedGroup>>);
pub(crate) struct ProcessContainment(Arc<Mutex<OwnedGroup>>);

impl Drop for ProcessContainment {
    fn drop(&mut self) {
        let _ = self
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .kill_group();
    }
}

impl ContainedProcess {
    pub(crate) fn try_wait(&self) -> Result<Option<u32>, IpcError> {
        Ok(self.0.lock().unwrap_or_else(|e| e.into_inner()).observe()?)
    }
}

pub(crate) fn connect_child(
    executable: &Path,
    worker_id: &str,
    channel_id: &str,
    journal_path: &Path,
    _: Instant,
    _: Duration,
) -> Result<(ContainedProcess, ProcessContainment, PipeStream), SupervisorError> {
    let (parent, worker) = UnixStream::pair().map_err(|e| SupervisorError::Ipc(e.into()))?;
    let pipe = PipeStream::new(parent).map_err(SupervisorError::Ipc)?;
    let child = Command::new(executable)
        .arg(worker_id)
        .arg(channel_id)
        .arg(journal_path)
        .arg(ENDPOINT)
        .stdin(Stdio::from(OwnedFd::from(worker)))
        .process_group(0)
        .spawn()
        .map_err(|e| SupervisorError::Ipc(e.into()))?;
    let group = Arc::new(Mutex::new(OwnedGroup {
        child: Some(child),
        status: None,
    }));
    Ok((
        ContainedProcess(group.clone()),
        ProcessContainment(group),
        pipe,
    ))
}

pub(crate) fn abandon_launch(
    child: ContainedProcess,
    containment: ProcessContainment,
    error: SupervisorError,
) -> SupervisorError {
    drop(containment);
    drop(child);
    error
}

pub(crate) fn request_termination(child: &ContainedProcess) -> Result<(), SupervisorError> {
    child
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .kill_group()
        .map_err(|e| SupervisorError::Ipc(e.into()))
}

pub(crate) fn terminate_bounded(
    child: &ContainedProcess,
    containment: Option<ProcessContainment>,
    timeout: Duration,
) -> Result<(), SupervisorError> {
    drop(containment);
    request_termination(child)?;
    let started = Instant::now();
    loop {
        if child.try_wait().map_err(SupervisorError::Ipc)?.is_some() {
            return Ok(());
        }
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(SupervisorError::ProcessExitTimedOut);
        }
        std::thread::sleep(RETRY.min(remaining));
    }
}

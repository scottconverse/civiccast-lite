//! Each worker receives one end of a private socket pair at exec. There is no
//! filesystem listener to replace, stale socket to unlink, or public port.
#[path = "worker_transport_unix/process.rs"]
mod process;
pub(crate) use process::{
    ContainedProcess, ProcessContainment, abandon_launch, connect_child, request_termination,
    terminate_bounded,
};

use std::io::{self, Read, Write};
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

const ENDPOINT: &str = "inherited-worker-socket";
const RETRY: Duration = Duration::from_millis(2);

#[derive(Debug)]
pub enum IpcError {
    Closed,
    TimedOut { operation: &'static str },
    Os(io::Error),
}

impl From<io::Error> for IpcError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::BrokenPipe
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::UnexpectedEof => Self::Closed,
            _ => Self::Os(error),
        }
    }
}

pub struct PipeStream {
    socket: UnixStream,
    // A timed read can consume part of a frame. Retain those bytes until a
    // complete read succeeds, including when the caller polls with zero timeout.
    pending: Vec<u8>,
}

impl PipeStream {
    fn new(socket: UnixStream) -> Result<Self, IpcError> {
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            pending: Vec::new(),
        })
    }

    pub fn connect(name: &str) -> Result<Self, IpcError> {
        if name != ENDPOINT {
            return Err(
                io::Error::new(io::ErrorKind::InvalidInput, "invalid worker endpoint").into(),
            );
        }
        let stdin = io::stdin();
        let socket = UnixStream::from(stdin.as_fd().try_clone_to_owned()?);
        socket.peer_addr()?;
        verify_peer(&socket)?;
        // Keep fd 0 out of future execs by the worker. Its Rust-owned
        // duplicate above already has close-on-exec set.
        if unsafe { libc::fcntl(stdin.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
            return Err(io::Error::last_os_error().into());
        }
        Self::new(socket)
    }

    pub fn peek_exact_timeout(
        &mut self,
        out: &mut [u8],
        timeout: Duration,
    ) -> Result<(), IpcError> {
        if out.len() > station_media_protocol::MAX_FRAME_BYTES + 4 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "oversize frame").into());
        }
        let started = Instant::now();
        let mut chunk = [0u8; 8192];
        while self.pending.len() < out.len() {
            let need = chunk.len().min(out.len() - self.pending.len());
            match self.socket.read(&mut chunk[..need]) {
                Ok(0) => return Err(IpcError::Closed),
                Ok(n) => self.pending.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => (),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => wait(started, timeout, "read")?,
                Err(e) => return Err(e.into()),
            }
        }
        out.copy_from_slice(&self.pending[..out.len()]);
        Ok(())
    }

    pub fn read_exact_timeout(
        &mut self,
        out: &mut [u8],
        timeout: Duration,
    ) -> Result<(), IpcError> {
        self.peek_exact_timeout(out, timeout)?;
        self.pending.drain(..out.len());
        Ok(())
    }

    pub fn write_all_timeout(
        &mut self,
        mut data: &[u8],
        timeout: Duration,
    ) -> Result<(), IpcError> {
        let started = Instant::now();
        while !data.is_empty() {
            match self.socket.write(data) {
                Ok(0) => return Err(IpcError::Closed),
                Ok(n) => data = &data[n..],
                Err(e) if e.kind() == io::ErrorKind::Interrupted => (),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => wait(started, timeout, "write")?,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
}

fn wait(started: Instant, timeout: Duration, operation: &'static str) -> Result<(), IpcError> {
    let remaining = timeout.saturating_sub(started.elapsed());
    if remaining.is_zero() {
        return Err(IpcError::TimedOut { operation });
    }
    std::thread::sleep(RETRY.min(remaining));
    Ok(())
}

impl Read for PipeStream {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if !self.pending.is_empty() {
            let n = self.pending.len().min(out.len());
            out[..n].copy_from_slice(&self.pending[..n]);
            self.pending.drain(..n);
            return Ok(n);
        }
        loop {
            match self.socket.read(out) {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(RETRY),
                result => return result,
            }
        }
    }
}

fn verify_peer(socket: &UnixStream) -> io::Result<()> {
    // The kernel records the socketpair creator's credentials. The parent
    // retains its end and passes the other exclusively to the child at spawn.
    #[cfg(target_os = "linux")]
    let trusted = unsafe {
        let mut cred: libc::ucred = std::mem::zeroed();
        let mut len = std::mem::size_of_val(&cred) as libc::socklen_t;
        if libc::getsockopt(
            socket.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut cred as *mut libc::ucred).cast(),
            &mut len,
        ) == -1
        {
            return Err(io::Error::last_os_error());
        }
        cred.uid == libc::geteuid() && cred.pid == libc::getppid()
    };
    #[cfg(target_os = "macos")]
    let trusted = unsafe {
        let (mut uid, mut gid) = (0, 0);
        if libc::getpeereid(socket.as_raw_fd(), &mut uid, &mut gid) == -1 {
            return Err(io::Error::last_os_error());
        }
        uid == libc::geteuid()
    };
    if trusted {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unexpected worker peer",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::OpenOptionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn partial_frame_survives_a_read_timeout_and_peek() {
        let (socket, mut sender) = UnixStream::pair().unwrap();
        let mut pipe = PipeStream::new(socket).unwrap();
        sender.write_all(b"ab").unwrap();
        let mut bytes = [0; 4];
        assert!(matches!(
            pipe.read_exact_timeout(&mut bytes, Duration::ZERO),
            Err(IpcError::TimedOut { .. })
        ));
        sender.write_all(b"cd").unwrap();
        pipe.peek_exact_timeout(&mut bytes, Duration::ZERO).unwrap();
        pipe.read_exact_timeout(&mut bytes, Duration::ZERO).unwrap();
        assert_eq!(&bytes, b"abcd");
        drop(sender);
        assert!(matches!(
            pipe.peek_exact_timeout(&mut bytes, Duration::ZERO),
            Err(IpcError::Closed)
        ));
    }

    #[test]
    fn child_endpoint() {
        if std::env::var_os("CIVICCAST_IPC_TEST_CHILD").is_none() {
            return;
        }
        let mut peer = PipeStream::connect(ENDPOINT).unwrap();
        let mut challenge = [0; 4];
        peer.read_exact_timeout(&mut challenge, Duration::from_secs(3))
            .unwrap();
        assert_eq!(&challenge, b"ping");
        // Keep a witness socket descriptor in a same-group descendant. EOF
        // can only arrive when the supervisor terminates that descendant.
        let witness = io::stdin().as_fd().try_clone_to_owned().unwrap();
        let _descendant = std::process::Command::new("sleep")
            .arg("30")
            .stdin(std::process::Stdio::from(witness))
            .spawn()
            .unwrap();
        peer.write_all_timeout(b"pong", Duration::from_secs(3))
            .unwrap();
    }

    #[test]
    fn private_child_socket_handshake_and_group_cleanup() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("lab-worker-{}-{nonce}", std::process::id()));
        std::os::unix::fs::DirBuilderExt::mode(&mut fs::DirBuilder::new(), 0o700)
            .create(&dir)
            .unwrap();
        let script = dir.join("launch");
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o700)
            .open(&script)
            .unwrap();
        // The descendant keeps fd 0 open. EOF after the direct child exits
        // demonstrates that supervision also terminated its remaining group.
        file.write_all(
            b"#!/bin/sh\nexport CIVICCAST_IPC_TEST_CHILD=1\nexec \"$1\" --exact \"$2\"\n",
        )
        .unwrap();
        drop(file);
        let (child, containment, mut pipe) = connect_child(
            &script,
            std::env::current_exe().unwrap().to_str().unwrap(),
            "worker_transport::tests::child_endpoint",
            &dir,
            Instant::now(),
            Duration::from_secs(3),
        )
        .unwrap();
        pipe.write_all_timeout(b"ping", Duration::from_secs(3))
            .unwrap();
        let mut reply = [0; 4];
        pipe.read_exact_timeout(&mut reply, Duration::from_secs(3))
            .unwrap();
        assert_eq!(&reply, b"pong");
        assert_eq!(
            crate::wait_for_exit(&child, Duration::from_secs(3)).unwrap(),
            0
        );
        assert!(matches!(
            pipe.read_exact_timeout(&mut reply, Duration::from_secs(3)),
            Err(IpcError::Closed)
        ));
        drop(containment);
        fs::remove_file(script).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}

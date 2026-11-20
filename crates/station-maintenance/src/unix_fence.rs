use super::{
    InstallAuthorityFence, authority_root_identity, canonical_install_authority_root,
    open_authority_root, sha256_lowercase,
};
use rustix::fs::{Mode, OFlags, openat};
use std::{
    fs::File,
    os::unix::fs::MetadataExt,
    path::Path,
    time::{Duration, Instant},
};

pub(super) struct Lease(File);

impl Drop for Lease {
    fn drop(&mut self) {
        // A process exit before release leaves a durable dirty marker.
        if self.0.set_len(0).is_ok() {
            let _ = self.0.sync_all();
        }
    }
}

pub(super) fn acquire_authority_fence_for<F>(
    data: &Path,
    timeout: Duration,
    name_for: F,
) -> Result<InstallAuthorityFence, String>
where
    F: FnOnce(&Path) -> Result<String, String>,
{
    if !data.is_absolute() {
        return Err("installation authority requires an absolute data directory".into());
    }
    let path = canonical_install_authority_root(data)?;
    let root = open_authority_root(&path)?;
    let identity = authority_root_identity(&root)?;
    let name = format!(
        ".civiccast-fence-{}",
        sha256_lowercase(name_for(&path)?.as_bytes())
    );
    let attempt = || -> std::io::Result<(File, bool)> {
        let dir = root.metadata()?;
        if dir.mode() & 0o022 != 0 {
            return Err(std::io::Error::other(
                "station data directory allows untrusted writes",
            ));
        }
        let flags = OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
        let file = match openat(
            &root,
            name.as_str(),
            flags | OFlags::CREATE | OFlags::EXCL,
            Mode::RUSR | Mode::WUSR,
        ) {
            Ok(fd) => {
                if rustix::process::geteuid().is_root() {
                    rustix::fs::fchown(&fd, Some(rustix::process::Uid::from_raw(dir.uid())), None)?;
                }
                root.sync_all()?;
                File::from(fd)
            }
            Err(rustix::io::Errno::EXIST) => {
                File::from(openat(&root, name.as_str(), flags, Mode::empty())?)
            }
            Err(e) => return Err(e.into()),
        };
        let started = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => break,
                Err(std::fs::TryLockError::WouldBlock) if started.elapsed() < timeout => {
                    std::thread::sleep(
                        Duration::from_millis(10).min(timeout.saturating_sub(started.elapsed())),
                    );
                }
                Err(e) => return Err(e.into()),
            }
        }
        let meta = file.metadata()?;
        if !meta.is_file()
            || meta.nlink() != 1
            || meta.mode() & 0o077 != 0
            || meta.uid() != dir.uid()
        {
            return Err(std::io::Error::other("invalid station authority lock file"));
        }
        let abandoned = meta.len() != 0;
        file.set_len(1)?;
        file.sync_all()?;
        Ok((file, abandoned))
    };
    let (file, abandoned) =
        attempt().map_err(|e| format!("acquire station authority fence: {e}"))?;
    Ok(InstallAuthorityFence {
        _lease: Lease(file),
        abandoned,
        authority_root: path,
        authority_root_identity: identity,
        _authority_root_handle: root,
        application_root: None,
    })
}

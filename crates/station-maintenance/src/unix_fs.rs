use super::StationDatabaseIdentity;
use rustix::fs::{Mode, OFlags, RenameFlags, open, openat, renameat_with};
use std::{fs::File, io, os::unix::fs::MetadataExt, path::Path};

const DIR_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

pub(super) fn open_authority_root(path: &Path) -> Result<File, String> {
    open(path, DIR_FLAGS, Mode::empty())
        .map(File::from)
        .map_err(|e| format!("open authority directory: {e}"))
}

pub(super) fn open_database(root: &File, leaf: &std::ffi::OsStr) -> io::Result<File> {
    let file = File::from(openat(
        root,
        leaf,
        OFlags::RDWR | OFlags::CREATE | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::RUSR | Mode::WUSR,
    )?);
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(io::Error::other(
            "database must be a regular file with a single directory entry",
        ));
    }
    Ok(file)
}

pub(super) fn authority_root_identity(file: &File) -> Result<(u64, u64), String> {
    let m = file
        .metadata()
        .map_err(|e| format!("stat authority directory: {e}"))?;
    Ok((m.dev(), m.ino()))
}

pub(super) fn station_database_identity(file: &File) -> Result<StationDatabaseIdentity, String> {
    let (dev, ino) = authority_root_identity(file)?;
    let mut file_id = [0; 16];
    file_id[..8].copy_from_slice(&ino.to_le_bytes());
    Ok(StationDatabaseIdentity {
        volume_serial_number: dev,
        file_id,
    })
}

fn parent_and_leaf(path: &Path) -> io::Result<(File, &std::ffi::OsStr)> {
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("missing entry name"))?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    Ok((File::from(open(parent, DIR_FLAGS, Mode::empty())?), name))
}

fn publish(source: &Path, destination: &Path, replace: bool) -> io::Result<()> {
    let (from, old_name) = parent_and_leaf(source)?;
    let (to, new_name) = parent_and_leaf(destination)?;
    if replace {
        rustix::fs::renameat(&from, old_name, &to, new_name)?;
    } else {
        renameat_with(&from, old_name, &to, new_name, RenameFlags::NOREPLACE)?;
    }
    to.sync_all()?;
    from.sync_all()
}

pub(super) fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    publish(source, destination, true)
}

pub(super) fn move_tree(source: &Path, destination: &Path) -> io::Result<()> {
    publish(source, destination, false)
}

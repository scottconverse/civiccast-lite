//! Capture leaf names are generated from validated UUIDs. Keep the directory
//! authority alive until publication (or abort cleanup) finishes.
use std::fs::File;
use std::io;
use std::path::Path;

#[cfg(windows)]
pub(super) struct CaptureDirectory {
    path: std::path::PathBuf,
    _pins: Vec<File>,
}

#[cfg(windows)]
impl CaptureDirectory {
    pub fn proof(path: &Path) -> io::Result<Self> {
        Ok(Self {
            path: path.to_owned(),
            _pins: Vec::new(),
        })
    }
    pub fn recording(path: &Path) -> io::Result<Self> {
        use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
            FILE_SHARE_WRITE,
        };
        let mut directory = Self::proof(path)?;
        for p in [path.parent().unwrap(), path] {
            let file = std::fs::OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
                .open(p)?;
            let meta = file.metadata()?;
            if !meta.is_dir() || meta.file_attributes() & 0x0400 != 0 {
                return Err(io::Error::other("invalid recording directory"));
            }
            directory._pins.push(file);
        }
        Ok(directory)
    }
    pub fn exists(&self, name: &str) -> io::Result<bool> {
        self.path.join(name).try_exists()
    }
    pub fn create(&self, name: &str) -> io::Result<File> {
        File::create_new(self.path.join(name))
    }
    pub fn remove(&self, name: &str) -> io::Result<()> {
        std::fs::remove_file(self.path.join(name))
    }
    pub fn publish(&self, part: &str, ready: &str) -> io::Result<()> {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::MoveFileExW;
        let wide = |name: &str| {
            self.path
                .join(name)
                .as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<_>>()
        };
        let part = wide(part);
        let ready = wide(ready);
        // Same-directory atomic rename. Omitting REPLACE_EXISTING preserves
        // a completed artifact even if another producer wins the name race.
        if unsafe { MoveFileExW(part.as_ptr(), ready.as_ptr(), 0) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    pub fn regular_names(&self) -> io::Result<Vec<String>> {
        let mut names = Vec::new();
        for entry in std::fs::read_dir(&self.path)? {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && let Ok(name) = entry.file_name().into_string()
            {
                names.push(name);
            }
        }
        Ok(names)
    }
}

#[cfg(unix)]
pub(super) struct CaptureDirectory(File);

#[cfg(unix)]
impl CaptureDirectory {
    pub fn proof(path: &Path) -> io::Result<Self> {
        use rustix::fs::{Mode, OFlags, open};
        Ok(Self(
            open(
                path,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )?
            .into(),
        ))
    }
    pub fn recording(path: &Path) -> io::Result<Self> {
        use rustix::fs::{Mode, OFlags, openat};
        let parent = Self::proof(path.parent().unwrap())?;
        Ok(Self(
            openat(
                &parent.0,
                path.file_name().unwrap(),
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )?
            .into(),
        ))
    }
    pub fn exists(&self, name: &str) -> io::Result<bool> {
        match rustix::fs::statat(&self.0, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
            Ok(_) => Ok(true),
            Err(rustix::io::Errno::NOENT) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
    pub fn create(&self, name: &str) -> io::Result<File> {
        use rustix::fs::{Mode, OFlags, openat};
        Ok(openat(
            &self.0,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )?
        .into())
    }
    pub fn remove(&self, name: &str) -> io::Result<()> {
        Ok(rustix::fs::unlinkat(
            &self.0,
            name,
            rustix::fs::AtFlags::empty(),
        )?)
    }
    pub fn publish(&self, part: &str, ready: &str) -> io::Result<()> {
        rustix::fs::renameat_with(
            &self.0,
            part,
            &self.0,
            ready,
            rustix::fs::RenameFlags::NOREPLACE,
        )?;
        self.0.sync_all()
    }
    pub fn regular_names(&self) -> io::Result<Vec<String>> {
        use rustix::fs::{AtFlags, Dir, FileType, statat};
        let mut names = Vec::new();
        for entry in Dir::read_from(&self.0)? {
            let entry = entry?;
            let name = entry.file_name();
            if name.to_bytes().ends_with(b".ts.part")
                && FileType::from_raw_mode(
                    statat(&self.0, name, AtFlags::SYMLINK_NOFOLLOW)?.st_mode,
                ) == FileType::RegularFile
                && let Ok(name) = name.to_str()
            {
                names.push(name.to_owned());
            }
        }
        Ok(names)
    }
}

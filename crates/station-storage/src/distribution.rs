use super::StationStore;
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

impl StationStore {
    fn distribution_path(&self, channel: &str) -> Result<PathBuf, String> {
        if !super::is_canonical_uuid(channel)
            || !self
                .list_channels()?
                .iter()
                .any(|c| c.channel_id == channel)
        {
            return Err("unknown station channel".into());
        }
        let dir = self._data_root.join("distribution");
        match fs::symlink_metadata(&dir) {
            Ok(meta) if super::is_link_metadata(&meta) || !meta.is_dir() => {
                return Err("invalid distribution directory".into());
            }
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                return Err(error.to_string());
            }
            _ => {}
        }
        Ok(dir.join(format!("{channel}.rtmp")))
    }

    pub fn on_air_rtmp_configured(&self, channel: &str) -> Result<bool, String> {
        self.distribution_path(channel)?
            .try_exists()
            .map_err(|e| e.to_string())
    }

    pub fn distribution_progress(
        &self,
        channel: &str,
    ) -> Result<(Option<u64>, Option<u64>), String> {
        fn updated(path: PathBuf) -> Option<u64> {
            fs::metadata(path)
                .ok()?
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs())
        }
        let rtmp = self.distribution_path(channel)?;
        let progress = if rtmp.is_file() {
            updated(rtmp.with_extension("progress"))
        } else {
            None
        };
        let playlist = station_publishing::portal_root(&self._data_root)
            .ok()
            .flatten()
            .and_then(|root| updated(root.join(channel).join("live.m3u8")));
        Ok((progress, playlist))
    }

    /// Store only a protected destination. Credentials never enter the API response or as-run journal.
    pub fn set_on_air_rtmp(&self, channel: &str, destination: Option<&str>) -> Result<(), String> {
        let path = self.distribution_path(channel)?;
        let dir = path.parent().ok_or("missing distribution directory")?;
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let lock = fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join("assignment.lock"))
            .map_err(|e| e.to_string())?;
        lock.lock().map_err(|e| e.to_string())?;
        if let Some(url) = destination {
            station_publishing::validate_on_air_rtmp(url)?;
            for other in self
                .list_channels()?
                .into_iter()
                .filter(|c| c.channel_id != channel)
            {
                let assigned = dir.join(format!("{}.rtmp", other.channel_id));
                let meta = match fs::symlink_metadata(&assigned) {
                    Ok(meta) => meta,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(e) => return Err(e.to_string()),
                };
                if super::is_link_metadata(&meta) || !meta.is_file() || meta.len() > 16384 {
                    return Err("invalid protected station destination".into());
                }
                let mut bytes = Vec::new();
                fs::File::open(&assigned)
                    .and_then(|f| f.take(16385).read_to_end(&mut bytes))
                    .map_err(|e| e.to_string())?;
                let clear =
                    zeroize::Zeroizing::new(station_secrets::unprotect_local_machine(&bytes)?);
                let existing = std::str::from_utf8(&clear)
                    .map_err(|_| "invalid protected station destination")?;
                if station_publishing::same_on_air_destination(url, existing) {
                    return Err(format!(
                        "The streaming destination is already assigned to channel {}.",
                        other.display_name
                    ));
                }
            }
            let sealed = station_secrets::protect_local_machine(url.as_bytes())?;
            if super::is_link_metadata(&fs::symlink_metadata(dir).map_err(|e| e.to_string())?) {
                return Err("invalid distribution directory".into());
            }
            let mut temporary = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
            temporary
                .write_all(&sealed)
                .and_then(|_| temporary.as_file().sync_all())
                .map_err(|e| e.to_string())?;
            temporary.persist(&path).map_err(|e| e.error.to_string())?;
            let _ = fs::remove_file(path.with_extension("progress"));
        } else {
            match fs::remove_file(&path) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(e) => return Err(e.to_string()),
            }
        }
        #[cfg(unix)]
        fs::File::open(dir)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

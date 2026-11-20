use super::StationStore;
use std::{fs, io::Write, path::PathBuf};

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

    /// Store only a protected destination. Credentials never enter the API response or as-run journal.
    pub fn set_on_air_rtmp(&self, channel: &str, destination: Option<&str>) -> Result<(), String> {
        let path = self.distribution_path(channel)?;
        let dir = path.parent().ok_or("missing distribution directory")?;
        if let Some(url) = destination {
            station_publishing::validate_on_air_rtmp(url)?;
            let sealed = station_secrets::protect_local_machine(url.as_bytes())?;
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            if super::is_link_metadata(&fs::symlink_metadata(dir).map_err(|e| e.to_string())?) {
                return Err("invalid distribution directory".into());
            }
            let mut temporary = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
            temporary
                .write_all(&sealed)
                .and_then(|_| temporary.as_file().sync_all())
                .map_err(|e| e.to_string())?;
            temporary.persist(&path).map_err(|e| e.error.to_string())?;
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

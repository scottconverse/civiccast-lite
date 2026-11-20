use station_schedule::ChannelConfiguration;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

pub(super) struct Portal {
    directory: PathBuf,
    catalog: Option<Vec<u8>>,
    installed: bool,
}

impl Portal {
    pub(super) fn open(data: &Path) -> Result<Option<Self>, String> {
        Ok(
            station_publishing::portal_root(data)?.map(|directory| Self {
                directory,
                catalog: None,
                installed: false,
            }),
        )
    }

    pub(super) fn publish(&mut self, channels: &[ChannelConfiguration]) -> Result<(), String> {
        if !self.installed {
            for (name, bytes) in [
                (
                    "hls.light.min.js",
                    include_bytes!("../../../web/portal/hls.light.min.js").as_slice(),
                ),
                (
                    "HLS-LICENSE.txt",
                    include_bytes!("../../../web/portal/HLS-LICENSE.txt").as_slice(),
                ),
                (
                    "player.js",
                    include_bytes!("../../../web/portal/player.js").as_slice(),
                ),
                (
                    "index.html",
                    include_bytes!("../../../web/portal/index.html").as_slice(),
                ),
            ] {
                self.replace(name, bytes)?;
            }
            self.installed = true;
        }
        let entries: Vec<_> = channels
            .iter()
            .filter(|c| c.enabled)
            .map(|c| serde_json::json!({"id":c.channel_id, "name":c.display_name}))
            .collect();
        let bytes = serde_json::to_vec(&entries).map_err(|e| e.to_string())?;
        if self.catalog.as_ref() != Some(&bytes) {
            self.replace("channels.json", &bytes)?;
            self.catalog = Some(bytes);
        }
        Ok(())
    }

    fn replace(&self, name: &str, content: &[u8]) -> Result<(), String> {
        let mut file =
            tempfile::NamedTempFile::new_in(&self.directory).map_err(|e| e.to_string())?;
        file.write_all(content).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.as_file()
                .set_permissions(std::fs::Permissions::from_mode(0o644))
                .map_err(|e| e.to_string())?;
        }
        file.persist(self.directory.join(name))
            .map_err(|e| e.error.to_string())?;
        Ok(())
    }
}

use super::*;
use rustix::fs::{
    AtFlags, Mode, OFlags, RenameFlags, mkdirat, open, openat, renameat_with, unlinkat,
};
use std::os::unix::fs::MetadataExt;

const DIR: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

fn directory(parent: &File, name: &str) -> std::io::Result<File> {
    Ok(File::from(openat(parent, name, DIR, Mode::empty())?))
}
fn regular(parent: &File, name: &str) -> std::io::Result<File> {
    let file = File::from(openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )?);
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::other("recording is not a regular file"));
    }
    Ok(file)
}
struct Part<'a> {
    dir: &'a File,
    name: String,
    armed: bool,
}
impl Drop for Part<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = unlinkat(self.dir, self.name.as_str(), AtFlags::empty());
        }
    }
}

pub(super) fn publish_recording_capture(
    database: &Path,
    channel: &str,
    ready: &ReadyRecordingCapture,
    cancel: &AtomicBool,
    deadline: Instant,
) -> Result<PublishedRecordingAsset, String> {
    ready.validate()?;
    check_finalization_control(cancel, deadline)?;
    let root_path = database
        .parent()
        .filter(|p| p.is_absolute())
        .ok_or("missing absolute station data directory")?;
    // Both names must be single path components before using them relative to a directory fd.
    let source_name = &ready.active.artifact_relative_path;
    for name in [channel, source_name.as_str()] {
        if Path::new(name).file_name().and_then(|n| n.to_str()) != Some(name) {
            return Err("invalid recording path component".into());
        }
    }
    let io = |e: std::io::Error| format!("recording publication: {e}");
    let root = File::from(open(root_path, DIR, Mode::empty()).map_err(|e| io(e.into()))?);
    let recordings = directory(&root, "recordings").map_err(io)?;
    let capture = directory(&recordings, channel).map_err(io)?;
    match mkdirat(&root, "media", Mode::RWXU) {
        Ok(()) => root.sync_all().map_err(io)?,
        Err(rustix::io::Errno::EXIST) => (),
        Err(e) => return Err(io(e.into())),
    }
    let media = directory(&root, "media").map_err(io)?;
    let mut source = regular(&capture, source_name).map_err(io)?;
    if source.metadata().map_err(io)?.len() != ready.size_bytes {
        return Err("recording size changed before publication".into());
    }
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|e| e.to_string())?;
    let name = format!(".recording-{:032x}.part", u128::from_le_bytes(random));
    let mut part_file = File::from(
        openat(
            &media,
            name.as_str(),
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|e| io(e.into()))?,
    );
    let mut part = Part {
        dir: &media,
        name,
        armed: true,
    };
    let mut digest = Sha256::new();
    let mut bytes = 0u64;
    let mut chunk = [0u8; RECORDING_FINALIZATION_CHUNK_BYTES];
    loop {
        check_finalization_control(cancel, deadline)?;
        let n = source.read(&mut chunk).map_err(io)?;
        if n == 0 {
            break;
        }
        bytes = bytes
            .checked_add(n as u64)
            .ok_or("recording length overflow")?;
        if bytes > ready.size_bytes {
            return Err("recording grew beyond worker evidence".into());
        }
        part_file.write_all(&chunk[..n]).map_err(io)?;
        digest.update(&chunk[..n]);
    }
    if bytes != ready.size_bytes || format!("{:x}", digest.finalize()) != ready.capture_sha256 {
        return Err("recording bytes differ from worker evidence".into());
    }
    part_file.sync_all().map_err(io)?;
    let media_path = root_path.join("media");
    let part_path = media_path.join(&part.name);
    let id = |m: std::fs::Metadata| (m.dev(), m.ino());
    let part_id = id(part_file.metadata().map_err(io)?);
    if id(fs::symlink_metadata(&part_path).map_err(io)?) != part_id {
        return Err("recording validation path changed".into());
    }
    let duration = validate_media(&part_path)
        .map_err(|e| format!("validate sealed recording media: {e:?}"))?
        .duration_ns
        / 1_000_000;
    if duration == 0 || duration.abs_diff(ready.captured_duration_ms) > 5_000 {
        return Err("recording duration differs from worker evidence".into());
    }
    check_finalization_control(cancel, deadline)?;
    let name = format!("{}.ts", ready.capture_sha256);
    let new = publish_part(&media, part.name.as_str(), name.as_str()).map_err(io)?;
    part.armed = false;
    drop(part);
    media.sync_all().map_err(io)?;
    let mut final_file = regular(&media, &name).map_err(io)?;
    let final_path = media_path.join(&name);
    let final_id = id(final_file.metadata().map_err(io)?);
    if new && final_id != part_id || id(fs::symlink_metadata(&final_path).map_err(io)?) != final_id
    {
        return Err("published recording identity changed".into());
    }
    if !new
        && hash_open_recording(&mut final_file, cancel, deadline)?
            != (ready.capture_sha256.clone(), ready.size_bytes)
    {
        return Err("existing recording disagrees with worker evidence".into());
    }
    Ok(PublishedRecordingAsset {
        asset: MediaAsset {
            asset_id: ready.capture_sha256.clone(),
            media_path: final_path.to_string_lossy().into_owned(),
            duration_ms: ready.captured_duration_ms,
            readiness: AssetReadiness::Ready,
        },
        _recording_ancestry: vec![root, recordings, capture],
        _media_ancestry: vec![media],
        _final_file: final_file,
    })
}

pub(super) fn publish_part(
    dir: &File,
    temporary: &str,
    destination: &str,
) -> std::io::Result<bool> {
    match renameat_with(dir, temporary, dir, destination, RenameFlags::NOREPLACE) {
        Ok(()) => Ok(true),
        Err(rustix::io::Errno::EXIST) => {
            unlinkat(dir, temporary, AtFlags::empty())?;
            Ok(false)
        }
        Err(e) => Err(e.into()),
    }
}

//! Each destination has a separate process, a bounded input queue and its own reconnect loop.
use gst::prelude::*;
use gstreamer as gst;
use station_media_engine::PersistentPlayout;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, RecvTimeoutError, sync_channel},
    },
    thread,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

#[derive(Clone, Copy)]
enum Destination {
    Portal,
    Rtmp,
}
impl Destination {
    fn argument(self) -> &'static str {
        match self {
            Self::Portal => "hls",
            Self::Rtmp => "rtmp",
        }
    }
}

struct Publisher {
    _subscription: station_media_engine::OnAirSubscription,
    stopping: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Drop for Publisher {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        if let Some(child) = self
            .child
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_mut()
        {
            let _ = child.kill();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(super) struct Publishers {
    _portal: Option<Publisher>,
    rtmp: Option<Publisher>,
    sealed_path: PathBuf,
    last_config: Option<Vec<u8>>,
    next_check: Instant,
}

impl Publishers {
    pub fn attach(media: &PersistentPlayout, data: &Path, channel: &str) -> Self {
        let portal = station_publishing::portal_root(data)
            .ok()
            .flatten()
            .and_then(|root| start(media, Destination::Portal, root.join(channel)));
        Self {
            _portal: portal,
            rtmp: None,
            sealed_path: data.join("distribution").join(format!("{channel}.rtmp")),
            last_config: None,
            next_check: Instant::now(),
        }
    }
    pub fn refresh(&mut self, media: &PersistentPlayout) {
        if Instant::now() < self.next_check {
            return;
        }
        self.next_check = Instant::now() + Duration::from_secs(2);
        let config = fs::File::open(&self.sealed_path).ok().and_then(|file| {
            let mut data = Vec::new();
            file.take(16385).read_to_end(&mut data).ok()?;
            (data.len() <= 16384 && !data.is_empty()).then_some(data)
        });
        if config != self.last_config || (config.is_some() && self.rtmp.is_none()) {
            self.rtmp.take();
            if config.is_some() {
                self.rtmp = start(media, Destination::Rtmp, self.sealed_path.clone());
            }
            self.last_config = config;
        }
    }
}

fn start(media: &PersistentPlayout, dest: Destination, path: PathBuf) -> Option<Publisher> {
    let (tx, rx) = sync_channel(256);
    let subscription = media.subscribe_on_air(tx).ok()?;
    let stopping = Arc::new(AtomicBool::new(false));
    let child = Arc::new(Mutex::new(None));
    let stop = stopping.clone();
    let handle = child.clone();
    let thread = thread::Builder::new()
        .name(format!("on-air-{}", dest.argument()))
        .spawn(move || {
            let mut backoff = Duration::from_secs(1);
            while !stop.load(Ordering::Acquire) {
                let began = Instant::now();
                let _ = pump(dest, &path, &rx, &stop, &handle);
                if stop.load(Ordering::Acquire) {
                    break;
                }
                if began.elapsed() > Duration::from_secs(60) {
                    backoff = Duration::from_secs(1);
                }
                let retry = Instant::now() + backoff;
                while Instant::now() < retry && !stop.load(Ordering::Acquire) {
                    if matches!(
                        rx.recv_timeout(Duration::from_millis(100)),
                        Err(RecvTimeoutError::Disconnected)
                    ) {
                        return;
                    }
                }
                backoff = (backoff * 2).min(Duration::from_secs(15));
            }
        })
        .ok()?;
    Some(Publisher {
        _subscription: subscription,
        stopping,
        child,
        thread: Some(thread),
    })
}

fn pump(
    dest: Destination,
    path: &Path,
    rx: &Receiver<gst::Buffer>,
    stop: &AtomicBool,
    handle: &Mutex<Option<Child>>,
) -> std::io::Result<()> {
    let mut child = Command::new(std::env::current_exe()?)
        .arg("--on-air-publisher")
        .arg(dest.argument())
        .arg(path)
        .env("GST_DEBUG", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdin = child.stdin.take().expect("piped publisher stdin");
    {
        let mut slot = handle.lock().unwrap_or_else(|e| e.into_inner());
        if stop.load(Ordering::Acquire) {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(());
        }
        *slot = Some(child);
    }
    while !stop.load(Ordering::Acquire) {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(buf) => {
                if let Ok(bytes) = buf.map_readable()
                    && stdin.write_all(bytes.as_slice()).is_err()
                {
                    break;
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                let mut slot = handle.lock().unwrap_or_else(|e| e.into_inner());
                if slot
                    .as_mut()
                    .and_then(|p| p.try_wait().ok())
                    .flatten()
                    .is_some()
                {
                    break;
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    drop(stdin);
    if let Some(mut child) = handle.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    Ok(())
}

pub(super) fn run(kind: &str, path: &Path) -> Result<(), ()> {
    gst::init().map_err(|_| ())?;
    let desc = match kind {
        "hls" => {
            "fdsrc fd=0 is-live=true ! tsdemux name=d d. ! video/x-h264 ! queue max-size-time=8000000000 max-size-buffers=0 max-size-bytes=8388608 ! h264parse config-interval=-1 ! sink.video d. ! audio/mpeg,mpegversion=4 ! queue max-size-time=8000000000 max-size-buffers=0 max-size-bytes=8388608 ! aacparse ! sink.audio hlssink2 name=sink target-duration=2 playlist-length=6 max-files=10 send-keyframe-requests=false"
        }
        "rtmp" => {
            "fdsrc fd=0 is-live=true ! tsdemux name=d d. ! video/x-h264 ! queue max-size-time=8000000000 max-size-buffers=0 max-size-bytes=8388608 ! h264parse config-interval=-1 ! video/x-h264,stream-format=avc ! mux.video d. ! audio/mpeg,mpegversion=4 ! queue max-size-time=8000000000 max-size-buffers=0 max-size-bytes=8388608 ! aacparse ! audio/mpeg,mpegversion=4,stream-format=raw ! mux.audio flvmux name=mux streamable=true ! rtmp2sink name=sink timeout=10 sync=false"
        }
        _ => return Err(()),
    };
    let pipeline = gst::parse::launch(desc)
        .map_err(|_| ())?
        .downcast::<gst::Pipeline>()
        .map_err(|_| ())?;
    let sink = pipeline.by_name("sink").ok_or(())?;
    let mut generation = String::new();
    let playlist = path.join("live.m3u8");
    if kind == "hls" {
        if !path.is_absolute() {
            return Err(());
        }
        fs::create_dir_all(path).map_err(|_| ())?;
        if !fs::symlink_metadata(path).map_err(|_| ())?.is_dir() {
            return Err(());
        }
        let mut random = [0u8; 8];
        getrandom::fill(&mut random).map_err(|_| ())?;
        generation = format!("seg-{:016x}-", u64::from_le_bytes(random));
        sink.set_property(
            "location",
            path.join(format!("{generation}%08d.ts"))
                .to_str()
                .ok_or(())?,
        );
        sink.set_property("playlist-location", playlist.to_str().ok_or(())?);
    } else {
        let mut sealed = Vec::new();
        fs::File::open(path)
            .map_err(|_| ())?
            .take(16385)
            .read_to_end(&mut sealed)
            .map_err(|_| ())?;
        if sealed.len() > 16384 {
            return Err(());
        }
        let clear =
            Zeroizing::new(station_secrets::unprotect_local_machine(&sealed).map_err(|_| ())?);
        let location = std::str::from_utf8(&clear).map_err(|_| ())?;
        station_publishing::validate_on_air_rtmp(location).map_err(|_| ())?;
        sink.set_property("location", location);
    }
    let bus = pipeline.bus().ok_or(())?;
    pipeline.set_state(gst::State::Playing).map_err(|_| ())?;
    let mut progress = Instant::now();
    let mut last = 0u64;
    let mut cleanup = Instant::now();
    let result = loop {
        if let Some(msg) = bus.timed_pop_filtered(
            gst::ClockTime::SECOND,
            &[gst::MessageType::Eos, gst::MessageType::Error],
        ) {
            break if msg.type_() == gst::MessageType::Eos {
                Ok(())
            } else {
                Err(())
            };
        }
        let observed = if kind == "hls" {
            fs::metadata(&playlist)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0)
        } else {
            sink.property::<gst::Structure>("stats")
                .get::<u64>("out-bytes-total")
                .unwrap_or(0)
        };
        if observed != last {
            progress = Instant::now();
            last = observed;
        }
        if progress.elapsed() > Duration::from_secs(30) {
            break Err(());
        }
        if kind == "hls" && cleanup.elapsed() > Duration::from_secs(30) {
            prune_old_segments(path, &generation);
            cleanup = Instant::now();
        }
    };
    let _ = pipeline.set_state(gst::State::Null);
    result
}

fn prune_old_segments(dir: &Path, generation: &str) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some((id, num)) = name
            .strip_prefix("seg-")
            .and_then(|x| x.strip_suffix(".ts"))
            .and_then(|x| x.split_once('-'))
        else {
            continue;
        };
        if id.len() != 16
            || !id.bytes().all(|b| b.is_ascii_hexdigit())
            || num.len() != 8
            || !num.bytes().all(|b| b.is_ascii_digit())
            || name.starts_with(generation)
        {
            continue;
        }
        if entry.file_type().is_ok_and(|t| t.is_file())
            && entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|a| a > Duration::from_secs(60))
        {
            let _ = fs::remove_file(entry.path());
        }
    }
}

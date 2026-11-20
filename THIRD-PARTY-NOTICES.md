# Third-party notices

Townlight Station's media engine dynamically uses GStreamer and its selected plugins. GStreamer is licensed under LGPL-2.1-or-later; source and license information are available from <https://gstreamer.freedesktop.org/>.

Townlight Station includes the unmodified `ffmpeg.exe` from BtbN's pinned `ffmpeg-n9.0.1-11-ge47273f4d9-win64-lgpl-9.0` build solely as an out-of-process caption decode-back tool. That build is LGPL, does not enable GPL-only FFmpeg components, and is installed with its `LICENSE.txt`. The exact build recipe is available from <https://github.com/BtbN/FFmpeg-Builds/tree/autobuild-2026-08-31-13-27>; corresponding FFmpeg source is available from <https://github.com/FFmpeg/FFmpeg/tree/e47273f4d9>. The distributed archive SHA-256 is `2484854ad6988d34560f4e6ea7a6ecb9dde0af7c229d2591815d056b04ec4f56`, and the installed executable SHA-256 is `63a0b3c76a245bc0d986853612d9ec43a2a2d1f1c7a3fa40ee459c248075b3a6`.

The Rust GStreamer bindings are available under MIT OR Apache-2.0 from <https://gitlab.freedesktop.org/gstreamer/gstreamer-rs>.

The RustCrypto SHA-2 implementation is available under MIT OR Apache-2.0 from <https://github.com/RustCrypto/hashes>.

The `ed25519-dalek` Rust crate is available under BSD-3-Clause from <https://github.com/dalek-cryptography/curve25519-dalek>. The `semver` Rust crate is available under MIT OR Apache-2.0 from <https://github.com/dtolnay/semver>.

The `zip` Rust crate is available under MIT from <https://github.com/zip-rs/zip2>. Its `typed-path` dependency is available under MIT OR Apache-2.0 from <https://github.com/chipsenkbeil/typed-path>.

The `windows-service` Rust crate is available under MIT OR Apache-2.0 from <https://github.com/mullvad/windows-service-rs>.

The Windows setup executable is produced with Inno Setup. Inno Setup source, license, and commercial-use terms are available from <https://jrsoftware.org/isinfo.php>.

The development installation used for machine tests may contain additional plugins that Townlight Station does not call. Product packaging will inventory and ship only required runtime components, preserve their license files, and provide corresponding-source notices where a component's license requires them.

Townlight Station's bounded future transport-capture packaging lane carries the
unmodified TSDuck Windows x64 portable runtime. TSDuck is distributed under the
BSD 2-Clause License; its `LICENSE.txt` and `OTHERS.txt` files are preserved in
the installed runtime. The pinned source is <https://github.com/tsduck/tsduck>
release `v3.44-4676`, asset
`TSDuck-Win64-3.44-4676-Portable.zip`. The source archive SHA-256 is
`b0ca0f963fcd77488b8c32d6f9d85030daa891ed753c36e57bbb458807a44eb1`.

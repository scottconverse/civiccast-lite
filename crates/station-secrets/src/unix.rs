use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use rustix::fs::{Mode, OFlags, open, openat};
use std::{
    fs::File,
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;

const FORMAT: &[u8] = b"CivicCast-key-v1\0";
const KEY_NAME: &str = "station-master.key";
const NONCE_LEN: usize = 12;

fn credential_directory() -> (PathBuf, bool) {
    if let Some(path) = std::env::var_os("CIVICCAST_CREDENTIAL_DIRECTORY") {
        return (path.into(), false);
    }
    #[cfg(target_os = "linux")]
    if let Some(path) = std::env::var_os("CREDENTIALS_DIRECTORY") {
        let path: PathBuf = path.into();
        let eligible =
            path.as_os_str() == std::ffi::OsStr::new("/run/credentials/civiccast-lab.service");
        return (path, eligible);
    }
    #[cfg(target_os = "macos")]
    return (
        "/Library/Application Support/CivicCast Lab/credentials".into(),
        false,
    );
    #[cfg(not(target_os = "macos"))]
    return ("/etc/civiccast-lab/credentials".into(), false);
}

fn load_key(directory: &Path, allow_acl_exception: bool) -> Result<LessSafeKey, String> {
    if !directory.is_absolute() {
        return Err("station credential directory must be absolute".into());
    }
    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let dir = File::from(
        open(directory, flags | OFlags::DIRECTORY, Mode::empty())
            .map_err(|e| format!("open station credential directory: {e}"))?,
    );
    let trusted =
        |m: &std::fs::Metadata| m.uid() == 0 || m.uid() == rustix::process::geteuid().as_raw();
    let dir_meta = dir.metadata().map_err(|e| e.to_string())?;
    if !trusted(&dir_meta) || dir_meta.mode() & 0o022 != 0 {
        return Err("unsafe station credential directory permissions".into());
    }
    let mut file = File::from(
        openat(&dir, KEY_NAME, flags | OFlags::NONBLOCK, Mode::empty())
            .map_err(|e| format!("open station credential key: {e}"))?,
    );
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() != 32 || !trusted(&meta) || meta.nlink() != 1 {
        return Err("invalid station credential key type, length, owner or permissions".into());
    }
    #[cfg(target_os = "linux")]
    let acl_exception = allow_acl_exception && meta.mode() & 0o7777 == 0o440;
    #[cfg(not(target_os = "linux"))]
    let acl_exception = {
        let _ = allow_acl_exception;
        false
    };
    #[cfg(target_os = "linux")]
    if acl_exception {
        check_acl_exception(&dir, &file, &dir_meta, &meta)?;
    }
    if !acl_exception && meta.mode() & 0o077 != 0 {
        return Err("invalid station credential key type, length, owner or permissions".into());
    }
    let mut bytes = Zeroizing::new([0u8; 32]);
    file.read_exact(bytes.as_mut())
        .map_err(|e| format!("read station credential key: {e}"))?;
    let key = UnboundKey::new(&AES_256_GCM, bytes.as_ref())
        .map_err(|_| "invalid station credential key")?;
    Ok(LessSafeKey::new(key))
}

#[cfg(target_os = "linux")]
fn check_acl_exception(
    dir: &File,
    file: &File,
    dir_meta: &std::fs::Metadata,
    key_meta: &std::fs::Metadata,
) -> Result<(), String> {
    if dir_meta.uid() != 0 || dir_meta.gid() != 0 || dir_meta.mode() & 0o7777 != 0o550 {
        return Err(
            "unsafe station credential directory ACL exception ownership or permissions".into(),
        );
    }
    if key_meta.uid() != 0 || key_meta.gid() != 0 || key_meta.mode() & 0o7777 != 0o440 {
        return Err("unsafe station credential key ACL exception ownership or permissions".into());
    }
    if !rustix::fs::fstatvfs(dir)
        .map_err(|e| format!("fstatvfs station credential directory: {e}"))?
        .f_flag
        .contains(rustix::fs::StatVfsMountFlags::RDONLY)
    {
        return Err("station credential directory is not read-only".into());
    }
    if !rustix::fs::fstatvfs(file)
        .map_err(|e| format!("fstatvfs station credential key: {e}"))?
        .f_flag
        .contains(rustix::fs::StatVfsMountFlags::RDONLY)
    {
        return Err("station credential key is not read-only".into());
    }
    let euid = rustix::process::geteuid().as_raw();
    let expected_dir = canonical_acl_bytes(5, euid);
    let mut buffer = [0u8; 44];
    let len = rustix::fs::fgetxattr(dir, "system.posix_acl_access", &mut buffer)
        .map_err(|e| format!("read station credential directory ACL: {e}"))?;
    if len != expected_dir.len() || buffer[..len] != expected_dir[..] {
        return Err("invalid station credential directory ACL".into());
    }
    let expected_key = canonical_acl_bytes(4, euid);
    let mut buffer = [0u8; 44];
    let len = rustix::fs::fgetxattr(file, "system.posix_acl_access", &mut buffer)
        .map_err(|e| format!("read station credential key ACL: {e}"))?;
    if len != expected_key.len() || buffer[..len] != expected_key[..] {
        return Err("invalid station credential key ACL".into());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn canonical_acl_bytes(perms: u16, euid: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(44);
    bytes.extend_from_slice(&2u32.to_le_bytes());
    for (tag, perm, id) in [
        (1u16, perms, u32::MAX),
        (2u16, perms, euid),
        (4u16, 0u16, u32::MAX),
        (16u16, perms, u32::MAX),
        (32u16, 0u16, u32::MAX),
    ] {
        bytes.extend_from_slice(&tag.to_le_bytes());
        bytes.extend_from_slice(&perm.to_le_bytes());
        bytes.extend_from_slice(&id.to_le_bytes());
    }
    bytes
}

fn seal(key: &LessSafeKey, plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).map_err(|_| "station credential randomness unavailable")?;
    let mut envelope = Vec::from(FORMAT);
    envelope.extend_from_slice(&nonce);
    let start = envelope.len();
    envelope.extend_from_slice(plaintext);
    let tag = key
        .seal_in_place_separate_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(FORMAT),
            &mut envelope[start..],
        )
        .map_err(|_| "could not protect station credential")?;
    envelope.extend_from_slice(tag.as_ref());
    Ok(envelope)
}

fn unseal(key: &LessSafeKey, protected: &[u8]) -> Result<Vec<u8>, String> {
    let body = protected
        .strip_prefix(FORMAT)
        .ok_or("unsupported station credential format")?;
    let (nonce, encrypted) = body
        .split_at_checked(NONCE_LEN)
        .ok_or("incomplete station credential")?;
    let mut buffer = Zeroizing::new(encrypted.to_vec());
    let plaintext = key
        .open_in_place(
            Nonce::try_assume_unique_for_key(nonce).map_err(|_| "invalid nonce")?,
            Aad::from(FORMAT),
            &mut buffer,
        )
        .map_err(|_| "station credential authentication failed")?;
    Ok(plaintext.to_vec())
}

pub fn protect_local_machine(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let (directory, allow_acl_exception) = credential_directory();
    seal(&load_key(&directory, allow_acl_exception)?, plaintext)
}

pub fn unprotect_local_machine(protected: &[u8]) -> Result<Vec<u8>, String> {
    let (directory, allow_acl_exception) = credential_directory();
    unseal(&load_key(&directory, allow_acl_exception)?, protected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::{PermissionsExt, symlink},
    };

    #[test]
    fn credential_reopens_and_rejects_tampering_and_key_substitution() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(KEY_NAME);
        fs::write(&path, [1u8; 32]).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let key = load_key(dir.path(), false).unwrap();
        let mut sealed = seal(&key, b"fixture").unwrap();
        assert_ne!(sealed, seal(&key, b"fixture").unwrap());
        assert_eq!(
            unseal(&load_key(dir.path(), false).unwrap(), &sealed).unwrap(),
            b"fixture"
        );
        fs::write(&path, [2u8; 32]).unwrap();
        assert!(unseal(&load_key(dir.path(), false).unwrap(), &sealed).is_err());
        *sealed.last_mut().unwrap() ^= 1;
        assert!(unseal(&key, &sealed).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(load_key(dir.path(), false).is_err());
        fs::rename(&path, dir.path().join("other")).unwrap();
        symlink("other", &path).unwrap();
        assert!(load_key(dir.path(), false).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn strict_0440_is_rejected_and_acl_bytes_are_exact() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(KEY_NAME);
        fs::write(&path, [1u8; 32]).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o440)).unwrap();
        assert!(load_key(dir.path(), false).is_err());

        let euid = rustix::process::geteuid().as_raw();
        let expected = canonical_acl_bytes(4, euid);
        assert_eq!(expected.len(), 44);
        assert_eq!(&expected[..4], &2u32.to_le_bytes());
        let mut wrong = expected.clone();
        wrong[10] ^= 1;
        assert_ne!(wrong, expected);
        assert_ne!(canonical_acl_bytes(5, euid), expected);
    }
}

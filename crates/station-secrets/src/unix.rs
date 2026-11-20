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

fn credential_directory() -> PathBuf {
    if let Some(path) = std::env::var_os("CIVICCAST_CREDENTIAL_DIRECTORY") {
        return path.into();
    }
    #[cfg(target_os = "linux")]
    if let Some(path) = std::env::var_os("CREDENTIALS_DIRECTORY") {
        return path.into();
    }
    #[cfg(target_os = "macos")]
    return "/Library/Application Support/CivicCast Lab/credentials".into();
    #[cfg(not(target_os = "macos"))]
    return "/etc/civiccast-lab/credentials".into();
}

fn load_key(directory: &Path) -> Result<LessSafeKey, String> {
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
    let meta = dir.metadata().map_err(|e| e.to_string())?;
    if !trusted(&meta) || meta.mode() & 0o022 != 0 {
        return Err("unsafe station credential directory permissions".into());
    }
    let mut file = File::from(
        openat(&dir, KEY_NAME, flags | OFlags::NONBLOCK, Mode::empty())
            .map_err(|e| format!("open station credential key: {e}"))?,
    );
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file()
        || meta.len() != 32
        || !trusted(&meta)
        || meta.mode() & 0o077 != 0
        || meta.nlink() != 1
    {
        return Err("invalid station credential key type, length, owner or permissions".into());
    }
    let mut bytes = Zeroizing::new([0u8; 32]);
    file.read_exact(bytes.as_mut())
        .map_err(|e| format!("read station credential key: {e}"))?;
    let key = UnboundKey::new(&AES_256_GCM, bytes.as_ref())
        .map_err(|_| "invalid station credential key")?;
    Ok(LessSafeKey::new(key))
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
    seal(&load_key(&credential_directory())?, plaintext)
}

pub fn unprotect_local_machine(protected: &[u8]) -> Result<Vec<u8>, String> {
    unseal(&load_key(&credential_directory())?, protected)
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
        let key = load_key(dir.path()).unwrap();
        let mut sealed = seal(&key, b"fixture").unwrap();
        assert_ne!(sealed, seal(&key, b"fixture").unwrap());
        assert_eq!(
            unseal(&load_key(dir.path()).unwrap(), &sealed).unwrap(),
            b"fixture"
        );
        fs::write(&path, [2u8; 32]).unwrap();
        assert!(unseal(&load_key(dir.path()).unwrap(), &sealed).is_err());
        *sealed.last_mut().unwrap() ^= 1;
        assert!(unseal(&key, &sealed).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(load_key(dir.path()).is_err());
        fs::rename(&path, dir.path().join("other")).unwrap();
        symlink("other", &path).unwrap();
        assert!(load_key(dir.path()).is_err());
    }
}

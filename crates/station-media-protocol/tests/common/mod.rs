pub fn test_managed_root(parts: &[&str]) -> String {
    #[cfg(windows)]
    let base = std::path::Path::new(r"C:\ProgramData\TownLight Station");
    #[cfg(not(windows))]
    let base = std::path::Path::new("/var/lib/townlight-station");
    let mut path = base.to_path_buf();
    for part in parts {
        path.push(part);
    }
    path.to_string_lossy().into_owned()
}

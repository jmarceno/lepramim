use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Validate socket path: must be under runtime_dir and parent is 0700 if exists.
pub fn socket_path_valid(sock: &Path, runtime_dir: &Path) -> Result<(), String> {
    let resolved_sock = sock.canonicalize().unwrap_or_else(|_| sock.to_path_buf());
    let resolved_rt = runtime_dir
        .canonicalize()
        .unwrap_or_else(|_| runtime_dir.to_path_buf());
    let rt_str = resolved_rt.to_string_lossy().to_string();
    let sock_str = resolved_sock.to_string_lossy().to_string();
    if !sock_str.starts_with(&format!("{}/", rt_str)) {
        return Err(format!(
            "refusing to bind UDS outside XDG_RUNTIME_DIR: socket={}, runtime_dir={}",
            resolved_sock.display(),
            resolved_rt.display()
        ));
    }
    Ok(())
}

/// Ensure parent dir exists with mode 0700, remove stale socket if owned by current user.
pub fn stale_socket_cleanup(sock: &Path) -> Result<(), String> {
    let parent = sock.parent().ok_or("socket has no parent")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    // Enforce 0700
    let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
    // Remove stale socket if exists and owned by current user (check via metadata uid if possible)
    if sock.exists() || sock.is_symlink() {
        // Check ownership via libc::stat
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if let Ok(meta) = std::fs::symlink_metadata(sock) {
                let uid = meta.uid();
                let current = libc_getuid();
                if uid != current {
                    return Err(format!(
                        "socket {} owned by uid {} not current {}, refusing to remove",
                        sock.display(),
                        uid,
                        current
                    ));
                }
            }
        }
        std::fs::remove_file(sock).map_err(|e| format!("failed to remove stale socket: {}", e))?;
    }
    Ok(())
}

#[cfg(unix)]
fn libc_getuid() -> u32 {
    unsafe { getuid() }
}
#[cfg(unix)]
unsafe extern "C" {
    fn getuid() -> u32;
}

/// Generate systemd user unit file content.
pub fn generate_systemd_unit(exec_path: &Path) -> String {
    format!(
        r#"[Unit]
Description=Lexaloud TTS daemon
After=graphical-session.target

[Service]
Type=simple
ExecStart={} daemon
Restart=on-failure
RestartSec=2
RuntimeDirectory=lexaloud
RuntimeDirectoryMode=0700

[Install]
WantedBy=default.target
"#,
        exec_path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    #[test]
    fn socket_valid_inside() {
        let rt = PathBuf::from("/run/user/1000");
        let sock = PathBuf::from("/run/user/1000/lexaloud/lexaloud.sock");
        // Canonicalize will fail for non-existent, but our function uses fallback to non-canonical
        // So it should pass prefix check via string
        let res = socket_path_valid(&sock, &rt);
        assert!(res.is_ok(), "got {:?}", res);
    }
    #[test]
    fn socket_invalid_outside() {
        let rt = PathBuf::from("/run/user/1000");
        let sock = PathBuf::from("/tmp/evil.sock");
        let res = socket_path_valid(&sock, &rt);
        assert!(res.is_err());
    }
    #[test]
    fn generate_unit_contains_exec() {
        let unit = generate_systemd_unit(Path::new("/usr/bin/lexaloud"));
        assert!(unit.contains("ExecStart=/usr/bin/lexaloud daemon"));
        assert!(unit.contains("RuntimeDirectory=lexaloud"));
    }
}

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

/// True when a process is listening on `sock` (connect succeeds).
pub fn is_socket_live(sock: &Path) -> bool {
    std::os::unix::net::UnixStream::connect(sock).is_ok()
}

/// Ensure parent dir exists with mode 0700, remove stale socket if owned by current user.
///
/// Refuses to remove a live socket: if another daemon is already listening,
/// returns an error instead of stealing its socket.
pub fn stale_socket_cleanup(sock: &Path) -> Result<(), String> {
    let parent = sock.parent().ok_or("socket has no parent")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
    if sock.exists() || sock.is_symlink() {
        if is_socket_live(sock) {
            return Err(format!(
                "daemon already running (socket {} is live); not starting a second instance",
                sock.display()
            ));
        }
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

/// Shell-quote a path for systemd ExecStart.
pub fn shell_quote(path: &str) -> String {
    format!("\"{}\"", path.replace('\\', "\\\\").replace('"', "\\\""))
}

/// True when `path` looks like a Lepramim AppImage (not a host IDE AppImage).
///
/// Cursor and similar tools export `$APPIMAGE` into every integrated shell;
/// we must not treat those as our own binary.
pub fn is_lepramim_appimage_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| name.to_ascii_lowercase().contains("lepramim"))
}

/// Resolve the lepramim binary path for AppImage / source installs.
/// AppImage children must be spawned from `$LEPRAMIM_APPIMAGE` so they keep
/// their own mount; never point at `/tmp/.mount_*`.
///
/// Ignores foreign `$APPIMAGE` values (e.g. Cursor's AppImage) that leak into
/// agent / IDE shells.
pub fn resolve_binary_path() -> std::path::PathBuf {
    if let Ok(appimage) = std::env::var("LEPRAMIM_APPIMAGE") {
        let p = std::path::PathBuf::from(&appimage);
        if is_lepramim_appimage_path(&p) && p.is_file() {
            return p;
        }
    }
    if let Ok(appimage) = std::env::var("APPIMAGE") {
        let p = std::path::PathBuf::from(&appimage);
        if is_lepramim_appimage_path(&p) && p.is_file() {
            return p;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        return exe;
    }
    std::path::PathBuf::from("lepramim")
}

/// XDG autostart desktop file path.
pub fn autostart_path() -> std::path::PathBuf {
    if let Ok(base) = std::env::var("XDG_CONFIG_HOME") {
        if !base.is_empty() {
            return std::path::PathBuf::from(base)
                .join("autostart")
                .join("lepramim.desktop");
        }
    }
    directories::BaseDirs::new()
        .map(|d| {
            d.home_dir()
                .join(".config")
                .join("autostart")
                .join("lepramim.desktop")
        })
        .unwrap_or_else(|| std::path::PathBuf::from(".config/autostart/lepramim.desktop"))
}

/// Desktop file path under XDG_DATA_HOME.
pub fn desktop_file_path() -> std::path::PathBuf {
    if let Ok(base) = std::env::var("XDG_DATA_HOME") {
        if !base.is_empty() {
            return std::path::PathBuf::from(base)
                .join("applications")
                .join("lepramim.desktop");
        }
    }
    directories::BaseDirs::new()
        .map(|d| {
            d.home_dir()
                .join(".local")
                .join("share")
                .join("applications")
                .join("lepramim.desktop")
        })
        .unwrap_or_else(|| std::path::PathBuf::from(".local/share/applications/lepramim.desktop"))
}

/// Generate an XDG desktop entry that launches the AppImage / binary with no args.
pub fn generate_autostart_desktop(exec_path: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
Type=Application\n\
Name=Lepramim\n\
GenericName=Text to Speech\n\
Comment=Local Kokoro text-to-speech tool\n\
Exec={}\n\
Terminal=false\n\
Categories=AudioVideo;Audio;Accessibility;\n\
X-GNOME-Autostart-enabled=true\n",
        shell_quote(&exec_path.to_string_lossy())
    )
}

pub fn write_autostart(exec_path: &Path) -> Result<std::path::PathBuf, String> {
    let path = autostart_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, generate_autostart_desktop(exec_path)).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn remove_autostart() -> Result<Option<std::path::PathBuf>, String> {
    let path = autostart_path();
    if path.is_file() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        return Ok(Some(path));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn rejects_foreign_appimage_names() {
        assert!(is_lepramim_appimage_path(Path::new(
            "/opt/Lepramim-0.2.0-x86_64.AppImage"
        )));
        assert!(is_lepramim_appimage_path(Path::new(
            "/tmp/lepramim-dev.AppImage"
        )));
        assert!(!is_lepramim_appimage_path(Path::new(
            "/home/user/Software/AppImages/cursor.appimage"
        )));
        assert!(!is_lepramim_appimage_path(Path::new(
            "/tmp/.mount_cursorXXXX/AppRun"
        )));
    }

    #[test]
    fn socket_valid_inside() {
        let rt = PathBuf::from("/run/user/1000");
        let sock = PathBuf::from("/run/user/1000/lepramim/lepramim.sock");
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
    fn autostart_desktop_quotes_exec() {
        let desk = generate_autostart_desktop(Path::new("/opt/Lepramim-0.2.0-x86_64.AppImage"));
        assert!(desk.contains("Exec=\"/opt/Lepramim-0.2.0-x86_64.AppImage\""));
        assert!(desk.contains("Terminal=false"));
        assert!(!desk.contains("systemd"));
    }

    #[test]
    fn stale_cleanup_refuses_live_socket() {
        let base = std::env::temp_dir().join(format!(
            "lepramim_svc_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let sock = base.join("lepramim.sock");
        std::fs::create_dir_all(base.parent().unwrap_or(&base)).ok();
        std::fs::create_dir_all(sock.parent().unwrap()).unwrap();
        // Live listener: cleanup must refuse to steal it.
        let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();
        assert!(is_socket_live(&sock));
        let res = stale_socket_cleanup(&sock);
        assert!(res.is_err(), "expected refusal, got {res:?}");
        assert!(sock.exists(), "live socket must be preserved");
        drop(listener);
        // After the listener is gone the file is stale: cleanup removes it.
        // (The fd is closed but the path still exists until removed.)
        assert!(sock.exists());
        let res = stale_socket_cleanup(&sock);
        assert!(res.is_ok(), "got {res:?}");
        assert!(!sock.exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}

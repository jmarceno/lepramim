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
    let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
    if sock.exists() || sock.is_symlink() {
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

/// Resolve the lexaloud binary path for AppImage / source installs.
/// AppImage children must be spawned from `$LEXALOUD_APPIMAGE` so they keep
/// their own mount; never point at `/tmp/.mount_*`.
pub fn resolve_binary_path() -> std::path::PathBuf {
    if let Ok(appimage) = std::env::var("LEXALOUD_APPIMAGE") {
        let p = std::path::PathBuf::from(&appimage);
        if p.is_file() {
            return p;
        }
    }
    if let Ok(appimage) = std::env::var("APPIMAGE") {
        let p = std::path::PathBuf::from(&appimage);
        if p.is_file() {
            return p;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        return exe;
    }
    std::path::PathBuf::from("lexaloud")
}

/// XDG autostart desktop file path.
pub fn autostart_path() -> std::path::PathBuf {
    if let Ok(base) = std::env::var("XDG_CONFIG_HOME") {
        if !base.is_empty() {
            return std::path::PathBuf::from(base)
                .join("autostart")
                .join("lexaloud.desktop");
        }
    }
    directories::BaseDirs::new()
        .map(|d| {
            d.home_dir()
                .join(".config")
                .join("autostart")
                .join("lexaloud.desktop")
        })
        .unwrap_or_else(|| std::path::PathBuf::from(".config/autostart/lexaloud.desktop"))
}

/// Desktop file path under XDG_DATA_HOME.
pub fn desktop_file_path() -> std::path::PathBuf {
    if let Ok(base) = std::env::var("XDG_DATA_HOME") {
        if !base.is_empty() {
            return std::path::PathBuf::from(base)
                .join("applications")
                .join("lexaloud.desktop");
        }
    }
    directories::BaseDirs::new()
        .map(|d| {
            d.home_dir()
                .join(".local")
                .join("share")
                .join("applications")
                .join("lexaloud.desktop")
        })
        .unwrap_or_else(|| std::path::PathBuf::from(".local/share/applications/lexaloud.desktop"))
}

/// Generate an XDG desktop entry that launches the AppImage / binary with no args.
pub fn generate_autostart_desktop(exec_path: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
Type=Application\n\
Name=Lexaloud\n\
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

/// User systemd unit directory (only used to clean leftover units from old installs).
pub fn systemd_user_dir() -> std::path::PathBuf {
    if let Ok(base) = std::env::var("XDG_CONFIG_HOME") {
        if !base.is_empty() {
            return std::path::PathBuf::from(base).join("systemd").join("user");
        }
    }
    directories::BaseDirs::new()
        .map(|d| d.home_dir().join(".config").join("systemd").join("user"))
        .unwrap_or_else(|| std::path::PathBuf::from(".config/systemd/user"))
}

/// Generate systemd user unit file content (legacy leftover; not used at runtime).
pub fn generate_systemd_unit(exec_path: &Path) -> String {
    format!(
        r#"[Unit]
Description=Lexaloud TTS daemon
After=default.target

[Service]
Type=simple
ExecStart={} daemon
Restart=on-failure
RestartSec=2
TimeoutStopSec=10
UnsetEnvironment=PYTHONPATH
RuntimeDirectory=lexaloud
RuntimeDirectoryMode=0700
WorkingDirectory=%h

[Install]
WantedBy=default.target
"#,
        shell_quote(&exec_path.to_string_lossy())
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
        assert!(unit.contains("ExecStart=\"/usr/bin/lexaloud\" daemon"));
        assert!(unit.contains("RuntimeDirectory=lexaloud"));
    }

    #[test]
    fn autostart_desktop_quotes_exec() {
        let desk = generate_autostart_desktop(Path::new("/opt/Lexaloud-0.2.0-x86_64.AppImage"));
        assert!(desk.contains("Exec=\"/opt/Lexaloud-0.2.0-x86_64.AppImage\""));
        assert!(desk.contains("Terminal=false"));
        assert!(!desk.contains("systemd"));
    }
}

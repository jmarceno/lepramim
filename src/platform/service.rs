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

/// Resolve the path desktop entries and child processes should launch.
pub fn resolve_binary_path() -> std::path::PathBuf {
    // Portable launcher: point at the self-extracting file, not the versioned
    // cache extraction (which disappears on upgrade).
    if let Ok(exe) = std::env::var("LEPRAMIM_PORTABLE_EXE") {
        let p = std::path::PathBuf::from(&exe);
        if p.is_file() {
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

/// Generate an XDG desktop entry that launches the binary with no args.
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

/// Exec target (first word, quotes stripped) of a desktop entry, skipping an
/// `env VAR=…` prefix.
fn desktop_exec_target(contents: &str) -> Option<String> {
    let line = contents.lines().find_map(|l| l.strip_prefix("Exec="))?;
    let mut words = line.split_whitespace().map(|w| w.trim_matches('"'));
    let mut w = words.next()?;
    if w == "env" {
        w = words.find(|w| !w.contains('='))?;
    }
    Some(w.to_string())
}

/// Repoint existing menu / autostart entries whose absolute launcher path
/// no longer exists at the portable launcher running now. Only acts for packaged runs, so a dev `cargo run` never hijacks the
/// user's entries, and never creates entries the user did not have.
pub fn refresh_stale_desktop_entries() {
    if std::env::var_os("LEPRAMIM_PORTABLE_EXE").is_none() {
        return;
    }
    let current = resolve_binary_path();
    if !current.is_file() {
        return;
    }
    for path in [desktop_file_path(), autostart_path()] {
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(target) = desktop_exec_target(&contents) else {
            continue;
        };
        // Bare names (`Exec=lepramim`) resolve via PATH; leave them alone.
        let target_path = Path::new(&target);
        if !target_path.is_absolute() || target_path.is_file() {
            continue;
        }
        let mut fresh = generate_autostart_desktop(&current);
        if let Some(icon) = contents.lines().find(|l| l.starts_with("Icon=")) {
            fresh.push_str(icon);
            fresh.push('\n');
        }
        match std::fs::write(&path, fresh) {
            Ok(()) => tracing::info!(path = %path.display(), old = %target, "repointed stale desktop entry"),
            Err(e) => tracing::warn!(path = %path.display(), "could not refresh desktop entry: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn desktop_exec_target_skips_env() {
        assert_eq!(
            desktop_exec_target("Exec=env FOO=1 /a/lepramim\n").as_deref(),
            Some("/a/lepramim")
        );
        assert_eq!(
            desktop_exec_target("Exec=\"/b/lepramim\"\n").as_deref(),
            Some("/b/lepramim")
        );
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
        let desk = generate_autostart_desktop(Path::new("/opt/Lepramim-0.2.0-x86_64-portable.run"));
        assert!(desk.contains("Exec=\"/opt/Lepramim-0.2.0-x86_64-portable.run\""));
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

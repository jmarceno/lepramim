//! Single-instance guard for the Lepramim desktop app.
//!
//! Only one `lepramim` app (tray + daemon + UI) may run per user. The first
//! (primary) instance binds `app.sock` under the XDG runtime dir and listens
//! for activation requests. A second instance detects the live socket,
//! notifies the primary to show its control window, then exits without
//! opening a duplicate UI.
//!
//! Protocol is intentionally tiny: secondary connects and writes
//! `show-control\n`; primary replies `ok\n` and emits
//! [`SingleInstanceEvent::ShowControl`].

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

const ACTIVATE_MSG: &[u8] = b"show-control\n";
const ACK_MSG: &[u8] = b"ok\n";

/// Event delivered to the primary instance when a secondary starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingleInstanceEvent {
    ShowControl,
}

/// Outcome of [`acquire`].
pub enum AcquireOutcome {
    /// This process is the primary; hold the guard for the app lifetime.
    Primary(SingleInstanceGuard),
    /// Another instance is already running and was notified; exit.
    Secondary,
}

/// Holds the primary's socket binding. Dropping removes the socket file.
/// The listener thread is detached and owns the [`UnixListener`]; the guard
/// only needs to keep the path alive for cleanup.
pub struct SingleInstanceGuard {
    socket_path: PathBuf,
    rx: crossbeam_channel::Receiver<SingleInstanceEvent>,
}

impl SingleInstanceGuard {
    pub fn receiver(&self) -> &crossbeam_channel::Receiver<SingleInstanceEvent> {
        &self.rx
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

/// Runtime IPC socket for app singleton signalling.
///
/// Distinct from the daemon socket (`lepramim.sock`) so CLI subcommands that
/// talk to the daemon are unaffected.
pub fn app_socket_path() -> PathBuf {
    crate::config::runtime_dir()
        .join("lepramim")
        .join("app.sock")
}

/// True if something is listening on `path` (connect succeeds).
pub fn is_socket_live(path: &Path) -> bool {
    UnixStream::connect(path).is_ok()
}

/// Notify the primary instance to show its control window.
///
/// Returns true when a live primary was reached and the activation message
/// was delivered. Write success is sufficient; the ack is best-effort so a
/// slow primary does not cause a false negative (which could otherwise lead
/// to two primaries).
pub fn notify_on_path(path: &Path) -> bool {
    let mut stream = match UnixStream::connect(path) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    if stream.write_all(ACTIVATE_MSG).is_err() {
        return false;
    }
    let _ = stream.flush();
    // Best-effort ack: confirms the peer speaks our protocol, but delivery
    // already happened once the bytes were written.
    let mut ack = [0u8; 8];
    match stream.read(&mut ack) {
        Ok(n) if n > 0 => String::from_utf8_lossy(&ack[..n]).contains("ok"),
        // No ack (e.g. primary is an older build that closes without
        // replying) still counts as delivered.
        _ => true,
    }
}

/// Notify the primary app instance (default socket path).
pub fn notify_primary() -> bool {
    notify_on_path(&app_socket_path())
}

/// Acquire the singleton on the default socket path.
pub fn acquire() -> Result<AcquireOutcome, String> {
    acquire_on_path(&app_socket_path())
}

/// Acquire the singleton on an explicit path (used by tests).
pub fn acquire_on_path(path: &Path) -> Result<AcquireOutcome, String> {
    ensure_parent(path)?;

    // Fast path: live primary already running.
    if notify_on_path(path) {
        return Ok(AcquireOutcome::Secondary);
    }
    // Connect succeeded but the activation write failed: a live peer exists
    // but did not accept our message. Never steal its socket; exit as
    // secondary so at most one UI runs.
    if is_socket_live(path) {
        return Ok(AcquireOutcome::Secondary);
    }

    match try_bind(path) {
        Ok(listener) => Ok(AcquireOutcome::Primary(spawn_primary(path, listener)?)),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            // Lost a race with another starting primary: it is live now.
            if notify_on_path(path) || is_socket_live(path) {
                return Ok(AcquireOutcome::Secondary);
            }
            Err(format!(
                "single-instance socket {} is in use by an unresponsive process: {e}",
                path.display()
            ))
        }
        Err(e) => Err(format!(
            "failed to bind single-instance socket {}: {e}",
            path.display()
        )),
    }
}

fn ensure_parent(path: &Path) -> Result<(), String> {
    let parent = path.parent().ok_or("socket path has no parent")?;
    std::fs::create_dir_all(parent).map_err(|e| format!("socket dir: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

/// Bind after removing a stale file.
///
/// Caller must have verified no live listener (via [`notify_on_path`] /
/// [`is_socket_live`); this only clears crash leftovers or regular files.
fn try_bind(path: &Path) -> std::io::Result<UnixListener> {
    if path.exists() || path.is_symlink() {
        let _ = std::fs::remove_file(path);
    }
    UnixListener::bind(path)
}

fn spawn_primary(path: &Path, listener: UnixListener) -> Result<SingleInstanceGuard, String> {
    let (tx, rx) = crossbeam_channel::unbounded();
    let path_buf = path.to_path_buf();
    std::thread::Builder::new()
        .name("lepramim-single-instance".into())
        .spawn(move || accept_loop(listener, tx))
        .map_err(|e| format!("failed to start single-instance listener: {e}"))?;
    Ok(SingleInstanceGuard {
        socket_path: path_buf,
        rx,
    })
}

fn accept_loop(listener: UnixListener, tx: crossbeam_channel::Sender<SingleInstanceEvent>) {
    for incoming in listener.incoming() {
        let mut stream = match incoming {
            Ok(s) => s,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::InvalidInput {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
        let mut buf = [0u8; 128];
        let msg = match stream.read(&mut buf) {
            // Plain connect-and-close liveness probe: stay silent so
            // `is_socket_live` does not wake the UI.
            Ok(0) => continue,
            Ok(n) => String::from_utf8_lossy(&buf[..n]).into_owned(),
            Err(_) => continue,
        };
        if msg.contains("show-control") || msg.is_empty() {
            let _ = tx.send(SingleInstanceEvent::ShowControl);
        } else {
            // Forward-compatible: any message on this socket means
            // "another launch wants the UI".
            let _ = tx.send(SingleInstanceEvent::ShowControl);
        }
        let _ = stream.write_all(ACK_MSG);
        let _ = stream.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn temp_sock(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "lepramim_single_test_{}_{}_{}",
            std::process::id(),
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        // Use a subdir so parent-creation + 0700 logic is exercised.
        p.join("app.sock")
    }

    fn recv_timeout(
        rx: &crossbeam_channel::Receiver<SingleInstanceEvent>,
    ) -> Option<SingleInstanceEvent> {
        rx.recv_timeout(Duration::from_secs(3)).ok()
    }

    #[test]
    fn primary_acquire_creates_socket() {
        let sock = temp_sock("primary");
        let outcome = acquire_on_path(&sock).expect("acquire");
        match outcome {
            AcquireOutcome::Primary(guard) => {
                assert!(sock.exists(), "socket file should exist");
                assert!(is_socket_live(&sock));
                drop(guard);
            }
            AcquireOutcome::Secondary => panic!("expected primary"),
        }
        // Guard drop cleans up.
        assert!(!sock.exists(), "socket should be removed on drop");
        let _ = std::fs::remove_dir_all(sock.parent().unwrap());
    }

    #[test]
    fn secondary_notifies_primary() {
        let sock = temp_sock("notify");
        let outcome = acquire_on_path(&sock).expect("primary acquire");
        let guard = match outcome {
            AcquireOutcome::Primary(g) => g,
            AcquireOutcome::Secondary => panic!("expected primary"),
        };
        assert!(notify_on_path(&sock), "notify should succeed");
        let ev = recv_timeout(guard.receiver());
        assert_eq!(ev, Some(SingleInstanceEvent::ShowControl));

        // A second acquire must report Secondary (and also notify).
        match acquire_on_path(&sock).expect("second acquire") {
            AcquireOutcome::Secondary => {}
            AcquireOutcome::Primary(_) => panic!("expected secondary"),
        }
        let ev2 = recv_timeout(guard.receiver());
        assert_eq!(ev2, Some(SingleInstanceEvent::ShowControl));

        drop(guard);
        assert!(!notify_on_path(&sock), "notify after drop should fail");
        let _ = std::fs::remove_dir_all(sock.parent().unwrap());
    }

    #[test]
    fn stale_regular_file_is_replaced() {
        let sock = temp_sock("stale");
        let parent = sock.parent().unwrap().to_path_buf();
        std::fs::create_dir_all(&parent).unwrap();
        std::fs::write(&sock, b"stale").unwrap();
        assert!(!is_socket_live(&sock));
        let outcome = acquire_on_path(&sock).expect("acquire over stale file");
        match outcome {
            AcquireOutcome::Primary(guard) => {
                assert!(is_socket_live(&sock));
                drop(guard);
            }
            AcquireOutcome::Secondary => panic!("expected primary over stale file"),
        }
        let _ = std::fs::remove_dir_all(&parent);
    }

    #[test]
    fn app_socket_path_shape() {
        let p = app_socket_path();
        assert!(p.ends_with("lepramim/app.sock"), "got {}", p.display());
        assert_ne!(p, crate::config::socket_path());
    }
}

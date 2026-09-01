"""Lexaloud desktop application mode.

`lexaloud` (or `lexaloud app`) is the "normal GUI app" entry point: it
shows the tray icon, runs the daemon, and performs first-run onboarding
without ever asking the user to open a terminal:

1. If the Kokoro model is not cached, download it automatically with a
   progress dialog (one-time, ~350 MB).
2. Create default global shortcuts when the desktop allows it
   (GNOME gsettings / XFCE xfconf / native KDE desktop actions).
3. Offer to start Lexaloud automatically at login (XDG autostart).
4. Start the daemon as a child process and hand control to the tray.

No systemd unit is required: the daemon lives exactly as long as the
app does, and "Start with desktop" replaces the service for autostart.
The systemd integration stays available as an opt-in CLI path
(`lexaloud setup`); if a unit is installed and running, the app adopts
the running daemon instead of starting a second one.
"""

from __future__ import annotations

import logging
import os
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path

from PySide6 import QtCore, QtWidgets

from .config import runtime_dir, socket_path

log = logging.getLogger(__name__)

DEFAULT_KEYBINDINGS: dict[str, str] = {
    "lexaloud": "<Super>r",
    "lexaloud-toggle": "<Super>p",
}

AUTOSTART_SECTION = "[Desktop Entry]"


# ---------- paths / pure helpers (unit-tested without Qt) -----------------


def binary_path() -> Path:
    """Absolute path that relaunches this application.

    Only trust ``APPIMAGE`` inside a frozen build: PyInstaller sets
    ``sys.frozen`` there, while processes that merely inherit the
    environment from an AppImage-launched parent (file managers, IDEs,
    this function being called from another app) also see the parent's
    ``APPIMAGE`` and must not return it.
    """
    if getattr(sys, "frozen", False):
        appimage = os.environ.get("APPIMAGE")
        if appimage:
            return Path(appimage).resolve()
        return Path(sys.executable).resolve()
    which = None
    try:
        from shutil import which as _which

        which = _which("lexaloud")
    except Exception:  # noqa: BLE001
        pass
    if which and Path(which).exists():
        return Path(which).resolve()
    # Fall back to the console script next to the current interpreter. Only
    # trust it when it actually exists; registering a desktop entry that
    # points at a missing binary makes KDE report "Could not find the
    # program". Callers that persist the path must check existence.
    fallback = Path(sys.executable).parent / "lexaloud"
    if fallback.exists():
        return fallback
    return Path(which).resolve() if which else fallback


def _xdg_config_home() -> Path:
    value = os.environ.get("XDG_CONFIG_HOME")
    return Path(value) if value else Path.home() / ".config"


def _xdg_data_home() -> Path:
    value = os.environ.get("XDG_DATA_HOME")
    return Path(value) if value else Path.home() / ".local" / "share"


def kde_shortcuts_path() -> Path:
    return _xdg_data_home() / "applications" / "lexaloud.desktop"


def kde_sync_marker_path() -> Path:
    """Record of the Exec command KGlobalAccel last imported.

    Lives in the runtime dir (cleared on logout) so every session
    re-verifies the import once; the expensive delete/rebuild cycle then
    only runs when the Exec actually changed.
    """
    return runtime_dir() / "lexaloud" / "kde-shortcuts-registered.exec"


def _desktop_exec(binary: Path, tail: str = "") -> str:
    """Return a safely quoted Desktop Entry Exec value."""
    escaped = str(binary).replace("\\", "\\\\").replace('"', '\\"')
    return f'"{escaped}"{tail}'


def _run_kbuildsycoca() -> bool:
    """Refresh Plasma's service cache so desktop-file changes propagate.

    KGlobalAccel only re-evaluates desktop components when KSycoca emits its
    database-changed notification; an incremental no-op rebuild is not enough
    after a file rewrite (KDE bug 487941). A forced full rebuild guarantees
    both the cache update and the notification.
    """
    for command in ("kbuildsycoca6", "kbuildsycoca5"):
        try:
            result = subprocess.run(
                [command, "--noincremental"],
                check=False,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                timeout=30,
            )
        except (FileNotFoundError, subprocess.SubprocessError):
            continue
        if result.returncode == 0:
            return True
    return False


def cleanup_kde_shortcuts() -> None:
    """Remove the transient KDE desktop entry on exit.

    The file is created by :func:`ensure_kde_shortcuts` only for the
    lifetime of the running app. Leaving it behind would pollute the
    user's applications menu with a stale entry pointing at a specific
    AppImage path, and the next launch would have to work around KDE
    bug 487941 to re-import it. Remove the file and its sync marker
    and refresh the sycoca cache. Best-effort — never raises.
    """
    try:
        from .platform import detect_desktop

        if not detect_desktop().is_kde:
            return
    except Exception:  # noqa: BLE001
        pass
    for path in (kde_shortcuts_path(), kde_sync_marker_path()):
        try:
            if path.exists():
                path.unlink()
        except OSError as e:
            log.debug("cleanup_kde_shortcuts: failed to remove %s: %s", path, e)
    # Don't warn if sycoca refresh fails during shutdown — session may already
    # be tearing down.
    try:
        _run_kbuildsycoca()
    except Exception:  # noqa: BLE001
        pass


def ensure_kde_shortcuts() -> bool:
    """Install/update the transient KDE launcher entry.

    The global hotkeys (Meta+R / Meta+P) are handled in-process by the
    running app via KGlobalAccel D-Bus (no per-press AppImage spawn).
    The desktop file exists only while the app is running, for the
    application menu. It contains no X-KDE-Shortcuts — the hotkeys live
    solely in the in-process component "lexaloud", so when the app isn't
    running Meta+R does nothing, as requested. The file is removed on
    exit by :func:`cleanup_kde_shortcuts`.
    """
    try:
        from .platform import detect_desktop

        if not detect_desktop().is_kde:
            return False
        path = kde_shortcuts_path()
        executable = binary_path()
        if not executable.exists():
            log.error(
                "KDE shortcut registration skipped: %s does not exist; "
                "refusing to register a shortcut that would report "
                "'Could not find the program'",
                executable,
            )
            return False
        # Transient launcher only — no Actions with X-KDE-Shortcuts.
        # Hotkeys are handled in-process via KGlobalAccel (see
        # _KGlobalAccelManager) so Meta+R is instant and does nothing
        # when the app isn't running, as requested.
        content = "\n".join(
            [
                "[Desktop Entry]",
                "Type=Application",
                "Name=Lexaloud",
                "GenericName=Text to Speech",
                "Comment=Read highlighted text aloud with local Kokoro voices",
                f"Exec={_desktop_exec(executable)}",
                "Icon=lexaloud",
                "Terminal=false",
                "Categories=AudioVideo;Audio;Accessibility;",
                "",
            ]
        )
        imported_exec = _desktop_exec(executable)
        marker = kde_sync_marker_path()
        already_synced = (
            path.exists()
            and path.read_text() == content
            and marker.exists()
            and marker.read_text() == imported_exec
        )
        if already_synced:
            return True
        marker.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        path.parent.mkdir(parents=True, exist_ok=True)
        if path.exists():
            path.unlink()
            _run_kbuildsycoca()
        path.write_text(content)
        if not _run_kbuildsycoca():
            log.warning(
                "kbuildsycoca6 did not run successfully; KDE may not pick up "
                "the shortcut entry until the next login"
            )
        marker.write_text(imported_exec)
        return True
    except (OSError, subprocess.SubprocessError) as e:
        log.error("KDE shortcut registration failed: %s", e)
        return False


class _KGlobalAccelManager(QtCore.QObject):
    """In-process global hotkey handler via KGlobalAccel D-Bus.

    Registers Meta+R (speak) and Meta+P (toggle) directly with KWin's
    kglobalaccel daemon. When the hotkey is pressed KWin delivers
    ``globalShortcutPressed`` to this process, so we can capture the
    selection in-process (no per-press AppImage spawn) and POST to the
    daemon. This is the fast path the user asked for: the AppImage is
    loaded once at login, hotkeys are instant after that.
    """

    def __init__(self, controller: DaemonController) -> None:
        super().__init__()
        self._controller = controller
        self._bus = None
        self._iface = None
        self._registered = False
        self._capture_deadline = 0.0
        self._capture_previous_text = ""
        self._capture_sentinel = ""
        try:
            from PySide6.QtDBus import QDBusConnection, QDBusInterface

            bus = QDBusConnection.sessionBus()
            if not bus.isConnected():
                log.warning("KGlobalAccel in-process hotkeys unavailable: no session bus")
                return
            # Keep a reference to the bus to prevent it being GC'd.
            self._bus = bus
            iface = QDBusInterface("org.kde.kglobalaccel", "/kglobalaccel", "org.kde.KGlobalAccel", bus)
            if not iface.isValid():
                log.warning("KGlobalAccel not available: %s", iface.lastError().message() if hasattr(iface, "lastError") else "invalid")
                return
            self._iface = iface
            # Register a regular KGlobalAccel component "lexaloud" (distinct
            # from the transient desktop file's lexaloud_desktop). The daemon
            # will deliver globalShortcutPressed signals to us on
            # org.kde.kglobalaccel:/component/lexaloud. When the app exits
            # the component becomes inactive, so Meta+R does nothing — no
            # fallback AppImage spawn, as requested.
            # Use busctl only for setShortcut's ai argument, which PySide6
            # incorrectly marshals as av. doRegister must originate from this
            # process's persistent D-Bus connection.
            import subprocess as _sp

            actions = [
                ("speak-selection", "Speak highlighted selection", "Meta+R"),
                ("toggle", "Pause / resume", "Meta+P"),
            ]
            for action, text, default_key in actions:
                action_id = ["lexaloud", action, "Lexaloud", text]
                try:
                    from PySide6.QtGui import QKeySequence

                    seq = QKeySequence(default_key)
                    key_int = seq[0].toCombined() if not seq.isEmpty() else 0
                except Exception:
                    key_int = 0
                reply = iface.call("doRegister", action_id)
                if reply.errorName():
                    log.warning("KGlobalAccel doRegister failed for %s: %s", action, reply.errorMessage())
                    return
                # getComponent ensures the daemon creates the object
                iface.call("getComponent", "lexaloud")
                if key_int:
                    # KGlobalAccel's SetPresent flag is 2. Without it the key
                    # is saved in kglobalshortcutsrc but is not grabbed.
                    result = _sp.run(
                        ["busctl", "--user", "call", "org.kde.kglobalaccel", "/kglobalaccel", "org.kde.KGlobalAccel", "setShortcut", "asaiu", "4", *action_id, "1", str(key_int), "2"],
                        check=False,
                        stdout=_sp.DEVNULL,
                        stderr=_sp.DEVNULL,
                        timeout=2,
                    )
                    if result.returncode != 0:
                        log.warning("KGlobalAccel setShortcut failed for %s", action)
                        return
            component = QDBusInterface(
                "org.kde.kglobalaccel",
                "/component/lexaloud",
                "org.kde.kglobalaccel.Component",
                bus,
            )
            active_reply = component.call("isActive")
            if active_reply.errorName() or not active_reply.arguments() or not active_reply.arguments()[0]:
                log.error("KGlobalAccel rejected Meta+R / Meta+P; component is inactive")
                return
            # Listen for key release so Meta is no longer held when selection
            # capture injects Ctrl+C. Handling key-down would produce
            # Meta+Ctrl+C instead of Copy.
            # No registerObject needed — we just connect to the daemon's
            # component signals. The daemon owns /component/lexaloud on
            # org.kde.kglobalaccel; we receive its globalShortcutPressed.
            from PySide6.QtCore import SLOT

            if not bus.connect(
                "org.kde.kglobalaccel",
                "/component/lexaloud",
                "org.kde.kglobalaccel.Component",
                "globalShortcutReleased",
                self,
                SLOT("globalShortcutReleased(QString,QString,qlonglong)"),
            ):
                log.warning("KGlobalAccel: bus.connect for globalShortcutReleased failed: %s", bus.lastError().message())
                return
            self._registered = True
            QtCore.QCoreApplication.instance().aboutToQuit.connect(self.close)
            log.info("KGlobalAccel in-process hotkeys registered (Meta+R / Meta+P) on lexaloud")
        except Exception as e:  # noqa: BLE001
            log.warning("KGlobalAccel in-process registration failed: %s", e, exc_info=True)

    @QtCore.Slot(str, str, "qlonglong")
    def globalShortcutReleased(self, component: str, action: str, timestamp: int) -> None:  # noqa: ARG002
        if component != "lexaloud":
            return
        if action == "speak-selection":
            self._handle_speak()
        elif action == "toggle":
            self._handle_toggle()

    @QtCore.Slot()
    def close(self) -> None:
        if not self._registered or self._iface is None:
            return
        self._registered = False
        for action in ("speak-selection", "toggle"):
            self._iface.call("unregister", "lexaloud", action)

    def _handle_speak(self) -> None:
        # Use a single-shot timer so we don't block the D-Bus thread.
        QtCore.QTimer.singleShot(0, self._do_speak)

    def _do_speak(self) -> None:
        try:
            from PySide6.QtGui import QClipboard, QGuiApplication

            clipboard = QGuiApplication.clipboard()
            from .session import detect_session

            if not detect_session().is_wayland:
                mime = clipboard.mimeData(mode=QClipboard.Mode.Selection)
                if mime and mime.hasText() and mime.text().strip():
                    self._post_text(mime.text().strip())
                    return

            # Wayland does not expose another application's PRIMARY selection
            # reliably. Claim CLIPBOARD with a unique sentinel so stale content
            # can never be mistaken for a successful copy, then accept only a
            # clipboard change caused by the injected Ctrl+C.
            from uuid import uuid4

            from .selection import try_force_copy

            previous_text = clipboard.text(mode=QClipboard.Mode.Clipboard)
            sentinel = f"lexaloud-capture-{uuid4()}"
            sentinel_owner = subprocess.run(
                ["wl-copy"],
                input=sentinel.encode(),
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                timeout=0.5,
                check=False,
            )
            if sentinel_owner.returncode != 0:
                raise RuntimeError("wl-copy could not establish fresh clipboard ownership")

            if not try_force_copy(timeout_s=0.5):
                if previous_text:
                    clipboard.setText(previous_text, mode=QClipboard.Mode.Clipboard)
                raise RuntimeError("no working keyboard injector is available")
            import time as _time

            self._capture_deadline = _time.monotonic() + 1.0
            self._capture_previous_text = previous_text
            self._capture_sentinel = sentinel
            # Return to Qt's event loop so the Wayland clipboard-change event
            # from the focused application can be delivered.
            QtCore.QTimer.singleShot(0, self._poll_fresh_clipboard)
        except Exception as e:  # noqa: BLE001
            log.exception("in-process speak failed: %s", e)

    def _poll_fresh_clipboard(self) -> None:
        import time

        from PySide6.QtGui import QClipboard, QGuiApplication

        clipboard = QGuiApplication.clipboard()
        text = ""
        try:
            from .config import load_config
            from .selection import read_clipboard

            cfg = load_config()
            text = read_clipboard(cfg.capture.max_bytes, 0.2).text.strip()
        except Exception:
            pass
        if text and text != self._capture_sentinel:
            self._capture_sentinel = ""
            self._capture_previous_text = ""
            self._post_text(text)
            return
        if time.monotonic() < self._capture_deadline:
            QtCore.QTimer.singleShot(50, self._poll_fresh_clipboard)
            return
        if self._capture_previous_text:
            clipboard.setText(self._capture_previous_text, mode=QClipboard.Mode.Clipboard)
        self._capture_sentinel = ""
        self._capture_previous_text = ""
        from .selection import try_notify

        try_notify("Select text first", "Lexaloud: no selection found. Select text and press Meta+R again.")

    def _post_text(self, text: str) -> None:
        try:
            import httpx

            with httpx.Client(
                transport=httpx.HTTPTransport(uds=str(socket_path())),
                base_url="http://lexaloud",
                timeout=httpx.Timeout(4.0, connect=1.0),
            ) as client:
                client.post("/speak", json={"text": text, "mode": "replace"})
        except Exception as e:  # noqa: BLE001
            log.error("failed to POST /speak to daemon: %s", e)

    def _handle_toggle(self) -> None:
        QtCore.QTimer.singleShot(0, self._do_toggle)

    def _do_toggle(self) -> None:
        try:
            import httpx

            with httpx.Client(
                transport=httpx.HTTPTransport(uds=str(socket_path())),
                base_url="http://lexaloud",
                timeout=httpx.Timeout(4.0, connect=1.0),
            ) as client:
                client.post("/toggle")
        except Exception as e:  # noqa: BLE001
            log.error("failed to POST /toggle: %s", e)


def autostart_path() -> Path:
    return _xdg_config_home() / "autostart" / "lexaloud.desktop"


def autostart_enabled() -> bool:
    return autostart_path().exists()


def set_autostart(enabled: bool) -> bool:
    """Create or remove the XDG autostart entry. Best-effort; returns success."""
    path = autostart_path()
    try:
        if not enabled:
            if path.exists():
                path.unlink()
            return True
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            "\n".join(
                [
                    AUTOSTART_SECTION,
                    "Type=Application",
                    "Name=Lexaloud",
                    "GenericName=Text to Speech",
                    "Comment=Local Kokoro text-to-speech tool",
                    f"Exec={binary_path()}",
                    "Terminal=false",
                    "Categories=AudioVideo;Audio;Accessibility;",
                    "X-GNOME-Autostart-enabled=true",
                    "",
                ]
            )
        )
        return True
    except OSError as e:
        log.error("autostart entry update failed: %s", e)
        return False


def onboarded_marker_path() -> Path:
    return _xdg_config_home() / "lexaloud" / "app-onboarded"


def is_onboarded() -> bool:
    return onboarded_marker_path().exists()


def mark_onboarded() -> None:
    try:
        onboarded_marker_path().parent.mkdir(parents=True, exist_ok=True)
        onboarded_marker_path().write_text("")
    except OSError as e:
        log.error("could not write onboarding marker: %s", e)


# ---------- daemon lifecycle ---------------------------------------------


def daemon_alive(timeout: float = 1.0) -> bool:
    """True if a daemon is answering on the Unix socket right now."""
    try:
        import httpx

        with httpx.Client(
            transport=httpx.HTTPTransport(uds=str(socket_path())),
            base_url="http://lexaloud",
            timeout=httpx.Timeout(timeout, connect=timeout),
        ) as client:
            return client.get("/state").status_code == 200
    except Exception:  # noqa: BLE001
        return False


class DaemonController:
    """Owns the in-app daemon subprocess.

    The daemon runs as a child process (`<binary> daemon`) so it has the
    same process shape as the systemd unit — including clean stdout/stderr
    separation — while being started and stopped with the app.
    """

    def __init__(self, binary: Path | None = None) -> None:
        self.binary = binary or binary_path()
        self._proc: subprocess.Popen | None = None

    @property
    def running(self) -> bool:
        return self._proc is not None and self._proc.poll() is None

    def start(self) -> bool:
        """Start the daemon child if not already running. Returns success."""
        if self.running:
            return True
        if daemon_alive():
            # Adopt a daemon that was started out-of-band (e.g. systemd).
            log.info("daemon already answering; adopting instead of starting")
            return True
        if getattr(sys, "frozen", False):
            # The outer AppImage launcher forks an extracted executable and is
            # not the daemon itself. Launch the internal frozen executable so
            # this controller owns the real daemon process and can reliably
            # stop/restart it while the parent's extraction directory exists.
            cmd = [sys.executable, "daemon"]
        else:
            # Source/venv installs: always go through the same
            # interpreter instead of resolving a console script, which
            # can be shadowed or missing on PATH.
            cmd = [sys.executable, "-m", "lexaloud", "daemon"]
        log_file = None
        try:
            log_path = runtime_dir() / "lexaloud" / "daemon.log"
            log_path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
            log_file = log_path.open("ab")
            self._proc = subprocess.Popen(
                cmd,
                stdin=subprocess.DEVNULL,
                stdout=log_file,
                stderr=subprocess.STDOUT,
            )
        except (OSError, subprocess.SubprocessError) as e:
            log.error("failed to start daemon subprocess: %s", e)
            self._proc = None
            return False
        finally:
            if log_file is not None:
                log_file.close()
        return True

    def stop(self, timeout: float = 8.0) -> None:
        proc, self._proc = self._proc, None
        if proc is None or proc.poll() is not None:
            return
        try:
            proc.terminate()
            proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            proc.kill()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                log.error("daemon subprocess refused to die after SIGKILL")
        except OSError as e:
            log.error("error stopping daemon subprocess: %s", e)


# ---------- onboarding helpers --------------------------------------------


def ensure_default_keybindings() -> list[str]:
    """Create default global shortcuts where the desktop allows it.

    Returns the shortcut labels that were created. KDE is handled separately
    by :func:`ensure_kde_shortcuts` because its native desktop actions support
    both persistent registration and System Settings visibility.
    """
    created: list[str] = []
    try:
        from .gui_control.keybindings import SHORTCUTS, detect_backend
    except Exception:  # noqa: BLE001
        log.debug("keybinding backend unavailable", exc_info=True)
        return created
    try:
        backend = detect_backend()
        if not backend.is_available():
            return created
        for shortcut_id, label, _tail in SHORTCUTS:
            if shortcut_id not in DEFAULT_KEYBINDINGS:
                continue
            if backend.get_binding(shortcut_id) in ("(unset)", ""):
                if backend.set_binding(shortcut_id, DEFAULT_KEYBINDINGS[shortcut_id]):
                    created.append(label)
    except Exception:  # noqa: BLE001
        log.debug("default keybinding creation failed", exc_info=True)
    return created


class ModelDownloadWorker(QtCore.QThread):
    """Download missing model artifacts off the GUI thread.

    Emits ``progress(done_bytes, total_bytes, filename)`` while running
    and ``finished_with(str)`` with an error message on failure (empty
    string on success).
    """

    progress = QtCore.Signal(int, int, str)
    finished_with = QtCore.Signal(str)

    def __init__(self) -> None:
        super().__init__()
        self._cancelled = False

    def cancel(self) -> None:
        self._cancelled = True

    def run(self) -> None:
        from .models import ARTIFACTS, ArtifactError, default_cache_dir, ensure_artifacts

        cache = default_cache_dir()
        missing = [art for art in ARTIFACTS if not (cache / art.filename).exists()]
        total = sum(art.expected_size for art in missing)
        done_before = 0

        def cb(filename: str, file_bytes: int) -> None:
            if self._cancelled:
                raise KeyboardInterrupt
            self.progress.emit(done_before + file_bytes, total, filename)

        try:
            ensure_artifacts(progress_cb=cb if missing else None)
        except KeyboardInterrupt:
            self.finished_with.emit("Model download cancelled.")
            return
        except ArtifactError as e:
            self.finished_with.emit(str(e))
            return
        except Exception as e:  # noqa: BLE001
            log.exception("model download failed")
            self.finished_with.emit(f"Model download failed: {e}")
            return
        self.finished_with.emit("")


# ---------- onboarding orchestration --------------------------------------


def _ask_autostart() -> None:
    """First-run question: start Lexaloud at login?"""
    answer = QtWidgets.QMessageBox.question(
        None,
        "Lexaloud",
        "Start Lexaloud automatically when you log in?\n\n"
        "The tray icon and the speech service will be ready right after "
        "login. You can change this any time from the tray menu "
        '("Start with desktop").',
        QtWidgets.QMessageBox.StandardButton.Yes | QtWidgets.QMessageBox.StandardButton.No,
        QtWidgets.QMessageBox.StandardButton.Yes,
    )
    set_autostart(answer == QtWidgets.QMessageBox.StandardButton.Yes)


class Onboarding(QtWidgets.QDialog):
    """Non-blocking first-run model download with a progress dialog."""

    def __init__(self, on_done: Callable[[], None]) -> None:
        super().__init__(None)
        self.setWindowTitle("Lexaloud — preparing speech")
        self.setMinimumWidth(420)
        self._on_done = on_done

        layout = QtWidgets.QVBoxLayout(self)
        layout.setSpacing(8)
        self._status = QtWidgets.QLabel("Downloading the Kokoro speech model…")
        layout.addWidget(self._status)
        self._bar = QtWidgets.QProgressBar()
        self._bar.setRange(0, 100)
        layout.addWidget(self._bar)
        self._file_label = QtWidgets.QLabel("")
        self._file_label.setStyleSheet("color: gray;")
        layout.addWidget(self._file_label)

        self._worker = ModelDownloadWorker()
        self._worker.progress.connect(self._on_progress)
        self._worker.finished_with.connect(self._on_finished)

    def start(self) -> None:
        self.show()
        self._worker.start()

    def _on_progress(self, done: int, total: int, filename: str) -> None:
        if total > 0:
            self._bar.setValue(int(done * 100 / total))
        self._file_label.setText(f"{filename}: {done / (1 << 20):.0f} / {total / (1 << 20):.0f} MB")

    def _on_finished(self, error: str) -> None:
        self.accept()
        if error:
            retry = QtWidgets.QMessageBox.critical(
                None,
                "Lexaloud — model download failed",
                f"{error}\n\nRetry the download? Lexaloud needs the model before it can speak.",
                QtWidgets.QMessageBox.StandardButton.Retry
                | QtWidgets.QMessageBox.StandardButton.Close,
                QtWidgets.QMessageBox.StandardButton.Retry,
            )
            if retry == QtWidgets.QMessageBox.StandardButton.Retry:
                self._worker = ModelDownloadWorker()
                self._worker.progress.connect(self._on_progress)
                self._worker.finished_with.connect(self._on_finished)
                self.start()
                return
            QtWidgets.QApplication.quit()
            return
        self._on_done()


# ---------- entry point ----------------------------------------------------


def main() -> int:
    """Entry point for `lexaloud` / `lexaloud app` / `lexaloud tray`."""
    # Single instance: the tray/app share the indicator lock.
    from .indicator import _acquire_single_instance_lock

    lock = _acquire_single_instance_lock()
    if lock is None:
        print("Lexaloud is already running.", file=sys.stderr)
        return 0
    if isinstance(lock, int):
        globals()["_instance_lock_fd"] = lock

    app = QtWidgets.QApplication.instance() or QtWidgets.QApplication(sys.argv)
    app.setApplicationName("Lexaloud")
    app.setQuitOnLastWindowClosed(False)

    # Repair/migrate shortcut registration for existing users as well as new
    # installs. This is intentionally not gated by the onboarding marker.
    ensure_kde_shortcuts()

    binary = binary_path()
    controller = DaemonController(binary)

    unit_exists = (
        Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))
        .joinpath("systemd/user/lexaloud.service")
        .exists()
    )
    systemd_mode = unit_exists and daemon_alive()

    from .indicator import LexaloudTray, _icon_path

    try:
        icon_path = _icon_path()
    except FileNotFoundError as e:
        print(f"Lexaloud: {e}", file=sys.stderr)
        return 2

    tray = LexaloudTray(
        icon_path,
        daemon=controller,
        systemd_mode=systemd_mode,
    )
    tray.show()

    # In-process global hotkeys (fast path): when the app is running, handle
    # Meta+R / Meta+P directly via KGlobalAccel D-Bus so we don't spawn a
    # new AppImage per press. The desktop file's Exec is `true` when the app
    # is not running the hotkey does nothing, as requested.
    _hotkey_manager = None
    try:
        from .platform import detect_desktop

        if detect_desktop().is_kde:
            _hotkey_manager = _KGlobalAccelManager(controller)
            # Keep a reference on the app to prevent GC.
            app._lexaloud_hotkey_manager = _hotkey_manager  # type: ignore[attr-defined]
    except Exception as e:  # noqa: BLE001
        log.debug("in-process hotkey manager not started: %s", e)

    # Qt's event loop does not exit on SIGINT/SIGTERM by default; without
    # this, closing the session (SIGTERM) would orphan the daemon child.
    import signal

    for sig in (signal.SIGINT, signal.SIGTERM):
        signal.signal(sig, lambda *_: app.quit())

    def start_daemon_and_finish() -> None:
        if not systemd_mode and not daemon_alive():
            controller.start()
        mark_onboarded()
        tray.refresh_state()

    first_run = not is_onboarded()
    if first_run:
        ensure_default_keybindings()
        _ask_autostart()
        onboarding = Onboarding(start_daemon_and_finish)
        onboarding.start()
    elif not daemon_alive():
        # Return visit: models are already cached, start immediately.
        if not systemd_mode:
            controller.start()
        tray.refresh_state()

    app.exec()

    controller.stop()
    tray.hide()
    cleanup_kde_shortcuts()
    return 0

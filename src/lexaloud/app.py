"""Lexaloud desktop application mode.

`lexaloud` (or `lexaloud app`) is the "normal GUI app" entry point: it
shows the tray icon, runs the daemon, and performs first-run onboarding
without ever asking the user to open a terminal:

1. If the Kokoro model is not cached, download it automatically with a
   progress dialog (one-time, ~350 MB).
2. Create default global shortcuts when the desktop allows it
   (GNOME gsettings / XFCE xfconf; KDE Plasma gets its shortcuts
   registered by the daemon through the GlobalShortcuts portal).
3. Offer to start Lexaloud automatically at login (XDG autostart).
4. Start the daemon as a child process and hand control to the tray.

No systemd unit is required: the daemon lives exactly as long as the
app does, and "Start with desktop" replaces the service for autostart.
The systemd integration stays available as an opt-in CLI path
(`lexaloud setup`) and through the tray's service actions for users who
prefer it; if a unit is installed and running, the app adopts the
running daemon instead of starting a second one.
"""

from __future__ import annotations

import logging
import os
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path

from PySide6 import QtCore, QtWidgets

from .config import socket_path

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
    if which:
        return Path(which).resolve()
    return Path(sys.executable).parent / "lexaloud"


def _xdg_config_home() -> Path:
    value = os.environ.get("XDG_CONFIG_HOME")
    return Path(value) if value else Path.home() / ".config"


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
            cmd = [str(self.binary), "daemon"]
        else:
            # Source/venv installs: always go through the same
            # interpreter instead of resolving a console script, which
            # can be shadowed or missing on PATH.
            cmd = [sys.executable, "-m", "lexaloud", "daemon"]
        try:
            self._proc = subprocess.Popen(
                cmd,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        except (OSError, subprocess.SubprocessError) as e:
            log.error("failed to start daemon subprocess: %s", e)
            self._proc = None
            return False
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

    Returns the shortcut labels that were created. KDE Plasma and wlroots
    compositors get their shortcuts registered by the daemon through the
    GlobalShortcuts portal, so nothing is done here for those.
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
    return 0

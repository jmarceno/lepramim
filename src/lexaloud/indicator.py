"""Qt system-tray icon for Lexaloud.

Uses ``QSystemTrayIcon`` from PySide6 (Qt 6). The icon follows the
StatusNotifierItem DBus protocol, which every mainstream desktop supports:
GNOME (via the AppIndicator extension), KDE Plasma, XFCE, Cinnamon, MATE,
and most trays that support SNI. Unlike the previous GTK/AppIndicator
implementation it no longer needs the system ``python3-gi`` bindings —
PySide6 ships inside the package and inside the AppImage.

Menu responsibilities (kept deliberately small and action-oriented):

- display the current global shortcut the user must press,
- start/stop the daemon and trigger playback,
- open the control window,
- reinstall the user systemd service (re-runs ``setup``),
- remove the user systemd service (runs ``uninstall``),
- quit the tray.
"""

from __future__ import annotations

import logging
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path
from typing import TYPE_CHECKING

from PySide6 import QtCore, QtGui, QtSvg, QtWidgets

if TYPE_CHECKING:
    from .gui_control import ControlWindow

log = logging.getLogger(__name__)

SERVICE = "lexaloud.service"

POLL_INTERVAL_MS = 3_000

# The tray shows the same artwork in every state; stopped states are
# dimmed instead of swapped for a different glyph so the icon stays
# recognizable at a glance.
OPACITY_RUNNING = 1.0
OPACITY_STOPPED = 0.35


def _icon_path() -> Path:
    """Locate the bundled SVG icon in source installs and frozen builds."""
    candidate = Path(__file__).resolve().parent / "icons" / "lexaloud.svg"
    if candidate.is_file():
        return candidate
    # PyInstaller fallback (not used by the AppImage's onedir bundle, but
    # harmless to check).
    meipass = getattr(sys, "_MEIPASS", None)
    if meipass:
        candidate = Path(meipass) / "lexaloud" / "icons" / "lexaloud.svg"
        if candidate.is_file():
            return candidate
    raise FileNotFoundError("lexaloud.svg icon not found in package data")


def _tinted_icon(path: Path, opacity: float) -> QtGui.QIcon:
    """Render the SVG at a fixed size and apply a flat opacity."""
    renderer = QtSvg.QSvgRenderer(str(path))
    image = QtGui.QImage(64, 64, QtGui.QImage.Format_ARGB32)
    image.fill(QtCore.Qt.GlobalColor.transparent)
    painter = QtGui.QPainter(image)
    painter.setOpacity(opacity)
    renderer.render(painter)
    painter.end()
    pixmap = QtGui.QPixmap.fromImage(image)
    pixmap.setDevicePixelRatio(2.0)  # crisp on HiDPI panels
    return QtGui.QIcon(pixmap)


def _notify(summary: str, body: str = "") -> None:
    """Fire a best-effort notify-send; never raises."""
    try:
        args = ["notify-send", "--app-name", "Lexaloud", "--expire-time", "4000", "--", summary]
        if body:
            args.append(body)
        subprocess.Popen(
            args,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    except Exception as e:  # noqa: BLE001
        log.debug("notify-send failed: %s", e)


def _daemon_active() -> bool:
    try:
        r = subprocess.run(
            ["systemctl", "--user", "is-active", SERVICE],
            capture_output=True,
            text=True,
            timeout=2,
        )
        return r.stdout.strip() == "active"
    except Exception:  # noqa: BLE001
        return False


def _daemon_state() -> str:
    """Ask the daemon what state it is in via GET /state.

    Returns the state string ("idle", "warming", "speaking", "paused")
    if the daemon answers, or "" on any failure. Uses httpx over the
    Unix domain socket at ``$XDG_RUNTIME_DIR/lexaloud/lexaloud.sock``.
    """
    try:
        import httpx

        from .config import socket_path

        with httpx.Client(
            transport=httpx.HTTPTransport(uds=str(socket_path())),
            base_url="http://lexaloud",
            timeout=httpx.Timeout(0.5, connect=0.5),
        ) as client:
            resp = client.get("/state")
            data = resp.json()
        return str(data.get("state", ""))
    except Exception:  # noqa: BLE001
        return ""


def _systemctl(action: str) -> int:
    """Run ``systemctl --user <action> lexaloud.service``, notify on failure."""
    try:
        r = subprocess.run(
            ["systemctl", "--user", action, SERVICE],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=10,
        )
    except subprocess.TimeoutExpired:
        log.error("systemctl %s %s timed out", action, SERVICE)
        _notify(
            f"Lexaloud: systemctl {action} timed out",
            "systemd --user may be unresponsive. Check `systemctl --user status`.",
        )
        return -1
    except (OSError, subprocess.SubprocessError) as e:
        log.error("systemctl %s %s failed to execute: %s", action, SERVICE, e)
        _notify(
            "Lexaloud: cannot invoke systemctl",
            f"Is systemctl on PATH? {e}",
        )
        return -1
    if r.returncode != 0:
        stderr = (r.stderr or b"").decode("utf-8", errors="replace").strip()
        log.error("systemctl %s exited %d: %s", action, r.returncode, stderr)
        _notify(
            f"Lexaloud: systemctl {action} failed",
            stderr[:200] or f"Exit code {r.returncode}",
        )
    return r.returncode


def _spawn_detached(args: list[str]) -> None:
    """Launch a CLI subcommand from a menu click, detached from the tray."""
    try:
        subprocess.Popen(
            args,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
    except (OSError, subprocess.SubprocessError) as e:
        log.error("Popen %s failed: %s", args[0], e)
        _notify("Lexaloud: could not invoke CLI", str(e))


def _lexaloud_binary() -> str:
    """Path of the CLI entry that shares this interpreter.

    In a venv install this is ``<venv>/bin/lexaloud``. In the frozen
    AppImage ``sys.executable`` *is* the CLI binary, so a plain
    ``<image> <subcommand>`` invocation works.
    """
    if getattr(sys, "frozen", False):
        return sys.executable
    return str(Path(sys.executable).parent / "lexaloud")


def _detect_backend_safe():
    """Best-effort keybinding backend; None keeps the tray usable."""
    try:
        from .gui_control.keybindings import detect_backend

        return detect_backend()
    except Exception:  # noqa: BLE001
        log.debug("keybinding backend detection failed", exc_info=True)
        return None


class _Worker(QtCore.QThread):
    """Run a blocking service action off the GUI thread; emit a signal."""

    finished_with = QtCore.Signal(str)  # human-readable result summary

    def __init__(self, label: str, fn: Callable[[], int]) -> None:
        super().__init__()
        self._label = label
        self._fn = fn

    def run(self) -> None:
        try:
            rc = int(self._fn() or 0)
        except Exception as e:  # noqa: BLE001
            log.exception("%s failed", self._label)
            self.finished_with.emit(f"{self._label} failed: {e}")
            return
        suffix = "" if rc == 0 else f" (exit code {rc})"
        self.finished_with.emit(f"{self._label} finished{suffix}")


class LexaloudTray(QtWidgets.QSystemTrayIcon):
    """Persistent status-bar icon and its menu."""

    def __init__(self, icon_path: Path) -> None:
        super().__init__(_tinted_icon(icon_path, OPACITY_STOPPED))
        self._icon_path = icon_path
        self._icons = {
            "running": _tinted_icon(icon_path, OPACITY_RUNNING),
            "stopped": _tinted_icon(icon_path, OPACITY_STOPPED),
        }
        self._current_icon: str | None = None

        self._kb_backend = _detect_backend_safe()

        self._menu = QtWidgets.QMenu()
        self._build_menu()
        self.setContextMenu(self._menu)
        self.setToolTip("Lexaloud: stopped")
        self._control_window: ControlWindow | None = None

        self._worker: _Worker | None = None  # keep a reference while running

        # Prime the state so the menu label and icon reflect reality.
        self._refresh_state()
        self._timer = QtCore.QTimer(self)
        self._timer.setInterval(POLL_INTERVAL_MS)
        self._timer.timeout.connect(self._refresh_state)
        self._timer.start()
        self.activated.connect(self._on_activated)

    # ---------- menu ----------

    def _build_menu(self) -> None:
        # Non-interactive row showing the global shortcut the user must
        # press to speak the current selection.
        self.item_shortcut = QtGui.QAction(self._shortcut_label())
        self.item_shortcut.setEnabled(False)
        self.item_shortcut.setToolTip("Global shortcut — press it with text selected")
        self._menu.addAction(self.item_shortcut)
        self._menu.addSeparator()

        self.item_toggle_daemon = QtGui.QAction("Start daemon")
        self.item_toggle_daemon.triggered.connect(self._on_toggle_daemon)
        self._menu.addAction(self.item_toggle_daemon)

        self.item_speak = QtGui.QAction("Speak highlighted selection")
        self.item_speak.triggered.connect(
            lambda: _spawn_detached([_lexaloud_binary(), "speak-selection"])
        )
        self._menu.addAction(self.item_speak)

        self.item_pause = QtGui.QAction("Pause / resume")
        self.item_pause.triggered.connect(lambda: _spawn_detached([_lexaloud_binary(), "toggle"]))
        self._menu.addAction(self.item_pause)

        self.item_stop = QtGui.QAction("Stop current playback")
        self.item_stop.triggered.connect(lambda: _spawn_detached([_lexaloud_binary(), "stop"]))
        self._menu.addAction(self.item_stop)

        self._menu.addSeparator()

        self.item_control = QtGui.QAction("Control window…")
        self.item_control.triggered.connect(self._on_control)
        self._menu.addAction(self.item_control)

        self._menu.addSeparator()

        self.item_reinstall = QtGui.QAction("Reinstall service…")
        self.item_reinstall.triggered.connect(self._on_reinstall)
        self._menu.addAction(self.item_reinstall)

        self.item_remove = QtGui.QAction("Remove service…")
        self.item_remove.triggered.connect(self._on_remove)
        self._menu.addAction(self.item_remove)

        self._menu.addSeparator()

        self.item_quit = QtGui.QAction("Quit Lexaloud")
        self.item_quit.triggered.connect(self._on_quit)
        self._menu.addAction(self.item_quit)

    def _shortcut_label(self) -> str:
        backend = self._kb_backend
        binding = backend.get_binding("lexaloud") if backend else ""
        if not binding or binding.startswith("("):
            return "Shortcut: not configured"
        return f"Shortcut: {binding}"

    # ---------- state polling ----------

    def _refresh_state(self) -> None:
        active = _daemon_active()
        # Distinguish warming from plain active by asking the daemon
        # directly; fall back to the systemctl is-active result.
        state_str = _daemon_state() if active else ""
        warming = state_str == "warming"

        if warming:
            desired = "running"  # same artwork; the tooltip carries the nuance
            tooltip = "Lexaloud: warming up"
            toggle_label = "Stop daemon (warming…)"
        elif active:
            desired = "running"
            tooltip = "Lexaloud: running"
            toggle_label = "Stop daemon (free GPU)"
        else:
            desired = "stopped"
            tooltip = "Lexaloud: stopped"
            toggle_label = "Start daemon"

        # Only update the icon when it actually changed, to avoid DBus
        # churn and tray flicker.
        if desired != self._current_icon:
            self.setIcon(self._icons[desired])
            self._current_icon = desired
        self.setToolTip(tooltip)
        if self.item_toggle_daemon.text() != toggle_label:
            self.item_toggle_daemon.setText(toggle_label)

        # Grey out playback actions unless the daemon is fully ready.
        ready_for_playback = active and not warming
        for action in (self.item_speak, self.item_pause, self.item_stop):
            action.setEnabled(ready_for_playback)

    # ---------- menu handlers ----------

    def _on_toggle_daemon(self) -> None:
        action = "stop" if _daemon_active() else "start"
        _systemctl(action)
        # Poll state again in 500ms to catch up with systemd.
        QtCore.QTimer.singleShot(500, self._refresh_state)

    def _on_control(self) -> None:
        # Import here so the tray starts even if the control window has a
        # transient problem.
        try:
            from .gui_control import ControlWindow
        except Exception as e:  # noqa: BLE001
            log.error("failed to import ControlWindow: %s", e)
            _notify("Lexaloud: control window unavailable", str(e))
            return
        # Reuse an existing window instance so repeated menu clicks don't
        # leak new windows on every open.
        if self._control_window is None or not self._control_window.isVisible():
            try:
                self._control_window = ControlWindow()
            except Exception as e:  # noqa: BLE001
                log.exception("ControlWindow construction failed: %s", e)
                _notify("Lexaloud: control window error", str(e)[:200])
                self._control_window = None
                return
        self._control_window.show()
        self._control_window.raise_()
        self._control_window.activateWindow()

    def _on_reinstall(self) -> None:
        if not self._confirm(
            "Reinstall Lexaloud service",
            "Re-run setup?\n\nThis re-downloads missing models, rewrites the "
            "user systemd unit, and leaves the service state untouched. "
            "Existing configuration is kept.",
        ):
            return
        self._run_service_action(
            label="Reinstall",
            fn=self._reinstall_fn,
            done=self._notify_result,
        )

    def _reinstall_fn(self) -> int:
        from .setup import run_setup

        return run_setup()

    def _on_remove(self) -> None:
        if not self._confirm(
            "Remove Lexaloud service",
            "Stop the daemon and remove its user systemd unit and desktop "
            "launcher?\n\nYour configuration and downloaded models are kept.",
        ):
            return
        self._run_service_action(
            label="Remove service",
            fn=self._remove_fn,
            done=self._notify_result,
        )

    def _remove_fn(self) -> int:
        from .uninstall import run_uninstall

        return run_uninstall()

    def _confirm(self, title: str, body: str) -> bool:
        answer = QtWidgets.QMessageBox.question(
            None,
            title,
            body,
            QtWidgets.QMessageBox.StandardButton.Yes | QtWidgets.QMessageBox.StandardButton.No,
            QtWidgets.QMessageBox.StandardButton.No,
        )
        return answer == QtWidgets.QMessageBox.StandardButton.Yes

    def _run_service_action(
        self,
        label: str,
        fn: Callable[[], int],
        done: Callable[[str], None],
    ) -> None:
        if self._worker is not None and self._worker.isRunning():
            _notify("Lexaloud", "Another service action is already running.")
            return
        self.item_reinstall.setEnabled(False)
        self.item_remove.setEnabled(False)
        worker = _Worker(label, fn)
        worker.finished_with.connect(done)
        worker.finished.connect(lambda: self._service_action_done(worker))
        worker.start()
        self._worker = worker

    def _service_action_done(self, worker: _Worker) -> None:
        worker.deleteLater()
        if self._worker is worker:
            self._worker = None
        self.item_reinstall.setEnabled(True)
        self.item_remove.setEnabled(True)
        self._refresh_state()

    def _notify_result(self, summary: str) -> None:
        _notify("Lexaloud", summary)

    def _on_quit(self) -> None:
        QtWidgets.QApplication.quit()

    def _on_activated(self, reason: QtWidgets.QSystemTrayIcon.ActivationReason) -> None:
        # Left click opens the menu too, so the tray is usable even on
        # desktops that do not show context menus on left click.
        if reason == QtWidgets.QSystemTrayIcon.ActivationReason.Trigger:
            self._menu.popup(QtGui.QCursor.pos())


def _acquire_single_instance_lock():
    """Take an exclusive flock on $XDG_RUNTIME_DIR/lexaloud-indicator.lock.

    Returns the open file descriptor (int) on success — the caller keeps
    it alive for the process lifetime. Returns None if another indicator
    is already running. Returns "skip" if we couldn't even create the
    runtime dir (rare; we then proceed without single-instance
    enforcement).

    Implementation notes:
    - We open with `O_RDWR | O_CREAT` (NOT "w" mode), so a second launcher
      does NOT truncate the winner's PID file while probing for the lock.
    - We only truncate and write the PID AFTER flock succeeds, so an
      informational `cat` of the lockfile shows the real winner's PID.
    - flock is released by the kernel when the process exits for any
      reason, including SIGKILL, so the lockfile is never stale.
    """
    import fcntl
    import os

    runtime = os.environ.get("XDG_RUNTIME_DIR") or f"/tmp/lexaloud-{os.getuid()}"
    try:
        os.makedirs(runtime, exist_ok=True)
    except OSError:
        return "skip"
    lock_path = os.path.join(runtime, "lexaloud-indicator.lock")
    try:
        fd = os.open(lock_path, os.O_RDWR | os.O_CREAT, 0o644)
    except OSError:
        return "skip"
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        os.close(fd)
        return None
    # We have the lock. Safe to truncate and write our PID for humans.
    try:
        os.ftruncate(fd, 0)
        os.write(fd, f"{os.getpid()}\n".encode())
    except OSError:
        pass
    return fd


def main() -> int:
    """Entry point for `lexaloud-indicator` / `lexaloud tray`."""
    lock = _acquire_single_instance_lock()
    if lock is None:
        print(
            "Lexaloud indicator is already running. Exiting.",
            file=sys.stderr,
        )
        return 0
    # If lock is "skip" we couldn't check; proceed anyway. If it's a real
    # file descriptor (int), keep it alive for the process lifetime by
    # stashing it on the module so the GC doesn't close it.
    if isinstance(lock, int):
        globals()["_instance_lock_fd"] = lock

    app = QtWidgets.QApplication.instance() or QtWidgets.QApplication(sys.argv)
    app.setApplicationName("Lexaloud")
    app.setQuitOnLastWindowClosed(False)  # closing the control window must
    # not kill the tray.

    try:
        icon_path = _icon_path()
    except FileNotFoundError as e:
        print(f"Lexaloud indicator: {e}", file=sys.stderr)
        return 2

    tray = LexaloudTray(icon_path)
    tray.show()
    tray.setVisible(True)

    app.exec()
    tray.hide()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

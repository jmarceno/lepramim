"""Floating overlay bar for Lexaloud — shows current sentence + playback controls.

A separate Qt 6 process (like the tray) that displays a small translucent
bar at the bottom-center of the primary monitor while Lexaloud is
speaking. Auto-hides when idle/warming, auto-shows when speaking/paused.

On Wayland the window manager decides stacking for ordinary windows; the
bar requests stays-on-top plus a notification window type, which KDE
Plasma, Hyprland, and sway honour. On X11 the same hints map to
keep-above and the notification window type.
"""

from __future__ import annotations

import logging
import sys
from collections.abc import Callable

import httpx
from PySide6 import QtCore, QtGui, QtWidgets

from .config import socket_path

log = logging.getLogger(__name__)

# --- constants ------------------------------------------------------------

POLL_INTERVAL_MS = 200
BAR_WIDTH = 500
BAR_HEIGHT = 80
CORNER_RADIUS = 16
BG_RGBA = (0.1, 0.1, 0.1, 0.85)
TEXT_COLOR = "#ffffff"
BOTTOM_MARGIN = 24

# Unicode button labels
LABEL_PAUSE = "\u23f8"  # ⏸
LABEL_PLAY = "\u23f5"  # ⏵ (resume)
LABEL_SKIP = "\u23ed"  # ⏭
LABEL_STOP = "\u23f9"  # ⏹


class OverlayWindow(QtWidgets.QWidget):
    """Translucent floating bar with sentence text and playback controls."""

    def __init__(self) -> None:
        super().__init__(
            None,
            QtCore.Qt.WindowType.FramelessWindowHint
            | QtCore.Qt.WindowType.WindowStaysOnTopHint
            | QtCore.Qt.WindowType.Tool,
        )  # skip taskbar and pager
        self.setAttribute(QtCore.Qt.WidgetAttribute.WA_TranslucentBackground)
        self.setAttribute(QtCore.Qt.WidgetAttribute.WA_ShowWithoutActivating)
        # Notification-style window on X11: most window managers keep
        # those above ordinary windows and off the taskbar.
        self.setWindowFlag(QtCore.Qt.WindowType.X11BypassWindowManagerHint, False)

        self._visible = False
        self._last_state: str | None = None

        self._client: httpx.Client | None = None
        self._ensure_client()

        self._setup_window()
        self._build_ui()
        self._position_window()

        # Start polling daemon state.
        self._timer = QtCore.QTimer(self)
        self._timer.setInterval(POLL_INTERVAL_MS)
        self._timer.timeout.connect(self._poll_state)
        self._timer.start()

    # --- window setup -----------------------------------------------------

    def _setup_window(self) -> None:
        self.setFixedSize(BAR_WIDTH, BAR_HEIGHT)
        self.setWindowTitle("Lexaloud Overlay")

    def _build_ui(self) -> None:
        layout = QtWidgets.QHBoxLayout(self)
        layout.setContentsMargins(16, 8, 12, 8)
        layout.setSpacing(8)

        # Sentence label — takes most of the width.
        self._label = QtWidgets.QLabel("")
        self._label.setStyleSheet(f"color: {TEXT_COLOR}; font-size: 14px; font-weight: 500;")
        self._label.setTextInteractionFlags(QtCore.Qt.TextInteractionFlag.NoTextInteraction)
        layout.addWidget(self._label, 1)

        # Button box — right-aligned.
        self._btn_pause = self._make_button(LABEL_PAUSE, self._on_pause_resume)
        self._btn_skip = self._make_button(LABEL_SKIP, self._on_skip)
        self._btn_stop = self._make_button(LABEL_STOP, self._on_stop)
        layout.addWidget(self._btn_pause)
        layout.addWidget(self._btn_skip)
        layout.addWidget(self._btn_stop)

    def _make_button(self, label: str, callback: Callable[[], None]) -> QtWidgets.QToolButton:
        btn = QtWidgets.QToolButton()
        btn.setText(label)
        btn.setToolButtonStyle(QtCore.Qt.ToolButtonStyle.ToolButtonTextOnly)
        btn.setAutoRaise(True)
        btn.setFocusPolicy(QtCore.Qt.FocusPolicy.NoFocus)
        btn.setStyleSheet(
            "QToolButton { color: rgba(255,255,255,0.9); font-size: 20px;"
            " padding: 4px 10px; border: none; background: transparent; }"
            "QToolButton:hover { background: rgba(255,255,255,0.15);"
            " border-radius: 6px; }"
        )
        btn.clicked.connect(callback)
        return btn

    def _position_window(self) -> None:
        """Center the bar at the bottom of the primary screen."""
        screen = QtGui.QGuiApplication.primaryScreen()
        if screen is None:
            return
        geom = screen.availableGeometry()
        x = geom.x() + (geom.width() - BAR_WIDTH) // 2
        y = geom.y() + geom.height() - BAR_HEIGHT - BOTTOM_MARGIN
        self.setGeometry(x, y, BAR_WIDTH, BAR_HEIGHT)

    # --- painting (rounded translucent background) -------------------------

    def paintEvent(self, event) -> None:  # noqa: N802 (Qt naming)
        painter = QtGui.QPainter(self)
        painter.setRenderHint(QtGui.QPainter.RenderHint.Antialiasing)
        painter.setPen(QtCore.Qt.PenStyle.NoPen)
        painter.setBrush(QtGui.QColor(*[int(c * 255) for c in BG_RGBA[:3]], int(BG_RGBA[3] * 255)))
        r = CORNER_RADIUS
        painter.drawRoundedRect(QtCore.QRectF(0, 0, self.width(), self.height()), r, r)

    # --- state polling ----------------------------------------------------

    def _poll_state(self) -> None:
        """Fetch daemon state and update the overlay."""
        state_data = self._fetch_state()
        if state_data is None:
            self._update_visibility("idle")
            return

        state = state_data.get("state", "idle")
        sentence = state_data.get("current_sentence")

        self._update_label(state, sentence)
        self._update_buttons(state)
        self._update_visibility(state)

    def _fetch_state(self) -> dict | None:
        """GET /state from the daemon. Returns parsed JSON or None on error."""
        try:
            self._ensure_client()
            assert self._client is not None
            resp = self._client.get("/state")
            return resp.json()
        except Exception:  # noqa: BLE001
            # Daemon down, socket missing, JSON error — all are expected.
            return None

    def _ensure_client(self) -> None:
        """Create the persistent httpx.Client if not already open."""
        if self._client is None or self._client.is_closed:
            self._client = httpx.Client(
                transport=httpx.HTTPTransport(uds=str(socket_path())),
                base_url="http://lexaloud",
                timeout=httpx.Timeout(0.5, connect=0.5),
            )

    def _close_client(self) -> None:
        """Cleanly close the httpx client."""
        if self._client is not None and not self._client.is_closed:
            try:
                self._client.close()
            except Exception:  # noqa: BLE001
                pass
            self._client = None

    def _elide(self, text: str, width: int) -> str:
        """Elide text to fit, matching the GTK label behaviour."""
        metrics = self._label.fontMetrics()
        return metrics.elidedText(text, QtCore.Qt.TextElideMode.ElideRight, width)

    def _update_label(self, state: str, sentence: str | None) -> None:
        """Update the sentence label text based on daemon state."""
        if state in ("speaking", "paused"):
            text = sentence if sentence else "Preparing\u2026"
            self._label.setText(self._elide(text, self._label.width() or BAR_WIDTH - 160))
        else:
            self._label.setText("")

    def _update_buttons(self, state: str) -> None:
        """Enable/disable buttons and toggle pause/resume label."""
        active = state in ("speaking", "paused")
        self._btn_pause.setEnabled(active)
        self._btn_skip.setEnabled(active)
        self._btn_stop.setEnabled(active)

        # Toggle the pause button label.
        if state == "paused":
            self._btn_pause.setText(LABEL_PLAY)
        else:
            self._btn_pause.setText(LABEL_PAUSE)

    def _update_visibility(self, state: str) -> None:
        """Show the overlay when speaking/paused, hide otherwise."""
        should_show = state in ("speaking", "paused")
        if should_show and not self._visible:
            self.show()
            self._visible = True
            # Re-position in case monitor layout changed.
            self._position_window()
        elif not should_show and self._visible:
            self.hide()
            self._visible = False
        self._last_state = state

    # --- button handlers --------------------------------------------------

    def _post_action(self, path: str) -> None:
        """POST to the daemon. Fire-and-forget; errors are silently ignored."""
        try:
            self._ensure_client()
            assert self._client is not None
            self._client.post(path)
        except Exception:  # noqa: BLE001
            log.debug("POST %s failed", path)

    def _on_pause_resume(self) -> None:
        self._post_action("/toggle")

    def _on_skip(self) -> None:
        self._post_action("/skip")

    def _on_stop(self) -> None:
        self._post_action("/stop")

    # --- cleanup ----------------------------------------------------------

    def closeEvent(self, event) -> None:  # noqa: N802 (Qt naming)
        self._close_client()
        super().closeEvent(event)


# --- single-instance lock ------------------------------------------------


def _acquire_single_instance_lock():
    """Take an exclusive flock on $XDG_RUNTIME_DIR/lexaloud-overlay.lock.

    Returns the open file descriptor (int) on success, None if another
    overlay is already running, or "skip" if we couldn't create the
    runtime dir.
    """
    import fcntl
    import os

    runtime = os.environ.get("XDG_RUNTIME_DIR") or f"/tmp/lexaloud-{os.getuid()}"
    try:
        os.makedirs(runtime, exist_ok=True)
    except OSError:
        return "skip"
    lock_path = os.path.join(runtime, "lexaloud-overlay.lock")
    try:
        fd = os.open(lock_path, os.O_RDWR | os.O_CREAT, 0o644)
    except OSError:
        return "skip"
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        os.close(fd)
        return None
    try:
        os.ftruncate(fd, 0)
        os.write(fd, f"{os.getpid()}\n".encode())
    except OSError:
        pass
    return fd


# --- entry point ----------------------------------------------------------


def main() -> int:
    """Entry point for ``lexaloud-overlay``."""
    lock = _acquire_single_instance_lock()
    if lock is None:
        print(
            "Lexaloud overlay is already running. Exiting.",
            file=sys.stderr,
        )
        return 0
    if isinstance(lock, int):
        globals()["_instance_lock_fd"] = lock

    app = QtWidgets.QApplication.instance() or QtWidgets.QApplication(sys.argv)
    app.setApplicationName("Lexaloud Overlay")
    overlay = OverlayWindow()  # noqa: F841 — prevent GC
    try:
        app.exec()
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

"""Keybinding backend abstraction and the capture dialog.

Supports GNOME (gsettings), XFCE (xfconf-query), KDE (read-only portal
display), and a NullBackend for unsupported desktops.

Only the gsettings/xfconf backends shell out to desktop tools; the key
capture dialog is Qt (PySide6), matching the rest of the GUI.
"""

from __future__ import annotations

import logging
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Protocol

from PySide6 import QtCore, QtGui, QtWidgets

log = logging.getLogger(__name__)

# (shortcut_id, human label, command tail to invoke lexaloud <tail>)
SHORTCUTS: list[tuple[str, str, str]] = [
    ("lexaloud", "Speak highlighted selection", "speak-selection"),
    ("lexaloud-toggle", "Pause / resume", "toggle"),
]


def _lexaloud_binary() -> str:
    """Resolve the absolute ``lexaloud`` binary path."""
    venv_bin = Path(sys.executable).parent
    return str(venv_bin / "lexaloud")


# --- backend protocol ---------------------------------------------------


class KeybindingBackend(Protocol):
    def get_binding(self, shortcut_id: str) -> str: ...
    def set_binding(self, shortcut_id: str, binding: str) -> bool: ...
    def is_available(self) -> bool: ...
    @property
    def frame_label(self) -> str: ...


# --- GNOME gsettings backend --------------------------------------------

KB_SCHEMA = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding"
KB_ARRAY_SCHEMA = "org.gnome.settings-daemon.plugins.media-keys"
KB_ARRAY_KEY = "custom-keybindings"
KB_BASE = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings"


def _gsettings_get(schema: str, key: str, path: str | None = None) -> str:
    schema_arg = f"{schema}:{path}" if path else schema
    try:
        r = subprocess.run(
            ["gsettings", "get", schema_arg, key],
            capture_output=True,
            text=True,
            timeout=2,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired, subprocess.SubprocessError) as e:
        log.debug("gsettings get %s %s failed: %s", schema_arg, key, e)
        return ""
    if r.returncode != 0:
        log.debug(
            "gsettings get %s %s exited %d: %s",
            schema_arg,
            key,
            r.returncode,
            (r.stderr or "").strip(),
        )
        return ""
    return r.stdout.strip().strip("'").strip('"')


def _gsettings_set(schema: str, key: str, value: str, path: str | None = None) -> bool:
    schema_arg = f"{schema}:{path}" if path else schema
    try:
        r = subprocess.run(
            ["gsettings", "set", schema_arg, key, value],
            check=False,
            capture_output=True,
            text=True,
            timeout=2,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired, subprocess.SubprocessError) as e:
        log.error("gsettings set %s %s failed: %s", schema_arg, key, e)
        return False
    if r.returncode != 0:
        log.error(
            "gsettings set %s %s exited %d: %s",
            schema_arg,
            key,
            r.returncode,
            (r.stderr or "").strip(),
        )
        return False
    return True


def _custom_keybindings_array() -> list[str]:
    raw = _gsettings_get(KB_ARRAY_SCHEMA, KB_ARRAY_KEY)
    if not raw or raw in ("@as []", "[]"):
        return []
    try:
        if raw.startswith("@as "):
            raw = raw[4:]
        inner = raw.strip("[]").strip()
        if not inner:
            return []
        parts = [p.strip().strip("'").strip('"') for p in inner.split(",")]
        return [p for p in parts if p]
    except Exception as e:  # noqa: BLE001
        log.warning("Could not parse custom-keybindings array %r: %s", raw, e)
        return []


def _ensure_keybinding_registered(path_suffix: str, label: str, command_tail: str) -> bool:
    path = f"{KB_BASE}/{path_suffix}/"
    current = _custom_keybindings_array()
    if path not in current:
        new_list = current + [path]
        gvariant = "[" + ", ".join(f"'{p}'" for p in new_list) + "]"
        if not _gsettings_set(KB_ARRAY_SCHEMA, KB_ARRAY_KEY, gvariant):
            return False
    command = f"{_lexaloud_binary()} {command_tail}"
    ok_name = _gsettings_set(KB_SCHEMA, "name", label, path)
    ok_cmd = _gsettings_set(KB_SCHEMA, "command", command, path)
    return ok_name and ok_cmd


class GnomeBackend:
    """GNOME custom-keybinding backend via gsettings."""

    def get_binding(self, shortcut_id: str) -> str:
        path = f"{KB_BASE}/{shortcut_id}/"
        raw = _gsettings_get(KB_SCHEMA, "binding", path)
        return _binding_to_human(raw)

    def set_binding(self, shortcut_id: str, binding: str) -> bool:
        path = f"{KB_BASE}/{shortcut_id}/"
        for suffix, label, tail in SHORTCUTS:
            if suffix == shortcut_id:
                if not _ensure_keybinding_registered(shortcut_id, label, tail):
                    return False
                break
        return _gsettings_set(KB_SCHEMA, "binding", binding, path)

    def is_available(self) -> bool:
        return shutil.which("gsettings") is not None

    @property
    def frame_label(self) -> str:
        return "Hotkeys (GNOME)"


# --- XFCE xfconf-query backend ------------------------------------------


class XfceBackend:
    """XFCE keybinding backend via xfconf-query."""

    _CHANNEL = "xfce4-keyboard-shortcuts"

    def _property_path(self, shortcut_id: str) -> str:
        for suffix, _label, tail in SHORTCUTS:
            if suffix == shortcut_id:
                return f"/commands/custom/lexaloud-{tail}"
        return f"/commands/custom/{shortcut_id}"

    def get_binding(self, shortcut_id: str) -> str:
        prop = self._property_path(shortcut_id)
        try:
            r = subprocess.run(
                ["xfconf-query", "-c", self._CHANNEL, "-p", prop],
                capture_output=True,
                text=True,
                timeout=2,
            )
            if r.returncode == 0 and r.stdout.strip():
                return r.stdout.strip()
        except (FileNotFoundError, subprocess.TimeoutExpired, subprocess.SubprocessError):
            pass
        return "(unset)"

    def set_binding(self, shortcut_id: str, binding: str) -> bool:
        for suffix, _label, tail in SHORTCUTS:
            if suffix == shortcut_id:
                command = f"{_lexaloud_binary()} {tail}"
                break
        else:
            return False
        # XFCE stores shortcuts as property → command mappings
        prop = f"/commands/custom/{binding}"
        try:
            r = subprocess.run(
                [
                    "xfconf-query",
                    "-c",
                    self._CHANNEL,
                    "-p",
                    prop,
                    "-n",
                    "-t",
                    "string",
                    "-s",
                    command,
                ],
                capture_output=True,
                text=True,
                timeout=2,
            )
            return r.returncode == 0
        except (FileNotFoundError, subprocess.TimeoutExpired, subprocess.SubprocessError) as e:
            log.error("xfconf-query set failed: %s", e)
            return False

    def is_available(self) -> bool:
        return shutil.which("xfconf-query") is not None

    @property
    def frame_label(self) -> str:
        return "Hotkeys (XFCE)"


# --- KDE native shortcut display ---------------------------------------


class PortalReadOnly:
    """Read-only display for KDE Plasma's native desktop actions."""

    def get_binding(self, shortcut_id: str) -> str:
        preferred = {
            "lexaloud": "Meta+R",
            "lexaloud-toggle": "Meta+P",
        }.get(shortcut_id)
        return preferred or "(see System Settings)"

    def set_binding(self, shortcut_id: str, binding: str) -> bool:
        return False  # read-only

    def is_available(self) -> bool:
        return False

    @property
    def frame_label(self) -> str:
        return "Hotkeys (KDE System Settings)"


# --- null backend (unsupported DEs) -------------------------------------


class NullBackend:
    """Fallback for desktops without integrated keybinding support."""

    def get_binding(self, shortcut_id: str) -> str:
        return "(manual setup)"

    def set_binding(self, shortcut_id: str, binding: str) -> bool:
        return False

    def is_available(self) -> bool:
        return False

    @property
    def frame_label(self) -> str:
        return "Hotkeys (manual setup)"


# --- backend selection ---------------------------------------------------


def detect_backend() -> KeybindingBackend:
    """Pick the right keybinding backend for the current desktop."""
    from ..platform import detect_desktop

    desktop = detect_desktop()
    if desktop.is_gnome:
        return GnomeBackend()
    if desktop.is_xfce:
        return XfceBackend()
    if desktop.is_kde:
        return PortalReadOnly()
    return NullBackend()


# --- shared helpers (used by CaptureDialog and control_window) ----------


_GNOME_MOD_NAMES = {
    "primary": "Ctrl",
    "ctrl": "Ctrl",
    "control": "Ctrl",
    "alt": "Alt",
    "shift": "Shift",
    "super": "Meta",
    "meta": "Meta",
    "hyper": "Meta",
}


def _binding_to_human(raw: str) -> str:
    """Convert a gsettings binding string to a friendly display string.

    gsettings uses GTK accelerator syntax (``<Primary><Alt>t``), while Qt
    parses ``Ctrl+Alt+R``; convert between the two notations.
    """
    if not raw:
        return "(unset)"
    try:
        mods: list[str] = []
        rest = raw
        while rest.startswith("<"):
            end = rest.index(">")
            mods.append(_GNOME_MOD_NAMES.get(rest[1:end].lower(), rest[1:end]))
            rest = rest[end + 1 :]
        if mods:
            key = rest.upper() if len(rest) == 1 else rest
            seq = QtGui.QKeySequence.fromString("+".join([*mods, key]))
        else:
            # Already Qt-style ("Meta+Shift+S") or a plain key name.
            seq = QtGui.QKeySequence.fromString(raw)
        if seq.isEmpty():
            return raw
        label = seq.toString(QtGui.QKeySequence.SequenceFormat.NativeText)
        return label if label else raw
    except Exception:  # noqa: BLE001
        return raw


_MODIFIER_NAMES = {
    QtCore.Qt.KeyboardModifier.ControlModifier: "<Primary>",
    QtCore.Qt.KeyboardModifier.AltModifier: "<Alt>",
    QtCore.Qt.KeyboardModifier.ShiftModifier: "<Shift>",
    QtCore.Qt.KeyboardModifier.MetaModifier: "<Super>",
}

_MODIFIER_ONLY_KEYS = {
    QtCore.Qt.Key.Key_Control,
    QtCore.Qt.Key.Key_Shift,
    QtCore.Qt.Key.Key_Alt,
    QtCore.Qt.Key.Key_Meta,
    QtCore.Qt.Key.Key_Super_L,
    QtCore.Qt.Key.Key_Super_R,
    QtCore.Qt.Key.Key_Hyper_L,
    QtCore.Qt.Key.Key_Hyper_R,
    QtCore.Qt.Key.Key_AltGr,
    QtCore.Qt.Key.Key_Menu,
}


def _event_to_binding(event: QtGui.QKeyEvent) -> str | None:
    """Turn a Qt key-press event into a gsettings binding string.

    gsettings uses GTK accelerator syntax (``<Primary><Alt>t``), so the
    result is rendered in that notation rather than Qt's own.
    """
    if event.key() in _MODIFIER_ONLY_KEYS or event.key() == QtCore.Qt.Key.Key_unknown:
        return None

    mods = event.modifiers() & QtCore.Qt.KeyboardModifier.KeyboardModifierMask
    parts = []
    for modifier, name in _MODIFIER_NAMES.items():
        if mods & modifier:
            parts.append(name)

    # Render the key itself in GTK accelerator notation: letters are
    # lowercase, special keys use their keysym-ish names ("Up", "F5").
    text = event.text()
    if text and text.isprintable() and not text.isspace():
        key_part = text.lower() if len(text) == 1 else text
    else:
        key_part = QtGui.QKeySequence(int(event.key())).toString()
        if not key_part:
            return None
        if len(key_part) == 1:
            key_part = key_part.lower()

    # GNOME ignores plain letters/digits without modifiers for global
    # shortcuts; require at least one modifier.
    if not parts:
        return None

    return "".join(parts) + key_part


# --- capture dialog (shared across backends that support set_binding) ---


class CaptureDialog(QtWidgets.QDialog):
    """Modal dialog that captures the next keypress as a new binding."""

    def __init__(
        self, parent: QtWidgets.QWidget, shortcut_id: str, backend: KeybindingBackend
    ) -> None:
        super().__init__(parent)
        self.setWindowTitle("Press a new shortcut")
        self.setModal(True)
        self.resize(360, 120)
        self.shortcut_id = shortcut_id
        self._backend = backend
        self.captured_binding: str | None = None
        self.write_ok: bool = False
        self._captured = False

        box = QtWidgets.QVBoxLayout(self)
        box.setContentsMargins(16, 16, 16, 16)
        box.setSpacing(8)
        msg = QtWidgets.QLabel(
            "Press the new key combination.\n(Esc to cancel, or just press Cancel.)"
        )
        box.addWidget(msg, 1)

        buttons = QtWidgets.QDialogButtonBox(QtWidgets.QDialogButtonBox.StandardButton.Cancel)
        buttons.rejected.connect(self.reject)
        box.addWidget(buttons)

    def keyPressEvent(self, event: QtGui.QKeyEvent) -> None:  # noqa: N802 (Qt naming)
        if self._captured:
            return
        if event.key() == QtCore.Qt.Key.Key_Escape:
            self.reject()
            return
        binding = _event_to_binding(event)
        if binding is None:
            return
        self._captured = True
        self.captured_binding = binding
        self.write_ok = self._backend.set_binding(self.shortcut_id, binding)
        self.accept()

"""Tests for the desktop app mode helpers.

Everything here runs without a QApplication: only the pure helper
functions (paths, autostart entries, daemon controller, keybinding
onboarding) are exercised.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path
from unittest.mock import patch

import pytest

pytest.importorskip("lexaloud.app", reason="PySide6 not available for this Python version")

from lexaloud.app import (  # noqa: E402
    DaemonController,
    autostart_enabled,
    autostart_path,
    binary_path,
    ensure_default_keybindings,
    is_onboarded,
    mark_onboarded,
    set_autostart,
)

# ---------- binary_path ----------


def test_binary_path_prefers_appimage(monkeypatch):
    monkeypatch.setattr(sys, "frozen", True, raising=False)
    monkeypatch.setenv("APPIMAGE", "/tmp/lexaloud-test.AppImage")
    assert binary_path() == Path("/tmp/lexaloud-test.AppImage")


def test_binary_path_ignores_foreign_appimage_env(monkeypatch):
    """A process that merely inherits an AppImage parent's env must not
    mistake the parent image for the lexaloud binary."""
    monkeypatch.setattr(sys, "frozen", False, raising=False)
    monkeypatch.setenv("APPIMAGE", "/tmp/parent.AppImage")
    with patch("shutil.which", return_value=None):
        result = binary_path()
    assert result != Path("/tmp/parent.AppImage")


def test_binary_path_falls_back_to_venv_script(monkeypatch, tmp_path):
    monkeypatch.delenv("APPIMAGE", raising=False)
    monkeypatch.setattr(sys, "frozen", False, raising=False)
    fake_bin = tmp_path / "bin" / "lexaloud"
    fake_bin.parent.mkdir(parents=True)
    fake_bin.touch()
    with patch("shutil.which", return_value=str(fake_bin)):
        assert binary_path() == fake_bin.resolve()


# ---------- autostart entry ----------


def test_autostart_write_and_remove(monkeypatch, tmp_path):
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path))
    monkeypatch.setattr(sys, "frozen", True, raising=False)
    monkeypatch.setenv("APPIMAGE", "/tmp/lexaloud-test.AppImage")

    assert autostart_enabled() is False
    assert set_autostart(True) is True

    entry = autostart_path()
    assert entry.exists()
    content = entry.read_text()
    assert content.startswith("[Desktop Entry]")
    assert "Exec=/tmp/lexaloud-test.AppImage" in content
    assert "Terminal=false" in content
    assert autostart_enabled() is True

    assert set_autostart(False) is True
    assert not entry.exists()
    assert autostart_enabled() is False


# ---------- onboarding marker ----------


def test_onboarding_marker_round_trip(monkeypatch, tmp_path):
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path))
    assert is_onboarded() is False
    mark_onboarded()
    assert is_onboarded() is True


# ---------- DaemonController ----------


class _FakePopen:
    def __init__(self, *args, **kwargs):
        self.terminated = False
        self.killed = False
        self.returncode = None

    def poll(self):
        return self.returncode

    def terminate(self):
        self.terminated = True
        self.returncode = 0

    def kill(self):
        self.killed = True
        self.returncode = -9

    def wait(self, timeout=None):
        return 0


def test_daemon_controller_start_and_stop(monkeypatch, tmp_path):
    started: list[list[str]] = []
    monkeypatch.setattr(sys, "frozen", True, raising=False)
    monkeypatch.setenv("APPIMAGE", str(tmp_path / "lexaloud.AppImage"))

    def fake_popen(args, **kwargs):
        started.append(args)
        return _FakePopen()

    monkeypatch.setattr(subprocess, "Popen", fake_popen)
    controller = DaemonController(binary=tmp_path / "lexaloud.AppImage")

    assert controller.running is False
    assert controller.start() is True
    assert started == [[str(tmp_path / "lexaloud.AppImage"), "daemon"]]
    assert controller.running is True

    controller.stop()
    assert controller.running is False
    # stop() is idempotent.
    controller.stop()


def test_daemon_controller_source_uses_same_interpreter(monkeypatch, tmp_path):
    started: list[list[str]] = []
    monkeypatch.delenv("APPIMAGE", raising=False)

    def fake_popen(args, **kwargs):
        started.append(args)
        return _FakePopen()

    monkeypatch.setattr(subprocess, "Popen", fake_popen)
    controller = DaemonController(binary=tmp_path / "lexaloud.AppImage")
    assert controller.start() is True
    assert started[0][:3] == [__import__("sys").executable, "-m", "lexaloud"]
    controller.stop()


def test_daemon_controller_adopts_running_daemon(monkeypatch, tmp_path):
    with patch("lexaloud.app.daemon_alive", return_value=True):
        with patch("subprocess.Popen", side_effect=AssertionError("must not spawn")):
            controller = DaemonController(binary=tmp_path / "lexaloud.AppImage")
            assert controller.start() is True
            assert controller.running is False


# ---------- default keybinding creation ----------


class _FakeBackend:
    def __init__(self, available=True, bindings=None):
        self._available = available
        self._bindings = bindings or {}
        self.set_calls: list[tuple[str, str]] = []

    def get_binding(self, shortcut_id):
        return self._bindings.get(shortcut_id, "(unset)")

    def set_binding(self, shortcut_id, binding):
        self.set_calls.append((shortcut_id, binding))
        self._bindings[shortcut_id] = binding
        return True

    def is_available(self):
        return self._available


def test_ensure_default_keybindings_sets_unset_bindings(monkeypatch):
    backend = _FakeBackend()
    with patch("lexaloud.gui_control.keybindings.detect_backend", return_value=backend):
        created = ensure_default_keybindings()
    assert len(created) == 2
    assert ("lexaloud", "<Super>r") in backend.set_calls
    assert ("lexaloud-toggle", "<Super>p") in backend.set_calls


def test_ensure_default_keybindings_respects_existing(monkeypatch):
    backend = _FakeBackend(bindings={"lexaloud": "Ctrl+Alt+R", "lexaloud-toggle": "(unset)"})
    with patch("lexaloud.gui_control.keybindings.detect_backend", return_value=backend):
        ensure_default_keybindings()
    assert backend.set_calls == [("lexaloud-toggle", "<Super>p")]


def test_ensure_default_keybindings_skips_unavailable_backend(monkeypatch):
    backend = _FakeBackend(available=False)
    with patch("lexaloud.gui_control.keybindings.detect_backend", return_value=backend):
        created = ensure_default_keybindings()
    assert created == []
    assert backend.set_calls == []

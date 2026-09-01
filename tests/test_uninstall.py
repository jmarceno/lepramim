"""Tests for safe removal of Lexaloud's user-level integration."""

from __future__ import annotations

import subprocess
from pathlib import Path
from unittest.mock import patch

from lexaloud.uninstall import run_uninstall


def test_uninstall_stops_service_removes_unit_and_launcher(tmp_path: Path, monkeypatch):
    config_home = tmp_path / "config"
    data_home = tmp_path / "data"
    unit = config_home / "systemd" / "user" / "lexaloud.service"
    launcher = data_home / "applications" / "lexaloud.desktop"
    unit.parent.mkdir(parents=True)
    launcher.parent.mkdir(parents=True)
    unit.write_text("[Service]\n")
    launcher.write_text("[Desktop Entry]\n")
    monkeypatch.setenv("XDG_CONFIG_HOME", str(config_home))
    monkeypatch.setenv("XDG_DATA_HOME", str(data_home))

    calls: list[list[str]] = []

    def fake_run(command, **kwargs):
        calls.append(command)
        return subprocess.CompletedProcess(command, 0, stdout="", stderr="")

    with patch("lexaloud.uninstall.shutil.which", return_value="/usr/bin/systemctl"):
        with patch("lexaloud.uninstall.subprocess.run", side_effect=fake_run):
            assert run_uninstall() == 0

    assert not unit.exists()
    assert not launcher.exists()
    assert calls == [
        ["systemctl", "--user", "disable", "--now", "lexaloud.service"],
        ["systemctl", "--user", "daemon-reload"],
    ]


def test_uninstall_keeps_unit_when_systemd_cannot_stop_it(tmp_path: Path, monkeypatch, capsys):
    config_home = tmp_path / "config"
    unit = config_home / "systemd" / "user" / "lexaloud.service"
    unit.parent.mkdir(parents=True)
    unit.write_text("[Service]\n")
    monkeypatch.setenv("XDG_CONFIG_HOME", str(config_home))

    def fake_run(command, **kwargs):
        return subprocess.CompletedProcess(
            command,
            1,
            stdout="",
            stderr="Failed to connect to bus: No medium found",
        )

    with patch("lexaloud.uninstall.shutil.which", return_value="/usr/bin/systemctl"):
        with patch("lexaloud.uninstall.subprocess.run", side_effect=fake_run):
            assert run_uninstall() == 1

    assert unit.exists()
    assert "unit was kept" in capsys.readouterr().err

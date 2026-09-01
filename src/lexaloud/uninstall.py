"""Remove Lexaloud's user-level desktop integration.

The command deliberately removes only artifacts that Lexaloud created for
desktop integration: the user systemd unit and the optional per-user desktop
launcher. Configuration and downloaded model files are retained so an
uninstall/reinstall does not silently discard user settings or a 340 MB
download.
"""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

from .cli import EXIT_GENERIC_ERROR, EXIT_OK

SERVICE_NAME = "lexaloud.service"


def _xdg_dir(variable: str, fallback_name: str) -> Path:
    value = os.environ.get(variable)
    return Path(value) if value else Path.home() / fallback_name


def _unit_path() -> Path:
    return _xdg_dir("XDG_CONFIG_HOME", ".config") / "systemd" / "user" / SERVICE_NAME


def _desktop_path() -> Path:
    return _xdg_dir("XDG_DATA_HOME", ".local/share") / "applications" / "lexaloud.desktop"


def _systemctl_user(*arguments: str) -> tuple[int, str]:
    """Run a user-manager systemctl command and return its status/detail."""
    try:
        result = subprocess.run(
            ["systemctl", "--user", *arguments],
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        return 127, str(exc)

    detail = "\n".join(part for part in (result.stdout.strip(), result.stderr.strip()) if part)
    return result.returncode, detail


def _is_absent_unit(detail: str) -> bool:
    normalized = detail.casefold()
    return any(
        marker in normalized
        for marker in (
            "not loaded",
            "not found",
            "does not exist",
            "not enabled",
        )
    )


def _remove_file(path: Path, label: str) -> bool:
    if not path.exists() and not path.is_symlink():
        print(f"{label}: not present ({path})")
        return False
    path.unlink()
    print(f"Removed {label}: {path}")
    return True


def run_uninstall() -> int:
    """Stop the daemon and remove its user-level integration artifacts."""
    unit_path = _unit_path()
    desktop_path = _desktop_path()
    changed = False

    if unit_path.exists() or unit_path.is_symlink():
        if shutil.which("systemctl") is None:
            print(
                "Cannot safely remove the systemd unit: `systemctl` is not on PATH. "
                "Stop the service manually first, then remove:",
                file=os.sys.stderr,
            )
            print(f"  {unit_path}", file=os.sys.stderr)
            return EXIT_GENERIC_ERROR

        rc, detail = _systemctl_user("disable", "--now", SERVICE_NAME)
        if rc != 0 and not _is_absent_unit(detail):
            print(
                "Could not stop/disable the Lexaloud user service. The unit was kept in place.",
                file=os.sys.stderr,
            )
            if detail:
                print(detail, file=os.sys.stderr)
            return EXIT_GENERIC_ERROR

        changed |= _remove_file(unit_path, "systemd unit")
        reload_rc, reload_detail = _systemctl_user("daemon-reload")
        if reload_rc != 0:
            print("Warning: systemd daemon-reload failed.", file=os.sys.stderr)
            if reload_detail:
                print(reload_detail, file=os.sys.stderr)
            return EXIT_GENERIC_ERROR
        print("systemd user manager reloaded.")
    else:
        print(f"systemd unit: not present ({unit_path})")

    if _remove_file(desktop_path, "desktop launcher"):
        changed = True

    if changed:
        print("Lexaloud desktop integration removed.")
    else:
        print("No Lexaloud desktop integration needed removal.")
    print("Configuration and downloaded models were kept.")
    print("Delete the AppImage or source checkout separately if desired.")
    return EXIT_OK

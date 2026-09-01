"""PyInstaller specification for the CPU AppImage.

The result is an onedir bundle.  The directory is placed inside the AppDir
and then compressed as an AppImage; using onedir avoids a second extraction
layer at every startup while retaining a complete Python runtime.
"""

# PyInstaller injects SPECPATH, Analysis, PYZ, EXE, and COLLECT when it
# evaluates a spec; they are intentionally not normal Python globals.
# ruff: noqa: F821

import importlib.util
from pathlib import Path

from PyInstaller.utils.hooks import collect_all, collect_data_files, collect_submodules

PROJECT_ROOT = Path(SPECPATH).resolve().parent
SOURCE_ROOT = PROJECT_ROOT / "src"

datas: list[tuple[str, str]] = []
binaries: list[tuple[str, str]] = []
hiddenimports: list[str] = [
    "lexaloud.daemon",
    "lexaloud.models",
    "lexaloud.audio",
    "lexaloud.player",
    "lexaloud.setup",
    "lexaloud.uninstall",
    "lexaloud.bug_report",
    "lexaloud.mpris",
    "lexaloud.shortcuts",
    "lexaloud.preprocessor.llm_normalize",
    "lexaloud.preprocessor.sre_latex",
]

for package in ("kokoro_onnx", "onnxruntime", "numpy", "espeakng_loader"):
    package_datas, package_binaries, package_hiddenimports = collect_all(package)
    datas.extend(package_datas)
    binaries.extend(package_binaries)
    hiddenimports.extend(package_hiddenimports)

# The setup command renders the systemd unit from package data rather than a
# Python string.  PyInstaller does not include setuptools package data unless
# it is collected explicitly.
datas.extend(collect_data_files("lexaloud", includes=["templates/*.template", "templates/*.toml"]))

# language_tags discovers its JSON catalogue at runtime.  Its hook reports
# these files, but PyInstaller's final Analysis can discard that report when
# several third-party packages contribute overlapping data.  Add the files
# explicitly so the frozen Kokoro provider can normalize language tags.
language_tags_spec = importlib.util.find_spec("language_tags")
if language_tags_spec is None or language_tags_spec.origin is None:
    raise RuntimeError("language_tags must be installed to build the AppImage")
language_tags_root = Path(language_tags_spec.origin).parent
language_tags_json = language_tags_root / "data" / "json"
datas.extend(
    (str(path), "language_tags/data/json") for path in sorted(language_tags_json.glob("*.json"))
)
if not any(destination == "language_tags/data/json" for _, destination in datas):
    raise RuntimeError(f"language_tags JSON catalogue not found under {language_tags_json}")

# ALSA and JACK are host audio-session interfaces, not application runtime
# libraries.  Bundling them overrides the user's PipeWire/PulseAudio setup
# and can make PortAudio reject otherwise valid sample rates.  Keep
# libportaudio in the image, but resolve these two session libraries from the
# host at runtime like Wayland and X11 themselves.
binaries = [
    entry for entry in binaries if Path(entry[0]).name not in {"libasound.so.2", "libjack.so.0"}
]

hiddenimports.extend(collect_submodules("lexaloud.preprocessor"))
hiddenimports.extend(collect_submodules("lexaloud.providers"))

a = Analysis(
    [str(PROJECT_ROOT / "packaging" / "lexaloud_entry.py")],
    pathex=[str(SOURCE_ROOT)],
    binaries=binaries,
    datas=datas,
    hiddenimports=hiddenimports,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=["gi", "gi.repository"],
    noarchive=False,
)

# The sounddevice PyInstaller hook can add these libraries after the explicit
# package collection above.  Filter the completed analysis as the final
# guard, otherwise they would still land beside the frozen executable.
a.binaries = [
    entry for entry in a.binaries if Path(entry[0]).name not in {"libasound.so.2", "libjack.so.0"}
]

pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name="lexaloud",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    console=True,
)

coll = COLLECT(
    exe,
    a.binaries,
    a.datas,
    strip=False,
    upx=False,
    name="lexaloud",
)

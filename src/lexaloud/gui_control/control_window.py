"""Qt 6 control window for Lexaloud.

Lets the user change the default Kokoro voice, playback speed, and the
desktop keyboard shortcuts.
"""

from __future__ import annotations

import logging
import subprocess

from PySide6 import QtCore, QtWidgets

from .config_io import _load_config_dict, _save_config_dict
from .keybindings import (
    SHORTCUTS,
    CaptureDialog,
    detect_backend,
)
from .voices import KOKORO_VOICES, LANGUAGES

log = logging.getLogger(__name__)


class ControlWindow(QtWidgets.QWidget):
    def __init__(self) -> None:
        super().__init__()
        self.setWindowTitle("Lexaloud — Control")
        self.setMinimumSize(520, 480)

        self._kb_backend = detect_backend()

        outer = QtWidgets.QVBoxLayout(self)
        outer.setContentsMargins(16, 16, 16, 16)
        outer.setSpacing(12)

        # Voice section
        voice_frame = QtWidgets.QGroupBox("Voice")
        voice_box = QtWidgets.QVBoxLayout(voice_frame)
        voice_box.setContentsMargins(12, 8, 12, 12)
        voice_box.setSpacing(8)

        self.voice_combo = QtWidgets.QComboBox()
        for _value, label in KOKORO_VOICES:
            self.voice_combo.addItem(label, _value)
        voice_box.addWidget(self.voice_combo)

        voice_box.addWidget(QtWidgets.QLabel("Language"))
        self.lang_combo = QtWidgets.QComboBox()
        for _value, label in LANGUAGES:
            self.lang_combo.addItem(label, _value)
        voice_box.addWidget(self.lang_combo)

        # Speed slider
        speed_row = QtWidgets.QHBoxLayout()
        voice_box.addLayout(speed_row)
        speed_row.addWidget(QtWidgets.QLabel("Speed"))

        self.speed_slider = QtWidgets.QSlider(QtCore.Qt.Orientation.Horizontal)
        self.speed_slider.setRange(50, 200)  # 0.5× – 2.0× in 0.05 steps
        self.speed_slider.setSingleStep(5)
        self.speed_slider.setPageStep(10)
        self.speed_slider.setTickPosition(QtWidgets.QSlider.TickPosition.TicksBelow)
        self.speed_slider.setTickInterval(25)
        speed_row.addWidget(self.speed_slider, 1)

        self.speed_value = QtWidgets.QLabel("1.00×")
        speed_row.addWidget(self.speed_value)

        self.speed_hint = QtWidgets.QLabel("")
        self.speed_hint.setStyleSheet("color: gray;")
        voice_box.addWidget(self.speed_hint)
        self.speed_slider.valueChanged.connect(self._on_speed_changed)

        outer.addWidget(voice_frame)

        # Hotkeys section
        keys_frame = QtWidgets.QGroupBox(self._kb_backend.frame_label)
        keys_grid = QtWidgets.QGridLayout(keys_frame)
        keys_grid.setContentsMargins(12, 12, 12, 12)
        keys_grid.setHorizontalSpacing(12)
        keys_grid.setVerticalSpacing(8)

        self.hotkey_labels: dict[str, QtWidgets.QLabel] = {}
        can_change = self._kb_backend.is_available()
        for row, (shortcut_id, label, _cmd) in enumerate(SHORTCUTS):
            name_lbl = QtWidgets.QLabel(f"{label}:")
            current_lbl = QtWidgets.QLabel(self._kb_backend.get_binding(shortcut_id))
            self.hotkey_labels[shortcut_id] = current_lbl
            change_btn = QtWidgets.QPushButton("Change…")
            change_btn.setEnabled(can_change)
            if not can_change:
                change_btn.setToolTip("Keybindings are managed by the desktop environment.")
            change_btn.clicked.connect(
                lambda _=False, sid=shortcut_id: self._on_change_binding(sid)
            )
            keys_grid.addWidget(name_lbl, row, 0)
            keys_grid.addWidget(current_lbl, row, 1)
            keys_grid.addWidget(change_btn, row, 2)
        keys_grid.setColumnStretch(1, 1)

        outer.addWidget(keys_frame)

        # Advanced section
        advanced_frame = QtWidgets.QGroupBox("Advanced")
        advanced_box = QtWidgets.QVBoxLayout(advanced_frame)
        advanced_box.setContentsMargins(12, 12, 12, 12)
        advanced_box.setSpacing(8)

        self.overlay_toggle = QtWidgets.QCheckBox("Show floating overlay when speaking")
        self.overlay_toggle.setToolTip(
            "Displays a small translucent bar at the bottom of the screen "
            "showing the current sentence with pause/skip/stop buttons."
        )
        advanced_box.addWidget(self.overlay_toggle)

        self.llm_toggle = QtWidgets.QCheckBox("Enable LLM text normalization")
        self.llm_toggle.setToolTip(
            "Use a local LLM to normalize complex text (acronyms, equations, "
            "tables) before speech synthesis. Requires ~1.2 GB additional VRAM.\n"
            "Install with: pip install lexaloud[llm]\n"
            "Download model: lexaloud download-models --llm"
        )
        advanced_box.addWidget(self.llm_toggle)

        outer.addWidget(advanced_frame)

        # Buttons
        button_box = QtWidgets.QHBoxLayout()
        outer.addLayout(button_box)

        self.status_label = QtWidgets.QLabel("")
        self.status_label.setStyleSheet("color: gray;")
        outer.addWidget(self.status_label)

        button_box.addStretch(1)
        apply_btn = QtWidgets.QPushButton("Apply & restart daemon")
        apply_btn.clicked.connect(self._on_apply_voice)
        button_box.addWidget(apply_btn)

        close_btn = QtWidgets.QPushButton("Close")
        close_btn.clicked.connect(self.close)
        button_box.addWidget(close_btn)

        self._load_current_config()

    # ---------- load current values ----------

    def _load_current_config(self) -> None:
        cfg = _load_config_dict()
        provider = cfg.get("provider", {}) if isinstance(cfg, dict) else {}
        current_voice = provider.get("voice", "af_heart")
        current_lang = provider.get("lang", "en-us")
        current_speed = float(provider.get("speed", 1.0))

        index = self.voice_combo.findData(current_voice)
        if index >= 0:
            self.voice_combo.setCurrentIndex(index)
        else:
            self.status_label.setText(
                f"Note: current voice '{current_voice}' is outside the curated list; "
                "edit ~/.config/lexaloud/config.toml directly to keep it."
            )

        lang_index = self.lang_combo.findData(current_lang)
        self.lang_combo.setCurrentIndex(lang_index if lang_index >= 0 else 0)

        clamped = max(0.5, min(2.0, current_speed))
        self.speed_slider.setValue(round(clamped * 100))
        self._on_speed_changed(self.speed_slider.value())

    def _on_speed_changed(self, value: int) -> None:
        v = value / 100.0
        self.speed_value.setText(f"{v:.2f}×")
        if 0.85 <= v <= 1.3:
            self.speed_hint.setText(f"{v:.2f}× — safe range for dense reading.")
        elif v < 0.85:
            self.speed_hint.setText(f"{v:.2f}× — slower than natural; may feel dragged.")
        elif v <= 1.5:
            self.speed_hint.setText(
                f"{v:.2f}× — fine for familiar material, may strain comprehension on new dense text."
            )
        else:
            self.speed_hint.setText(
                f"{v:.2f}× — risky for unfamiliar academic material; comprehension drops."
            )

    # ---------- handlers ----------

    def _selected_voice(self) -> str | None:
        return self.voice_combo.currentData()

    def _selected_lang(self) -> str | None:
        return self.lang_combo.currentData()

    def _on_apply_voice(self) -> None:
        voice = self._selected_voice()
        lang = self._selected_lang()
        speed = round(self.speed_slider.value() / 100.0, 2)
        if voice is None or lang is None:
            self.status_label.setText("Pick a voice and a language first.")
            return

        cfg = _load_config_dict()
        provider = cfg.setdefault("provider", {})
        provider["voice"] = voice
        provider["lang"] = lang
        provider["speed"] = speed
        advanced = cfg.setdefault("advanced", {})
        advanced["overlay"] = self.overlay_toggle.isChecked()
        normalizer = cfg.setdefault("normalizer", {})
        normalizer["enabled"] = self.llm_toggle.isChecked()
        try:
            _save_config_dict(cfg)
        except Exception as e:  # noqa: BLE001
            self.status_label.setText(f"Saving config failed: {e}")
            return

        summary = f"voice={voice}, lang={lang}, speed={speed:.2f}×"
        try:
            r = subprocess.run(
                ["systemctl", "--user", "is-active", "lexaloud.service"],
                capture_output=True,
                text=True,
                timeout=2,
            )
            if r.stdout.strip() == "active":
                subprocess.run(
                    ["systemctl", "--user", "restart", "lexaloud.service"],
                    capture_output=True,
                    timeout=10,
                )
                self.status_label.setText(f"Saved {summary}; daemon restarted.")
            else:
                self.status_label.setText(
                    f"Saved {summary}. Daemon is stopped; "
                    "it will use the new settings on the next start."
                )
        except Exception as e:  # noqa: BLE001
            self.status_label.setText(f"Saved {summary}; couldn't restart daemon: {e}")

    def _on_change_binding(self, shortcut_id: str) -> None:
        dialog = CaptureDialog(self, shortcut_id, self._kb_backend)
        result = dialog.exec()
        captured = dialog.captured_binding
        write_ok = dialog.write_ok
        dialog.deleteLater()
        self.hotkey_labels[shortcut_id].setText(self._kb_backend.get_binding(shortcut_id))
        if result == QtWidgets.QDialog.DialogCode.Accepted and captured:
            if not write_ok:
                self.status_label.setText(
                    "Failed to write hotkey binding. Check `journalctl --user` for details."
                )
                return
            # Verify the write stuck
            actual = self._kb_backend.get_binding(shortcut_id)
            human = captured  # the raw binding string
            if actual != "(unset)" and human not in actual:
                self.status_label.setText(
                    f"Hotkey write may not have stuck: binding now shows {actual!r}."
                )


def main() -> int:
    """Standalone entry: open the control window without the tray."""
    import sys

    app = QtWidgets.QApplication.instance() or QtWidgets.QApplication(sys.argv)
    app.setApplicationName("Lexaloud")
    win = ControlWindow()
    win.show()
    try:
        app.exec()
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

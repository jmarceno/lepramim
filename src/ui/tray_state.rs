//! Tray state machine (ported from Qt `tray.cpp`).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayActionState {
    pub speak_enabled: bool,
    pub pause_enabled: bool,
    pub stop_enabled: bool,
    pub is_warming: bool,
    pub is_active: bool,
    pub toggle_label: String,
    pub tooltip: String,
    pub icon_running: bool,
    pub cpu_fallback: bool,
}

pub fn tray_state_for_daemon(state_str: &str, active: bool, cpu_fallback: bool) -> TrayActionState {
    let is_warming = state_str == "warming";
    let (icon_running, tooltip, toggle_label) = if is_warming {
        (
            true,
            "Lexaloud: warming up".to_string(),
            "Stop daemon (warming\u{2026})".to_string(),
        )
    } else if active {
        (
            true,
            "Lexaloud: running".to_string(),
            "Stop daemon".to_string(),
        )
    } else {
        (
            false,
            "Lexaloud: stopped".to_string(),
            "Start daemon".to_string(),
        )
    };
    let ready = active && !is_warming;
    TrayActionState {
        speak_enabled: ready,
        pause_enabled: ready,
        stop_enabled: ready,
        is_warming,
        is_active: active,
        toggle_label,
        tooltip,
        icon_running,
        cpu_fallback: active && cpu_fallback,
    }
}

pub const MENU_SHORTCUT: &str = "Shortcut: Meta+R";
pub const MENU_CPU_FALLBACK: &str = "Running on CPU, CUDA not available.";
pub const MENU_SPEAK: &str = "Speak highlighted selection";
pub const MENU_PAUSE: &str = "Pause / resume";
pub const MENU_STOP: &str = "Stop current playback";
pub const MENU_CONTROL: &str = "Control window\u{2026}";
pub const MENU_AUTOSTART: &str = "Start with desktop";
pub const MENU_QUIT: &str = "Quit Lexaloud";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle() {
        let s = tray_state_for_daemon("idle", false, false);
        assert!(!s.is_active);
        assert!(!s.is_warming);
        assert!(!s.speak_enabled);
        assert!(!s.pause_enabled);
        assert!(!s.stop_enabled);
        assert!(!s.icon_running);
        assert_eq!(s.tooltip, "Lexaloud: stopped");
        assert_eq!(s.toggle_label, "Start daemon");
    }

    #[test]
    fn warming() {
        let s = tray_state_for_daemon("warming", true, false);
        assert!(s.is_active);
        assert!(s.is_warming);
        assert!(!s.speak_enabled);
        assert!(s.toggle_label.contains("warming"));
    }

    #[test]
    fn speaking() {
        let s = tray_state_for_daemon("speaking", true, false);
        assert!(s.speak_enabled);
        assert!(s.pause_enabled);
        assert!(s.stop_enabled);
        assert_eq!(s.toggle_label, "Stop daemon");
    }

    #[test]
    fn paused() {
        let s = tray_state_for_daemon("paused", true, false);
        assert!(s.speak_enabled);
        assert!(s.pause_enabled);
        assert!(s.stop_enabled);
    }

    #[test]
    fn unknown_active() {
        let s = tray_state_for_daemon("unknown", true, false);
        assert!(s.is_active);
        assert!(s.speak_enabled);
    }

    #[test]
    fn cpu_fallback_when_active() {
        let s = tray_state_for_daemon("idle", true, true);
        assert!(s.cpu_fallback);
        let hidden = tray_state_for_daemon("idle", false, true);
        assert!(!hidden.cpu_fallback);
    }
}

//! Tray state machine.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayIconPhase {
    Idle,
    Preparing,
    Speaking,
}

/// Resolve tray icon phase from daemon playback state and local preparing flag.
///
/// `speaking` without a current sentence is still synthesis / queueing, not
/// audible playback — keep the preparing (breath) look until a sentence starts.
pub fn tray_icon_phase(
    state_str: &str,
    preparing_speech: bool,
    current_sentence: &str,
) -> TrayIconPhase {
    match state_str {
        "paused" => TrayIconPhase::Speaking,
        "speaking" if !current_sentence.is_empty() => TrayIconPhase::Speaking,
        "speaking" => TrayIconPhase::Preparing,
        _ if preparing_speech => TrayIconPhase::Preparing,
        _ => TrayIconPhase::Idle,
    }
}

/// Map phase (+ breath animation mix) to blue↔green interpolation.
pub fn tray_icon_mix(phase: TrayIconPhase, breath_mix: f32) -> f32 {
    match phase {
        TrayIconPhase::Idle => 0.0,
        TrayIconPhase::Preparing => breath_mix.clamp(0.0, 1.0),
        TrayIconPhase::Speaking => 1.0,
    }
}

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
            "Lepramim: warming up".to_string(),
            "Stop daemon (warming\u{2026})".to_string(),
        )
    } else if active {
        (
            true,
            "Lepramim: running".to_string(),
            "Stop daemon".to_string(),
        )
    } else {
        (
            false,
            "Lepramim: stopped".to_string(),
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
pub const MENU_WAYLAND_COPY_FIRST: &str =
    "Wayland: copy first (Ctrl+C), then Meta+R — no key injector found.";
pub const MENU_CPU_FALLBACK: &str = "Running on CPU, CUDA not available.";
pub const MENU_SPEAK: &str = "Speak highlighted selection";
pub const MENU_PAUSE: &str = "Pause / resume";
pub const MENU_STOP: &str = "Stop current playback";
pub const MENU_CONTROL: &str = "Control window\u{2026}";
pub const MENU_AUTOSTART: &str = "Start with desktop";
pub const MENU_QUIT: &str = "Quit Lepramim";

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
        assert_eq!(s.tooltip, "Lepramim: stopped");
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

    #[test]
    fn icon_phase_idle_while_preparing_false() {
        assert_eq!(tray_icon_phase("idle", false, ""), TrayIconPhase::Idle);
    }

    #[test]
    fn icon_phase_preparing_while_idle_and_flag_set() {
        assert_eq!(tray_icon_phase("idle", true, ""), TrayIconPhase::Preparing);
    }

    #[test]
    fn icon_phase_speaking_without_sentence_stays_preparing() {
        assert_eq!(
            tray_icon_phase("speaking", true, ""),
            TrayIconPhase::Preparing
        );
        assert_eq!(
            tray_icon_phase("speaking", false, ""),
            TrayIconPhase::Preparing
        );
    }

    #[test]
    fn icon_phase_speaking_with_sentence_is_solid_green() {
        assert_eq!(
            tray_icon_phase("speaking", true, "Hello."),
            TrayIconPhase::Speaking
        );
    }

    #[test]
    fn icon_phase_paused_is_speaking() {
        assert_eq!(
            tray_icon_phase("paused", false, ""),
            TrayIconPhase::Speaking
        );
    }

    #[test]
    fn icon_mix_by_phase() {
        assert_eq!(tray_icon_mix(TrayIconPhase::Idle, 0.8), 0.0);
        assert_eq!(tray_icon_mix(TrayIconPhase::Speaking, 0.2), 1.0);
        assert!((tray_icon_mix(TrayIconPhase::Preparing, 0.42) - 0.42).abs() < f32::EPSILON);
    }
}

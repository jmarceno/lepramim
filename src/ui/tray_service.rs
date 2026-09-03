//! StatusNotifier tray via ksni (no GTK). The app must not start without a tray.

use crossbeam_channel::{Receiver, Sender, unbounded};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use ksni::menu::{CheckmarkItem, MenuItem, StandardItem};
use ksni::{Handle, Tray, TrayService};

use super::tray_state::{
    MENU_AUTOSTART, MENU_CONTROL, MENU_CPU_FALLBACK, MENU_PAUSE, MENU_QUIT, MENU_SHORTCUT,
    MENU_SPEAK, MENU_STOP, TrayIconPhase, tray_icon_mix,
};
use crate::ui::icon;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    ToggleDaemon,
    SpeakSelection,
    PauseResume,
    StopPlayback,
    OpenControl,
    ToggleAutostart,
    Quit,
}

#[derive(Debug, Clone)]
pub struct TraySharedState {
    pub icon_running: bool,
    pub icon_phase: TrayIconPhase,
    /// Written by the breath thread while `icon_phase == Preparing`.
    pub breath_mix: f32,
    pub tooltip: String,
    pub toggle_label: String,
    pub speak_enabled: bool,
    pub pause_enabled: bool,
    pub stop_enabled: bool,
    pub autostart_checked: bool,
    /// When the daemon is on CPU because CUDA is missing.
    pub cpu_fallback: bool,
}

impl Default for TraySharedState {
    fn default() -> Self {
        Self {
            icon_running: false,
            icon_phase: TrayIconPhase::Idle,
            breath_mix: 0.0,
            tooltip: "Lepramim: stopped".into(),
            toggle_label: "Start daemon".into(),
            speak_enabled: false,
            pause_enabled: false,
            stop_enabled: false,
            autostart_checked: crate::platform::service::autostart_path().is_file(),
            cpu_fallback: false,
        }
    }
}

struct LepramimTray {
    tx: Sender<TrayEvent>,
    state: TraySharedState,
}

impl LepramimTray {
    fn send(&self, ev: TrayEvent) {
        let _ = self.tx.send(ev);
    }

    fn icon_mix(&self) -> f32 {
        if !self.state.icon_running {
            return 0.0;
        }
        tray_icon_mix(self.state.icon_phase, self.state.breath_mix)
    }
}

impl Tray for LepramimTray {
    fn id(&self) -> String {
        "lepramim".into()
    }

    fn title(&self) -> String {
        "Lepramim".into()
    }

    fn status(&self) -> ksni::Status {
        ksni::Status::Active
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::ApplicationStatus
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        icon::render_tray_icon_argb32_with_mix(self.state.icon_running, self.icon_mix())
            .into_iter()
            .collect()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: self.state.tooltip.clone(),
            description: String::new(),
            icon_name: String::new(),
            icon_pixmap: self.icon_pixmap(),
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut items: Vec<MenuItem<Self>> = vec![
            StandardItem {
                label: MENU_SHORTCUT.into(),
                enabled: false,
                ..Default::default()
            }
            .into(),
        ];
        if self.state.cpu_fallback {
            items.push(
                StandardItem {
                    label: MENU_CPU_FALLBACK.into(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
        }
        items.extend([
            MenuItem::Separator,
            StandardItem {
                label: self.state.toggle_label.clone(),
                activate: Box::new(move |t: &mut Self| t.send(TrayEvent::ToggleDaemon)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: MENU_SPEAK.into(),
                enabled: self.state.speak_enabled,
                activate: Box::new(move |t: &mut Self| t.send(TrayEvent::SpeakSelection)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: MENU_PAUSE.into(),
                enabled: self.state.pause_enabled,
                activate: Box::new(move |t: &mut Self| t.send(TrayEvent::PauseResume)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: MENU_STOP.into(),
                enabled: self.state.stop_enabled,
                activate: Box::new(move |t: &mut Self| t.send(TrayEvent::StopPlayback)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: MENU_CONTROL.into(),
                activate: Box::new(move |t: &mut Self| t.send(TrayEvent::OpenControl)),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: MENU_AUTOSTART.into(),
                checked: self.state.autostart_checked,
                activate: Box::new(move |t: &mut Self| t.send(TrayEvent::ToggleAutostart)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: MENU_QUIT.into(),
                activate: Box::new(move |t: &mut Self| t.send(TrayEvent::Quit)),
                ..Default::default()
            }
            .into(),
        ]);
        items
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayEvent::OpenControl);
    }

    fn watcher_offine(&self) -> bool {
        false
    }
}

pub struct TrayHandle {
    pub rx: Receiver<TrayEvent>,
    handle: Handle<LepramimTray>,
    breath_stop: Arc<AtomicBool>,
}

impl Drop for TrayHandle {
    fn drop(&mut self) {
        self.breath_stop.store(true, Ordering::Relaxed);
        self.handle.shutdown();
    }
}

impl TrayHandle {
    pub fn start() -> Result<Self, String> {
        if !status_notifier_watcher_present() {
            return Err(
                "no StatusNotifierWatcher on the session bus (system tray host missing)".into(),
            );
        }

        let (tx, rx) = unbounded();
        let service = TrayService::new(LepramimTray {
            tx,
            state: TraySharedState::default(),
        });
        let handle = service.handle();
        let (err_tx, err_rx) = std::sync::mpsc::channel();
        thread::Builder::new()
            .name("lepramim-tray".into())
            .spawn(move || {
                if let Err(e) = service.run() {
                    let _ = err_tx.send(e.to_string());
                }
            })
            .map_err(|e| format!("failed to start tray thread: {e}"))?;

        match err_rx.recv_timeout(Duration::from_millis(800)) {
            Ok(e) => return Err(format!("could not create system tray: {e}")),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err("tray thread exited before registering".into());
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }

        let breath_stop = Arc::new(AtomicBool::new(false));
        let breath_handle = handle.clone();
        let breath_flag = Arc::clone(&breath_stop);
        thread::Builder::new()
            .name("lepramim-tray-breath".into())
            .spawn(move || tray_breath_loop(breath_handle, breath_flag))
            .map_err(|e| format!("failed to start tray breath thread: {e}"))?;

        Ok(Self {
            rx,
            handle,
            breath_stop,
        })
    }

    pub fn update(&self, state: &TraySharedState) {
        self.handle.update(|tray| {
            let keep_mix = tray.state.breath_mix;
            tray.state = state.clone();
            if tray.state.icon_phase == TrayIconPhase::Preparing {
                tray.state.breath_mix = keep_mix;
            }
        });
    }
}

fn tray_breath_loop(handle: Handle<LepramimTray>, stop: Arc<AtomicBool>) {
    let start = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(100));
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let t = start.elapsed().as_secs_f32();
        let mix = 0.5 * (1.0 + (std::f32::consts::TAU * t / 2.0).sin());
        handle.update(|tray| {
            if tray.state.icon_running && tray.state.icon_phase == TrayIconPhase::Preparing {
                tray.state.breath_mix = mix;
            }
        });
    }
}

fn status_notifier_watcher_present() -> bool {
    let Ok(conn) = zbus::blocking::Connection::session() else {
        return false;
    };
    let Ok(dbus) = zbus::blocking::fdo::DBusProxy::new(&conn) else {
        return false;
    };
    for name in [
        "org.kde.StatusNotifierWatcher",
        "org.freedesktop.StatusNotifierWatcher",
    ] {
        let Ok(bus_name) = zbus::names::BusName::try_from(name) else {
            continue;
        };
        if dbus.name_has_owner(bus_name).unwrap_or(false) {
            return true;
        }
    }
    false
}

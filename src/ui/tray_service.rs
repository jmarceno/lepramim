//! StatusNotifier tray via ksni (no GTK). The app must not start without a tray.

use crossbeam_channel::{Receiver, Sender, unbounded};
use std::thread;
use std::time::Duration;

use ksni::menu::{CheckmarkItem, MenuItem, StandardItem};
use ksni::{Handle, Tray, TrayService};

use super::tray_state::{
    MENU_AUTOSTART, MENU_CONTROL, MENU_PAUSE, MENU_QUIT, MENU_SHORTCUT, MENU_SPEAK, MENU_STOP,
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
    pub tooltip: String,
    pub toggle_label: String,
    pub speak_enabled: bool,
    pub pause_enabled: bool,
    pub stop_enabled: bool,
    pub autostart_checked: bool,
}

impl Default for TraySharedState {
    fn default() -> Self {
        Self {
            icon_running: false,
            tooltip: "Lexaloud: stopped".into(),
            toggle_label: "Start daemon".into(),
            speak_enabled: false,
            pause_enabled: false,
            stop_enabled: false,
            autostart_checked: crate::platform::service::autostart_path().is_file(),
        }
    }
}

struct LexaloudTray {
    tx: Sender<TrayEvent>,
    state: TraySharedState,
}

impl LexaloudTray {
    fn send(&self, ev: TrayEvent) {
        let _ = self.tx.send(ev);
    }
}

impl Tray for LexaloudTray {
    fn id(&self) -> String {
        "lexaloud".into()
    }

    fn title(&self) -> String {
        "Lexaloud".into()
    }

    fn status(&self) -> ksni::Status {
        ksni::Status::Active
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::ApplicationStatus
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        icon::render_tray_icon_argb32(self.state.icon_running)
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
        vec![
            StandardItem {
                label: MENU_SHORTCUT.into(),
                enabled: false,
                ..Default::default()
            }
            .into(),
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
        ]
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
    handle: Handle<LexaloudTray>,
}

impl Drop for TrayHandle {
    fn drop(&mut self) {
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
        let service = TrayService::new(LexaloudTray {
            tx,
            state: TraySharedState::default(),
        });
        let handle = service.handle();
        let (err_tx, err_rx) = std::sync::mpsc::channel();
        thread::Builder::new()
            .name("lexaloud-tray".into())
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

        Ok(Self { rx, handle })
    }

    pub fn update(&self, state: &TraySharedState) {
        self.handle.update(|tray| {
            tray.state = state.clone();
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

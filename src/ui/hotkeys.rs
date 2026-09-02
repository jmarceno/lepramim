//! Global hotkeys: KGlobalAccel on KDE + session D-Bus service for GNOME.

use crossbeam_channel::{Receiver, Sender, unbounded};
use iced::futures::StreamExt;
use std::thread;
use std::time::{Duration, Instant};
use zbus::interface;
use zbus::zvariant::OwnedObjectPath;

const COMPONENT: &str = "lexaloud";
const FRIENDLY: &str = "Lexaloud";
/// Qt `QKeySequence::fromString("Meta+R")[0].toCombined()`.
const QT_META: i32 = 0x0100_0000;
const QT_KEY_R: i32 = 0x52;
const QT_KEY_P: i32 = 0x50;
const SET_SHORTCUT_FLAGS: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    SpeakSelection,
    Toggle,
}

pub struct HotkeyManager {
    rx: Receiver<HotkeyEvent>,
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        unregister_kglobal_accel_sync();
    }
}

struct AppDBus {
    tx: Sender<HotkeyEvent>,
}

#[interface(name = "org.lexaloud.App")]
impl AppDBus {
    fn speak_selection(&self) {
        let _ = self.tx.send(HotkeyEvent::SpeakSelection);
    }

    fn toggle(&self) {
        let _ = self.tx.send(HotkeyEvent::Toggle);
    }
}

#[zbus::proxy(
    interface = "org.kde.KGlobalAccel",
    default_service = "org.kde.kglobalaccel",
    default_path = "/kglobalaccel"
)]
trait KGlobalAccel {
    #[zbus(name = "doRegister")]
    fn do_register(&self, action_id: Vec<String>) -> zbus::Result<()>;
    #[zbus(name = "setShortcut")]
    fn set_shortcut(
        &self,
        action_id: Vec<String>,
        keys: Vec<i32>,
        flags: u32,
    ) -> zbus::Result<Vec<i32>>;
    #[zbus(name = "getComponent")]
    fn get_component(&self, component_unique: &str) -> zbus::Result<OwnedObjectPath>;
    #[zbus(name = "unregister")]
    fn unregister(&self, component_unique: &str, unique_name: &str) -> zbus::Result<bool>;
}

#[zbus::proxy(
    interface = "org.kde.kglobalaccel.Component",
    default_service = "org.kde.kglobalaccel"
)]
trait KGlobalAccelComponent {
    #[zbus(name = "isActive")]
    fn is_active(&self) -> zbus::Result<bool>;

    #[zbus(signal, name = "globalShortcutPressed")]
    fn global_shortcut_pressed(
        &self,
        component: String,
        action: String,
        timestamp: i64,
    ) -> zbus::Result<()>;

    #[zbus(signal, name = "globalShortcutReleased")]
    fn global_shortcut_released(
        &self,
        component: String,
        action: String,
        timestamp: i64,
    ) -> zbus::Result<()>;
}

impl HotkeyManager {
    pub fn start() -> Self {
        let (tx, rx) = unbounded();
        let tx_thread = tx.clone();
        thread::Builder::new()
            .name("lexaloud-hotkeys".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        tracing::error!("hotkey runtime: {e}");
                        return;
                    }
                };
                rt.block_on(hotkey_service(tx_thread));
            })
            .ok();
        Self { rx }
    }

    pub fn receiver(&self) -> &Receiver<HotkeyEvent> {
        &self.rx
    }
}

async fn hotkey_service(tx: Sender<HotkeyEvent>) {
    let Ok(conn) = zbus::Connection::session().await else {
        tracing::error!("hotkeys: no session bus");
        return;
    };

    if let Err(e) = conn
        .object_server()
        .at("/org/lexaloud/App", AppDBus { tx: tx.clone() })
        .await
    {
        tracing::error!("hotkeys: export org.lexaloud.App failed: {e}");
        return;
    }
    if let Err(e) = conn.request_name("org.lexaloud.App").await {
        tracing::warn!("hotkeys: request_name org.lexaloud.App: {e}");
    }

    match register_and_listen(&conn, tx).await {
        Ok(()) => {}
        Err(e) => {
            tracing::warn!(
                "hotkeys: KGlobalAccel unavailable ({e}); org.lexaloud.App still exported"
            );
            std::future::pending::<()>().await;
        }
    }
}

async fn register_and_listen(
    conn: &zbus::Connection,
    tx: Sender<HotkeyEvent>,
) -> Result<(), zbus::Error> {
    let accel = KGlobalAccelProxy::new(conn).await?;
    let actions = [
        (
            "speak-selection",
            "Speak highlighted selection",
            QT_META | QT_KEY_R,
        ),
        ("toggle", "Pause / resume", QT_META | QT_KEY_P),
    ];
    for (id, label, key) in actions {
        let action_id = vec![
            COMPONENT.to_string(),
            id.to_string(),
            FRIENDLY.to_string(),
            label.to_string(),
        ];
        accel.do_register(action_id.clone()).await?;
        let assigned = accel
            .set_shortcut(action_id, vec![key], SET_SHORTCUT_FLAGS)
            .await?;
        tracing::info!("hotkeys: setShortcut {id} -> {assigned:?}");
    }

    let path = accel.get_component(COMPONENT).await?;
    let component = KGlobalAccelComponentProxy::builder(conn)
        .path(path)?
        .build()
        .await?;
    match component.is_active().await {
        Ok(true) => {}
        Ok(false) => tracing::warn!("hotkeys: KGlobalAccel component is not active"),
        Err(e) => tracing::warn!("hotkeys: isActive: {e}"),
    }

    let mut pressed = component.receive_global_shortcut_pressed().await?;
    let mut released = component.receive_global_shortcut_released().await?;
    tracing::info!("hotkeys: listening for Meta+R / Meta+P");

    let mut last_at = Instant::now() - Duration::from_secs(1);
    let mut last_action = String::new();
    loop {
        let incoming = tokio::select! {
            Some(s) = pressed.next() => s.args().ok().map(|a| a.action),
            Some(s) = released.next() => s.args().ok().map(|a| a.action),
            else => break,
        };
        let Some(action) = incoming else {
            continue;
        };
        let now = Instant::now();
        if action == last_action && now.duration_since(last_at) < Duration::from_millis(80) {
            continue;
        }
        last_at = now;
        last_action = action.clone();
        match action.as_str() {
            "speak-selection" => {
                let _ = tx.send(HotkeyEvent::SpeakSelection);
            }
            "toggle" => {
                let _ = tx.send(HotkeyEvent::Toggle);
            }
            _ => {}
        }
    }
    Ok(())
}

fn unregister_kglobal_accel_sync() {
    let Ok(conn) = zbus::blocking::Connection::session() else {
        return;
    };
    let Ok(accel) = KGlobalAccelProxyBlocking::new(&conn) else {
        return;
    };
    let _ = accel.unregister(COMPONENT, "speak-selection");
    let _ = accel.unregister(COMPONENT, "toggle");
}

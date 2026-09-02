//! Global hotkeys: KGlobalAccel on KDE + session D-Bus service for GNOME.
//!
//! Qt `QKeySequence("Meta+R")[0].toCombined()` is `Qt::MetaModifier | Key_R`
//! (`0x10000000 | 0x52` = 268435538). Registering any other int leaves Meta+R
//! unbound, so the shortcut does nothing.

use crossbeam_channel::{Receiver, Sender, unbounded};
use iced::futures::StreamExt;
use std::thread;
use std::time::{Duration, Instant};
use zbus::interface;
use zbus::zvariant::OwnedObjectPath;

const COMPONENT: &str = "lexaloud";
const FRIENDLY: &str = "Lexaloud";
/// Qt `MetaModifier` — not `0x01000000`.
const QT_META: i32 = 0x1000_0000;
const QT_KEY_R: i32 = 0x52;
const QT_KEY_P: i32 = 0x50;
const SPEAK_KEY: i32 = QT_META | QT_KEY_R;
const TOGGLE_KEY: i32 = QT_META | QT_KEY_P;
/// KGlobalAccel `SetPresent`. Without this the key is saved but never grabbed.
const SET_PRESENT: u32 = 2;

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

    if let Err(e) = register_and_listen(&conn, tx).await {
        tracing::warn!("hotkeys: KGlobalAccel unavailable ({e}); org.lexaloud.App still exported");
    }
    std::future::pending::<()>().await;
}

async fn register_and_listen(
    conn: &zbus::Connection,
    tx: Sender<HotkeyEvent>,
) -> Result<(), zbus::Error> {
    let accel = KGlobalAccelProxy::new(conn).await?;
    let actions = [
        ("speak-selection", "Speak highlighted selection", SPEAK_KEY),
        ("toggle", "Pause / resume", TOGGLE_KEY),
    ];
    for (id, label, key) in actions {
        let action_id = vec![
            COMPONENT.to_string(),
            id.to_string(),
            FRIENDLY.to_string(),
            label.to_string(),
        ];
        accel.do_register(action_id.clone()).await?;
        let _ = accel.get_component(COMPONENT).await;
        // busctl is the proven `asaiu` marshal for `ai` (same as the working
        // Qt/Python path). zbus is the fallback if busctl is missing.
        let assigned = match busctl_set_shortcut(id, label, key) {
            Ok(keys) => keys,
            Err(e) => {
                tracing::warn!("hotkeys: busctl setShortcut {id} failed ({e}); trying zbus");
                accel
                    .set_shortcut(action_id, vec![key], SET_PRESENT)
                    .await?
            }
        };
        tracing::info!("hotkeys: setShortcut {id} key={key} -> {assigned:?}");
        if !assigned.contains(&key) {
            tracing::warn!(
                "hotkeys: KWin did not keep {id} as {key} (got {assigned:?}); Meta combo may be taken"
            );
        }
    }

    let path = accel.get_component(COMPONENT).await?;
    let component = KGlobalAccelComponentProxy::builder(conn)
        .path(path)?
        .build()
        .await?;
    match component.is_active().await {
        Ok(true) => {}
        Ok(false) => {
            tracing::error!("hotkeys: KGlobalAccel component is inactive; Meta+R will do nothing");
        }
        Err(e) => tracing::warn!("hotkeys: isActive: {e}"),
    }

    let mut pressed = component.receive_global_shortcut_pressed().await?;
    let mut released = component.receive_global_shortcut_released().await?;
    tracing::info!("hotkeys: listening for Meta+R / Meta+P");

    let mut last_at = Instant::now() - Duration::from_secs(1);
    let mut last_action = String::new();
    loop {
        let incoming = tokio::select! {
            Some(s) = pressed.next() => match s.args() {
                Ok(a) => Some(a.action),
                Err(e) => {
                    tracing::warn!("hotkeys: pressed args: {e}");
                    None
                }
            },
            Some(s) = released.next() => match s.args() {
                Ok(a) => Some(a.action),
                Err(e) => {
                    tracing::warn!("hotkeys: released args: {e}");
                    None
                }
            },
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
        tracing::info!("hotkeys: KWin delivered {action}");
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
    tracing::warn!("hotkeys: KGlobalAccel signal stream ended");
    Ok(())
}

fn busctl_set_shortcut(action: &str, label: &str, key: i32) -> Result<Vec<i32>, String> {
    let output = std::process::Command::new("busctl")
        .args([
            "--user",
            "call",
            "org.kde.kglobalaccel",
            "/kglobalaccel",
            "org.kde.KGlobalAccel",
            "setShortcut",
            "asaiu",
            "4",
            COMPONENT,
            action,
            FRIENDLY,
            label,
            "1",
            &key.to_string(),
            &SET_PRESENT.to_string(),
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(if err.trim().is_empty() {
            format!("exit {}", output.status)
        } else {
            err.trim().to_string()
        });
    }
    parse_busctl_ai(&String::from_utf8_lossy(&output.stdout))
}

fn parse_busctl_ai(stdout: &str) -> Result<Vec<i32>, String> {
    let mut parts = stdout.split_whitespace();
    match parts.next() {
        Some("ai") => {}
        other => {
            return Err(format!(
                "unexpected setShortcut reply {:?}: {}",
                other,
                stdout.trim()
            ));
        }
    }
    let n: usize = parts
        .next()
        .ok_or_else(|| stdout.trim().to_string())?
        .parse()
        .map_err(|_| stdout.trim().to_string())?;
    let mut keys = Vec::with_capacity(n);
    for _ in 0..n {
        let value = parts
            .next()
            .ok_or_else(|| stdout.trim().to_string())?
            .parse()
            .map_err(|_| stdout.trim().to_string())?;
        keys.push(value);
    }
    Ok(keys)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qt_meta_r_matches_working_python_binding() {
        assert_eq!(SPEAK_KEY, 268_435_538);
        assert_eq!(TOGGLE_KEY, 268_435_536);
        assert_ne!(SPEAK_KEY, 0x0100_0000 | QT_KEY_R);
    }

    #[test]
    fn parse_busctl_ai_reads_assigned_keys() {
        assert_eq!(
            parse_busctl_ai("ai 1 268435538\n").unwrap(),
            vec![268435538]
        );
        assert_eq!(parse_busctl_ai("ai 0\n").unwrap(), Vec::<i32>::new());
    }
}

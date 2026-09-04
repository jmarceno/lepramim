pub mod capture;
pub mod client;
pub mod controller;
mod hotkeys;
mod icon;
mod tray_service;
mod tray_state;
pub mod voices;

use std::sync::{Arc, Mutex};

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQuickStyle, QString, QUrl};

use crate::config::Config;
use crate::ui::controller::{UiBootstrap, install_bootstrap};
use crate::ui::hotkeys::HotkeyManager;
use crate::ui::tray_service::TrayHandle;

pub fn run(show_control: bool, force_overlay: bool) -> i32 {
    println!(
        "Lepramim {} — local text-to-speech",
        env!("CARGO_PKG_VERSION")
    );

    let _single_guard = match crate::single_instance::acquire() {
        Ok(crate::single_instance::AcquireOutcome::Secondary) => {
            eprintln!("Lepramim is already running — showing the existing control window.");
            return 0;
        }
        Ok(crate::single_instance::AcquireOutcome::Primary(g)) => Some(g),
        Err(e) => {
            tracing::warn!("single-instance check failed (continuing): {e}");
            None
        }
    };
    let single_rx = _single_guard.as_ref().map(|g| g.receiver().clone());

    if let Err(e) = ensure_config_only() {
        eprintln!("{e}");
        return 1;
    }

    let has_display = std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok();
    if !has_display {
        eprintln!("No graphical session (DISPLAY/WAYLAND_DISPLAY unset).");
        return 1;
    }

    let tray = match TrayHandle::start() {
        Ok(tray) => tray,
        Err(e) => {
            eprintln!("Lepramim cannot start without a system tray: {e}");
            return 1;
        }
    };

    let daemon_child = Arc::new(Mutex::new(None));
    let daemon_for_exit = daemon_child.clone();
    let hotkeys = HotkeyManager::start();

    install_bootstrap(UiBootstrap {
        tray,
        hotkeys,
        daemon_child,
        single_rx,
        show_control,
        force_overlay,
    });

    QQuickStyle::set_style(&QString::from("Basic"));

    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();

    if let Some(mut app) = app.as_mut() {
        app.as_mut()
            .set_application_name(&QString::from("lepramim"));
        app.as_mut()
            .set_organization_name(&QString::from("Lepramim"));
    }

    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/app/lepramim/qml/Main.qml"));
    }

    let code = if let Some(app) = app.as_mut() {
        app.exec()
    } else {
        1
    };

    shutdown_daemon_sync(daemon_for_exit);
    code
}

fn ensure_config_only() -> Result<(), String> {
    let cfg_path = crate::config::config_path();
    if let Some(parent) = cfg_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("config dir: {e}"))?;
    }
    if !cfg_path.exists() {
        let default = Config::default();
        let toml_str = toml::to_string(&default).map_err(|e| format!("serialize config: {e}"))?;
        std::fs::write(&cfg_path, toml_str).map_err(|e| format!("write config: {e}"))?;
    }
    Ok(())
}

fn shutdown_daemon_sync(daemon_child: Arc<Mutex<Option<std::process::Child>>>) {
    let _ = client::post_shutdown();
    let rt = tokio::runtime::Runtime::new().expect("tokio");
    rt.block_on(async {
        for _ in 0..25 {
            if crate::api::uds_get("/healthz").await.is_err() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        if let Ok(mut guard) = daemon_child.lock() {
            if let Some(ref mut c) = *guard {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    });
}

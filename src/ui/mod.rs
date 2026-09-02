pub mod capture;
pub mod client;
mod hotkeys;
mod icon;
mod tray_service;
mod tray_state;
mod voices;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iced::time;
use iced::widget::{
    Space, button, checkbox, column, container, horizontal_space, pick_list, progress_bar, row,
    scrollable, slider, text,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Subscription, Task, Theme, window};

use crate::config::Config;
use crate::models;
use crate::ui::capture::{toggle_playback, SelectionCapture};
use crate::ui::client::ApiResult;
use crate::ui::hotkeys::{HotkeyEvent, HotkeyManager};
use crate::ui::tray_service::{TrayEvent, TrayHandle, TraySharedState};
use crate::ui::tray_state::{tray_icon_phase, tray_state_for_daemon, TrayIconPhase};
use crate::ui::voices::{
    ControlForm, KOKORO_VOICES, LANGUAGES, language_label, speed_from_slider,
    speed_hint_for_value,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum WindowKind {
    Control,
    Overlay,
    Onboarding,
    Warning,
}

#[derive(Debug, Clone)]
struct PlaybackState {
    state: String,
    current_sentence: String,
    session_providers: Vec<String>,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            state: "idle".into(),
            current_sentence: String::new(),
            session_providers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct DownloadProgress {
    filename: String,
    percent: u8,
    status: String,
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
    OverlayTick,
    PlaybackPoll,
    Tray(TrayEvent),
    Hotkey(HotkeyEvent),
    DaemonSpawned(Result<(), String>),
    StatePolled(Result<ApiResult, ()>),
    OverlayPolled(Result<ApiResult, ()>),
    ConfigLoaded(Result<ApiResult, ()>),
    ModelsLoaded(Result<ApiResult, ()>),
    ControlTabSelected(usize),
    VoiceSelected(String),
    LangSelected(String),
    FilterVoicesToggled(bool),
    SpeedChanged(i32),
    OverlayToggled(bool),
    DedupeToggled(bool),
    StripMarkdownToggled(bool),
    StripCitationsToggled(bool),
    ExpandLatinToggled(bool),
    NormalizeNumbersToggled(bool),
    ApplySettings,
    SettingsApplied(Result<ApiResult, ()>),
    TestSpeak,
    TestSpeakDone(Result<ApiResult, ()>),
    CloseControl,
    OpenControl,
    OpenOverlay,
    CloseOverlay,
    OpenOnboarding,
    CloseOnboarding,
    OnboardingSkip,
    OnboardingContinue,
    DownloadProgress(DownloadProgress),
    DownloadFinished(Result<(), String>),
    StartDownload,
    RefreshModels,
    OverlayPause,
    OverlaySkip,
    OverlayStop,
    InputPoll,
    SurfaceOpened(window::Id),
    WindowClosed(window::Id),
    SpeakNow,
    SelectionCaptured(SelectionCapture),
    SelectionPosted(Result<ApiResult, ()>),
    Quit,
    ShowWarning(String),
    CloseWarning,
}

struct App {
    windows: BTreeMap<window::Id, WindowKind>,
    tray: TrayHandle,
    hotkeys: HotkeyManager,
    daemon_child: Arc<Mutex<Option<std::process::Child>>>,
    playback: PlaybackState,
    tray_state: TraySharedState,
    control_tab: usize,
    control_form: ControlForm,
    base_config: Config,
    models_status: String,
    show_control_on_start: bool,
    force_overlay: bool,
    overlay_enabled: bool,
    overlay_visible: bool,
    onboarding_visible: bool,
    onboarding_skipped: bool,
    download_active: bool,
    download_rx: Option<crossbeam_channel::Receiver<DownloadProgress>>,
    download_progress: DownloadProgress,
    warning_text: Option<String>,
    quit_requested: bool,
    preparing_speech: bool,
}

impl App {
    fn new(
        show_control: bool,
        force_overlay: bool,
        daemon_child: Arc<Mutex<Option<std::process::Child>>>,
        tray: TrayHandle,
    ) -> Self {
        let hotkeys = HotkeyManager::start();
        let missing = models::artifacts_missing();
        let base_config = crate::config::load_config(None::<&std::path::Path>);
        let control_form = ControlForm::load_from_config(&base_config);
        let overlay_enabled = force_overlay || base_config.advanced.overlay;
        Self {
            windows: BTreeMap::new(),
            tray,
            hotkeys,
            daemon_child,
            playback: PlaybackState::default(),
            tray_state: TraySharedState::default(),
            control_tab: 0,
            control_form,
            base_config,
            models_status: String::new(),
            show_control_on_start: show_control,
            force_overlay,
            overlay_enabled,
            overlay_visible: false,
            onboarding_visible: missing,
            onboarding_skipped: false,
            download_active: false,
            download_rx: None,
            download_progress: DownloadProgress {
                filename: String::new(),
                percent: 0,
                status: "Downloading the Kokoro speech model\u{2026}".into(),
            },
            warning_text: None,
            quit_requested: false,
            preparing_speech: false,
        }
    }

    fn refresh_tray(&self) {
        self.tray.update(&self.tray_state);
    }

    fn set_preparing_speech(&mut self, preparing: bool) {
        self.preparing_speech = preparing;
        if !self.tray_state.icon_running {
            return;
        }
        let icon_phase = tray_icon_phase(
            &self.playback.state,
            self.preparing_speech,
            &self.playback.current_sentence,
        );
        self.tray_state.icon_phase = icon_phase;
        if icon_phase != TrayIconPhase::Preparing {
            self.tray_state.breath_mix = 0.0;
        }
        self.refresh_tray();
    }

    fn apply_playback(&mut self, active: bool, state_str: &str) {
        let cpu_fallback = !self.playback.session_providers.is_empty()
            && !self
                .playback
                .session_providers
                .iter()
                .any(|p| p.contains("CUDA"));
        let s = tray_state_for_daemon(state_str, active, cpu_fallback);
        let icon_phase = if s.icon_running {
            tray_icon_phase(
                state_str,
                self.preparing_speech,
                &self.playback.current_sentence,
            )
        } else {
            TrayIconPhase::Idle
        };
        if icon_phase == TrayIconPhase::Speaking {
            self.preparing_speech = false;
        }
        let breath_mix = if icon_phase == TrayIconPhase::Preparing {
            self.tray_state.breath_mix
        } else {
            0.0
        };
        self.tray_state = TraySharedState {
            icon_running: s.icon_running,
            icon_phase,
            breath_mix,
            tooltip: s.tooltip,
            toggle_label: s.toggle_label,
            speak_enabled: s.speak_enabled,
            pause_enabled: s.pause_enabled,
            stop_enabled: s.stop_enabled,
            autostart_checked: crate::platform::service::autostart_path().is_file(),
            cpu_fallback: s.cpu_fallback,
        };
        self.refresh_tray();
    }

    fn overlay_should_show(&self) -> bool {
        (self.overlay_enabled || self.force_overlay)
            && (self.playback.state == "speaking" || self.playback.state == "paused")
    }
}

pub fn run(show_control: bool, force_overlay: bool) -> i32 {
    println!(
        "Lexaloud {} — local text-to-speech",
        env!("CARGO_PKG_VERSION")
    );

    if let Err(e) = ensure_config_only() {
        eprintln!("{e}");
        return 1;
    }

    let has_display = std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok();
    if !has_display {
        eprintln!("No graphical session (DISPLAY/WAYLAND_DISPLAY unset).");
        return 1;
    }
    configure_iced_backend();

    let tray = match TrayHandle::start() {
        Ok(tray) => tray,
        Err(e) => {
            eprintln!("Lexaloud cannot start without a system tray: {e}");
            return 1;
        }
    };

    let daemon_child = Arc::new(Mutex::new(None));
    let daemon_for_exit = daemon_child.clone();
    let daemon_boot = daemon_child.clone();

    let result = iced::daemon(title, update, view)
        .settings(iced::Settings {
            id: Some("lexaloud".into()),
            ..Default::default()
        })
        .subscription(subscription)
        .theme(|_, _| Theme::Dark)
        .run_with(move || {
            let mut app = App::new(show_control, force_overlay, daemon_boot, tray);
            let dc = app.daemon_child.clone();
            let mut tasks = vec![Task::done(Message::Tick)];
            if app.onboarding_visible {
                tasks.push(open_window(&mut app, WindowKind::Onboarding, 420.0, 220.0));
            } else {
                tasks.push(spawn_daemon_task(dc));
            }
            if app.show_control_on_start {
                tasks.push(open_window(&mut app, WindowKind::Control, 540.0, 520.0));
            }
            if app.force_overlay {
                tasks.push(open_window(&mut app, WindowKind::Overlay, 500.0, 80.0));
            }
            (app, Task::batch(tasks))
        });

    shutdown_daemon_sync(daemon_for_exit);
    if result.is_ok() { 0 } else { 1 }
}

fn configure_iced_backend() {
    let is_wayland = std::env::var("XDG_SESSION_TYPE")
        .map(|session| session.eq_ignore_ascii_case("wayland"))
        .unwrap_or(false);
    let is_kde = std::env::var("XDG_CURRENT_DESKTOP")
        .map(|desktop| desktop.to_ascii_uppercase().contains("KDE"))
        .unwrap_or(false);
    let has_xwayland = std::env::var("DISPLAY").is_ok();
    let Ok(wayland_display) = std::env::var("WAYLAND_DISPLAY") else {
        return;
    };
    if !is_wayland || !is_kde || !has_xwayland {
        return;
    }

    // iced 0.13 creates an internal hidden Wayland surface for its compositor.
    // Plasma exposes that surface as a permanent "winit window" task. X11 can
    // represent the compositor window as truly hidden. Preserve the Wayland
    // socket for capture helpers before forcing only Iced onto XWayland.
    //
    // SAFETY: this runs before the tray, hotkey, daemon, or Iced threads start.
    unsafe {
        std::env::set_var("LEXALOUD_WAYLAND_DISPLAY", wayland_display);
        std::env::remove_var("WAYLAND_DISPLAY");
    }
    tracing::info!("using XWayland for Iced to avoid Plasma's phantom winit task");
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

async fn spawn_daemon_async(
    daemon_child: Arc<Mutex<Option<std::process::Child>>>,
) -> Result<(), String> {
    if is_daemon_healthy().await {
        return Ok(());
    }
    let bin =
        std::env::current_exe().unwrap_or_else(|_| crate::platform::service::resolve_binary_path());
    let rt = crate::config::runtime_dir().join("lexaloud");
    let _ = std::fs::create_dir_all(&rt);
    let log_path = rt.join("daemon.log");
    let mut cmd = std::process::Command::new(&bin);
    cmd.arg("daemon").stdin(std::process::Stdio::null());
    if let Ok(log) = std::fs::File::create(&log_path) {
        if let Ok(log2) = log.try_clone() {
            cmd.stdout(std::process::Stdio::from(log));
            cmd.stderr(std::process::Stdio::from(log2));
        }
    }
    let child = cmd
        .spawn()
        .map_err(|e| format!("failed to start daemon: {e}"))?;
    for _ in 0..150 {
        tokio::time::sleep(Duration::from_millis(200)).await;
        if is_daemon_healthy().await {
            if let Ok(mut guard) = daemon_child.lock() {
                *guard = Some(child);
            }
            return Ok(());
        }
    }
    Err(format!(
        "daemon did not become ready. See {}",
        log_path.display()
    ))
}

async fn is_daemon_healthy() -> bool {
    crate::api::uds_get("/healthz").await.is_ok()
}

fn shutdown_daemon_sync(daemon_child: Arc<Mutex<Option<std::process::Child>>>) {
    let rt = tokio::runtime::Runtime::new().expect("tokio");
    rt.block_on(async {
        let sock = crate::config::socket_path();
        if let Ok(mut stream) = tokio::net::UnixStream::connect(&sock).await {
            use tokio::io::AsyncWriteExt;
            let req = b"POST /shutdown HTTP/1.1\r\nHost: lexaloud\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}";
            let _ = stream.write_all(req).await;
            let _ = stream.flush().await;
        }
        for _ in 0..25 {
            if !is_daemon_healthy().await {
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        if let Ok(mut guard) = daemon_child.lock() {
            if let Some(ref mut c) = *guard {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    });
}

fn open_window(app: &mut App, kind: WindowKind, width: f32, height: f32) -> Task<Message> {
    if app.windows.values().any(|k| *k == kind) {
        return Task::none();
    }
    let level = if kind == WindowKind::Overlay {
        window::Level::AlwaysOnTop
    } else {
        window::Level::Normal
    };
    let transparent = kind == WindowKind::Overlay;
    let decorations = kind != WindowKind::Overlay;
    let (id, open_task) = window::open(window::Settings {
        size: iced::Size::new(width, height),
        decorations,
        transparent,
        level,
        platform_specific: window::settings::PlatformSpecific {
            application_id: "lexaloud".into(),
            override_redirect: false,
        },
        ..Default::default()
    });
    app.windows.insert(id, kind);
    if kind == WindowKind::Overlay {
        app.overlay_visible = true;
    }
    open_task.discard()
}

fn spawn_daemon_task(daemon_child: Arc<Mutex<Option<std::process::Child>>>) -> Task<Message> {
    Task::perform(spawn_daemon_async(daemon_child), Message::DaemonSpawned)
}

fn poll_daemon_state() -> Task<Message> {
    Task::perform(async { client::get_healthz() }, |r| {
        if r.is_success() {
            Message::StatePolled(Ok(client::get_state()))
        } else {
            Message::StatePolled(Ok(ApiResult {
                status_code: 0,
                json: serde_json::json!({ "state": "idle" }),
                error: String::new(),
                raw_body: Vec::new(),
            }))
        }
    })
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::InputPoll => {
            let pending = drain_input(app);
            if app.quit_requested {
                return iced::exit();
            }
            pending
        }
        Message::Tick => {
            let pending = drain_input(app);
            if app.quit_requested {
                return iced::exit();
            }
            while let Some(rx) = app.download_rx.as_ref() {
                match rx.try_recv() {
                    Ok(p) => {
                        if p.percent >= 100 || p.status.starts_with("Download failed") {
                            app.download_progress = p.clone();
                            app.download_active = false;
                            app.download_rx = None;
                            if p.status.starts_with("Download failed") {
                                return Task::done(Message::DownloadFinished(Err(p.status)));
                            }
                            return Task::done(Message::DownloadFinished(Ok(())));
                        }
                        app.download_progress = p;
                    }
                    Err(crossbeam_channel::TryRecvError::Empty) => break,
                    Err(crossbeam_channel::TryRecvError::Disconnected) => {
                        app.download_rx = None;
                        app.download_active = false;
                        break;
                    }
                }
            }
            let poll = poll_daemon_state();
            Task::batch([pending, poll])
        }
        Message::OverlayTick => {
            if !app.overlay_should_show() {
                app.overlay_visible = false;
                return Task::none();
            }
            Task::perform(async { client::get_state() }, |r| {
                Message::OverlayPolled(Ok(r))
            })
        }
        Message::PlaybackPoll => poll_daemon_state(),
        Message::Tray(ev) => {
            let task = handle_tray(app, ev);
            if app.quit_requested {
                return iced::exit();
            }
            task
        }
        Message::Hotkey(ev) => handle_hotkey(ev),
        Message::DaemonSpawned(Ok(())) => Task::perform(async { client::get_config() }, |r| {
            Message::ConfigLoaded(Ok(r))
        }),
        Message::DaemonSpawned(Err(e)) => {
            app.warning_text = Some(e);
            open_window(app, WindowKind::Warning, 420.0, 140.0)
        }
        Message::StatePolled(Ok(r)) => {
            let active = r.status_code == 200 && client::get_healthz().is_success();
            let state_str = r
                .json
                .get("state")
                .and_then(|v| v.as_str())
                .unwrap_or("idle");
            app.playback.state = state_str.to_string();
            app.playback.current_sentence = r
                .json
                .get("current_sentence")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            app.playback.session_providers = r
                .json
                .get("session_providers")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            app.apply_playback(active, state_str);
            if app.overlay_should_show() && !app.overlay_visible {
                app.overlay_visible = true;
                return open_window(app, WindowKind::Overlay, 500.0, 80.0);
            }
            if !app.overlay_should_show() && app.overlay_visible {
                app.overlay_visible = false;
                let ids: Vec<_> = app
                    .windows
                    .iter()
                    .filter(|(_, k)| **k == WindowKind::Overlay)
                    .map(|(id, _)| *id)
                    .collect();
                return Task::batch(
                    ids.into_iter()
                        .map(|id| window::close(id).map(move |_: ()| Message::WindowClosed(id)))
                        .collect::<Vec<_>>(),
                );
            }
            Task::none()
        }
        Message::StatePolled(Err(_)) => Task::none(),
        Message::OverlayPolled(Ok(r)) => {
            if r.is_success() {
                app.playback.state = r
                    .json
                    .get("state")
                    .and_then(|v| v.as_str())
                    .unwrap_or("idle")
                    .to_string();
                app.playback.current_sentence = r
                    .json
                    .get("current_sentence")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
            }
            Task::none()
        }
        Message::OverlayPolled(Err(_)) => Task::none(),
        Message::ConfigLoaded(Ok(r)) if r.is_success() => {
            if let Ok(cfg) = serde_json::from_value::<Config>(r.json) {
                app.base_config = cfg.clone();
                app.control_form = ControlForm::load_from_config(&cfg);
                app.overlay_enabled = cfg.advanced.overlay;
            }
            Task::none()
        }
        Message::ConfigLoaded(_) => Task::none(),
        Message::ModelsLoaded(Ok(r)) if r.is_success() => {
            app.models_status = format_models_status(&r.json);
            Task::none()
        }
        Message::ModelsLoaded(_) => Task::none(),
        Message::ControlTabSelected(tab) => {
            app.control_tab = tab;
            if tab == 3 {
                return Task::perform(async { client::get_models_status() }, |r| {
                    Message::ModelsLoaded(Ok(r))
                });
            }
            Task::none()
        }
        Message::VoiceSelected(v) => {
            app.control_form.voice = v;
            app.control_form.unknown_voice_note =
                !KOKORO_VOICES.iter().any(|e| e.id == app.control_form.voice);
            Task::none()
        }
        Message::LangSelected(l) => {
            app.control_form.lang = l;
            app.control_form.ensure_voice_matches_filter();
            Task::none()
        }
        Message::FilterVoicesToggled(v) => {
            app.control_form.filter_voices_by_lang = v;
            app.control_form.ensure_voice_matches_filter();
            Task::none()
        }
        Message::SpeedChanged(v) => {
            app.control_form.speed_slider = v;
            Task::none()
        }
        Message::OverlayToggled(v) => {
            app.control_form.overlay = v;
            app.overlay_enabled = v;
            Task::none()
        }
        Message::DedupeToggled(v) => {
            app.control_form.dedupe_mathjax = v;
            Task::none()
        }
        Message::StripMarkdownToggled(v) => {
            app.control_form.strip_markdown = v;
            Task::none()
        }
        Message::StripCitationsToggled(v) => {
            app.control_form.strip_numeric_citations = v;
            Task::none()
        }
        Message::ExpandLatinToggled(v) => {
            app.control_form.expand_latin = v;
            Task::none()
        }
        Message::NormalizeNumbersToggled(v) => {
            app.control_form.normalize_numbers = v;
            Task::none()
        }
        Message::ApplySettings => {
            let merged = app.control_form.merge_into_config(&app.base_config);
            Task::perform(async move { client::post_config(&merged) }, |r| {
                Message::SettingsApplied(Ok(r))
            })
        }
        Message::SettingsApplied(Ok(r)) if r.is_success() => {
            app.control_form.status = format!(
                "Saved voice={}, lang={}, speed={:.2}\u{00d7}; it applies on the next playback start.",
                app.control_form.voice,
                app.control_form.lang,
                speed_from_slider(app.control_form.speed_slider)
            );
            app.overlay_enabled = app.control_form.overlay;
            Task::perform(async { client::get_config() }, |r| {
                Message::ConfigLoaded(Ok(r))
            })
        }
        Message::SettingsApplied(Ok(r)) => {
            app.control_form.status = format!("Saving config failed: {}", r.error);
            Task::none()
        }
        Message::SettingsApplied(Err(_)) => Task::none(),
        Message::TestSpeak => {
            app.set_preparing_speech(true);
            Task::perform(
                async { client::post_speak("Hello from Lexaloud. This is a test.", "replace") },
                |r| Message::TestSpeakDone(Ok(r)),
            )
        }
        Message::TestSpeakDone(Ok(r)) if r.is_success() => {
            app.control_form.status = "Test speak sent.".into();
            Task::none()
        }
        Message::TestSpeakDone(Ok(r)) => {
            app.set_preparing_speech(false);
            app.control_form.status = format!("Test speak failed: {}", r.error);
            capture::notify_speak_result(&r);
            Task::none()
        }
        Message::TestSpeakDone(Err(_)) => Task::none(),
        Message::CloseControl => {
            let ids: Vec<_> = app
                .windows
                .iter()
                .filter(|(_, k)| **k == WindowKind::Control)
                .map(|(id, _)| *id)
                .collect();
            Task::batch(
                ids.into_iter()
                    .map(|id| window::close(id).map(move |_: ()| Message::WindowClosed(id)))
                    .collect::<Vec<_>>(),
            )
        }
        Message::OpenControl => open_window(app, WindowKind::Control, 540.0, 520.0),
        Message::OpenOverlay => open_window(app, WindowKind::Overlay, 500.0, 80.0),
        Message::CloseOverlay => {
            app.overlay_visible = false;
            Task::none()
        }
        Message::OpenOnboarding => open_window(app, WindowKind::Onboarding, 420.0, 220.0),
        Message::CloseOnboarding => Task::none(),
        Message::OnboardingSkip => {
            app.onboarding_skipped = true;
            app.onboarding_visible = false;
            let ids: Vec<_> = app
                .windows
                .iter()
                .filter(|(_, k)| **k == WindowKind::Onboarding)
                .map(|(id, _)| *id)
                .collect();
            let close = Task::batch(
                ids.into_iter()
                    .map(|id| window::close(id).map(move |_: ()| Message::WindowClosed(id)))
                    .collect::<Vec<_>>(),
            );
            if !app.download_active && models::artifacts_missing() {
                return Task::batch(vec![close, Task::done(Message::StartDownload)]);
            }
            close
        }
        Message::OnboardingContinue => {
            if models::artifacts_missing() && !app.download_active {
                return Task::done(Message::StartDownload);
            }
            if !models::artifacts_missing() {
                app.onboarding_visible = false;
                let ids: Vec<_> = app
                    .windows
                    .iter()
                    .filter(|(_, k)| **k == WindowKind::Onboarding)
                    .map(|(id, _)| *id)
                    .collect();
                return Task::batch(vec![
                    Task::batch(
                        ids.into_iter()
                            .map(|id| window::close(id).map(move |_: ()| Message::WindowClosed(id)))
                            .collect::<Vec<_>>(),
                    ),
                    spawn_daemon_task(app.daemon_child.clone()),
                ]);
            }
            Task::none()
        }
        Message::StartDownload => {
            if app.download_active {
                return Task::none();
            }
            app.download_active = true;
            let (tx, rx) = crossbeam_channel::unbounded();
            app.download_rx = Some(rx);
            std::thread::spawn(move || {
                let cb = |filename: &str, percent: u8| {
                    let _ = tx.send(DownloadProgress {
                        filename: filename.to_string(),
                        percent,
                        status: format!("Downloading {filename}\u{2026}"),
                    });
                };
                let result = models::ensure_artifacts_with_progress(None, true, cb);
                let _ = tx.send(DownloadProgress {
                    filename: String::new(),
                    percent: 100,
                    status: match result {
                        Ok(_) => "Models ready.".into(),
                        Err(e) => format!("Download failed: {e}"),
                    },
                });
            });
            Task::none()
        }
        Message::DownloadProgress(p) => {
            app.download_progress = p;
            Task::none()
        }
        Message::DownloadFinished(Ok(())) => {
            app.download_active = false;
            app.download_progress.percent = 100;
            app.download_progress.status = "Models ready.".into();
            if app.onboarding_visible && !app.onboarding_skipped {
                return spawn_daemon_task(app.daemon_child.clone());
            }
            if models::artifacts_missing() {
                return Task::none();
            }
            if app
                .daemon_child
                .lock()
                .ok()
                .map(|g| g.is_none())
                .unwrap_or(true)
            {
                return spawn_daemon_task(app.daemon_child.clone());
            }
            Task::none()
        }
        Message::DownloadFinished(Err(e)) => {
            app.download_active = false;
            app.download_progress.status = format!("Download failed: {e}");
            Task::none()
        }
        Message::RefreshModels => Task::perform(async { client::get_models_status() }, |r| {
            Message::ModelsLoaded(Ok(r))
        }),
        Message::OverlayPause => {
            let _ = client::post_toggle();
            Task::none()
        }
        Message::OverlaySkip => {
            let _ = client::post_skip();
            Task::none()
        }
        Message::OverlayStop => {
            let _ = client::post_stop();
            Task::none()
        }
        Message::SurfaceOpened(id) => {
            if app.windows.contains_key(&id) {
                return Task::none();
            }
            window::change_mode(id, window::Mode::Hidden)
        }
        Message::SpeakNow => Task::perform(
            async { crate::ui::capture::capture_highlighted_text() },
            Message::SelectionCaptured,
        ),
        Message::SelectionCaptured(cap) => {
            if cap.text.is_empty() {
                crate::ui::capture::notify_empty_selection();
                return Task::none();
            }
            if cap.truncated {
                crate::ui::capture::notify_truncated_selection();
            }
            app.set_preparing_speech(true);
            let text = cap.text;
            Task::perform(
                async move { client::post_speak(&text, "replace") },
                |r| Message::SelectionPosted(Ok(r)),
            )
        }
        Message::SelectionPosted(Ok(r)) => {
            if !r.is_success() || r.is_daemon_down() {
                app.set_preparing_speech(false);
                capture::notify_speak_result(&r);
            }
            Task::none()
        }
        Message::SelectionPosted(Err(_)) => {
            app.set_preparing_speech(false);
            Task::none()
        }
        Message::WindowClosed(id) => {
            if let Some(kind) = app.windows.remove(&id) {
                if kind == WindowKind::Overlay {
                    app.overlay_visible = false;
                }
                if kind == WindowKind::Onboarding {
                    app.onboarding_visible = false;
                }
            }
            Task::none()
        }
        Message::Quit => iced::exit(),
        Message::ShowWarning(text) => {
            app.warning_text = Some(text);
            open_window(app, WindowKind::Warning, 420.0, 140.0)
        }
        Message::CloseWarning => {
            app.warning_text = None;
            Task::none()
        }
    }
}

fn drain_input(app: &mut App) -> Task<Message> {
    let mut pending = Task::none();
    while let Ok(ev) = app.tray.rx.try_recv() {
        pending = Task::batch([pending, handle_tray(app, ev)]);
        if app.quit_requested {
            return pending;
        }
    }
    while let Ok(ev) = app.hotkeys.receiver().try_recv() {
        pending = Task::batch([pending, handle_hotkey(ev)]);
    }
    pending
}

fn handle_tray(app: &mut App, ev: TrayEvent) -> Task<Message> {
    match ev {
        TrayEvent::ToggleDaemon => {
            if client::get_healthz().is_success() {
                let _ = client::post_shutdown();
                if let Ok(mut guard) = app.daemon_child.lock() {
                    if let Some(ref mut c) = *guard {
                        let _ = c.wait();
                    }
                    *guard = None;
                }
            } else if !models::artifacts_missing() {
                return spawn_daemon_task(app.daemon_child.clone());
            } else {
                return Task::done(Message::OpenOnboarding);
            }
            Task::none()
        }
        TrayEvent::SpeakSelection => Task::perform(
            async {
                // Let the StatusNotifier menu close and restore focus to the
                // window that owns the highlighted text before sending Ctrl+C.
                tokio::time::sleep(Duration::from_millis(300)).await;
            },
            |_| Message::SpeakNow,
        ),
        TrayEvent::PauseResume => {
            toggle_playback();
            Task::none()
        }
        TrayEvent::StopPlayback => {
            let _ = client::post_stop();
            Task::none()
        }
        TrayEvent::OpenControl => Task::done(Message::OpenControl),
        TrayEvent::ToggleAutostart => {
            let path = crate::platform::service::autostart_path();
            if path.is_file() {
                let _ = crate::platform::service::remove_autostart();
            } else {
                let _ = crate::platform::service::write_autostart(
                    &crate::platform::service::resolve_binary_path(),
                );
            }
            app.tray_state.autostart_checked = crate::platform::service::autostart_path().is_file();
            app.refresh_tray();
            Task::none()
        }
        TrayEvent::Quit => {
            app.quit_requested = true;
            Task::none()
        }
    }
}

fn handle_hotkey(ev: HotkeyEvent) -> Task<Message> {
    match ev {
        HotkeyEvent::SpeakSelection => Task::perform(
            async {
                // Allow the user to release the physical Meta key so injecting
                // Ctrl+C does not produce Meta+Ctrl+C.
                tokio::time::sleep(Duration::from_millis(250)).await;
            },
            |_| Message::SpeakNow,
        ),
        HotkeyEvent::Toggle => {
            toggle_playback();
            Task::none()
        }
    }
}

fn format_models_status(json: &serde_json::Value) -> String {
    let mut lines = Vec::new();
    if let Some(obj) = json.as_object() {
        for (name, info) in obj {
            let present = info
                .get("present")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let size = info.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
            let expected = info
                .get("expected_size")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            if present {
                lines.push(format!("{name}: present ({size} / {expected} bytes)"));
            } else {
                lines.push(format!("{name}: missing (expected {expected} bytes)"));
            }
        }
    }
    if lines.is_empty() {
        "No model status available.".into()
    } else {
        lines.join("\n")
    }
}

fn subscription(app: &App) -> Subscription<Message> {
    let tick = time::every(Duration::from_secs(1)).map(|_| Message::Tick);
    let input = time::every(Duration::from_millis(10)).map(|_| Message::InputPoll);
    let overlay = if app.overlay_visible {
        time::every(Duration::from_millis(200)).map(|_| Message::OverlayTick)
    } else {
        Subscription::none()
    };
    let playback_watch = if app.preparing_speech
        || app.playback.state == "speaking"
        || app.playback.state == "paused"
    {
        time::every(Duration::from_millis(200)).map(|_| Message::PlaybackPoll)
    } else {
        Subscription::none()
    };
    let opened = window::open_events().map(Message::SurfaceOpened);
    let closed = window::close_events().map(Message::WindowClosed);
    Subscription::batch([tick, input, overlay, playback_watch, opened, closed])
}

fn title(app: &App, id: window::Id) -> String {
    match app.windows.get(&id) {
        Some(WindowKind::Control) => "Lexaloud — Control".into(),
        Some(WindowKind::Overlay) => "Lexaloud Overlay".into(),
        Some(WindowKind::Onboarding) => "Lexaloud — preparing speech".into(),
        Some(WindowKind::Warning) => "Lexaloud".into(),
        None => "Lexaloud".into(),
    }
}

fn view(app: &App, id: window::Id) -> Element<'_, Message> {
    match app.windows.get(&id) {
        Some(WindowKind::Control) => view_control(app),
        Some(WindowKind::Overlay) => view_overlay(app),
        Some(WindowKind::Onboarding) => view_onboarding(app),
        Some(WindowKind::Warning) => view_warning(app),
        None => Space::new(Length::Fill, Length::Fill).into(),
    }
}

fn tab_button(label: &'static str, index: usize, active: usize) -> Element<'static, Message> {
    // Official widget styles only: the active tab reads as the primary
    // action, the rest keep the default button look.
    let btn = button(text(label).size(14))
        .padding([8, 14])
        .on_press(Message::ControlTabSelected(index));
    if active == index {
        btn.style(button::primary).into()
    } else {
        btn.into()
    }
}

// Explicit dark surface: `bordered_box` derives `background.weak`, which
// mixes toward near-white in linear space and lands on mid-grey.
// Square corners fit the OS-decorated frame, and NO shadow -- container
// shadows fill black under the tiny-skia backend (iced#2339).
// NOTE: this style is applied to the container OUTSIDE the scrollable, never
// to a `Fill`-sized child inside it -- unbounded scrollable limits + `Fill`
// crash layout on window open.
fn panel_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb8(0x2A, 0x2D, 0x32))),
        border: Border {
            radius: 0.0.into(),
            width: 1.0,
            color: Color::from_rgb8(0x3A, 0x3E, 0x44),
        },
        ..Default::default()
    }
}

fn hint(content: String) -> Element<'static, Message> {
    text(content)
        .size(12)
        .style(iced::widget::text::secondary)
        .into()
}

fn view_control(app: &App) -> Element<'_, Message> {
    let visible = app.control_form.visible_voices();
    let voice_labels: Vec<String> = visible.iter().map(|v| v.label.to_string()).collect();
    let lang_labels: Vec<String> = LANGUAGES.iter().map(|l| l.label.to_string()).collect();
    let current_voice = KOKORO_VOICES
        .iter()
        .find(|v| v.id == app.control_form.voice)
        .map(|v| v.label.to_string())
        .unwrap_or_else(|| app.control_form.voice.clone());
    let current_lang = LANGUAGES
        .iter()
        .find(|l| l.id == app.control_form.lang)
        .map(|l| l.label.to_string())
        .unwrap_or_else(|| app.control_form.lang.clone());
    let speed = speed_from_slider(app.control_form.speed_slider);

    let tabs = row![
        tab_button("Voice", 0, app.control_tab),
        tab_button("Preprocessor", 1, app.control_tab),
        tab_button("Advanced", 2, app.control_tab),
        tab_button("Models", 3, app.control_tab),
    ]
    .spacing(8);

    let tab_body: Element<Message> = match app.control_tab {
        1 => column![
                checkbox(
                    "Deduplicate MathJax selection",
                    app.control_form.dedupe_mathjax
                )
                .on_toggle(Message::DedupeToggled),
                checkbox("Strip Markdown", app.control_form.strip_markdown)
                    .on_toggle(Message::StripMarkdownToggled),
                checkbox(
                    "Strip numeric bracket citations",
                    app.control_form.strip_numeric_citations
                )
                .on_toggle(Message::StripCitationsToggled),
                checkbox("Expand Latin abbreviations", app.control_form.expand_latin)
                    .on_toggle(Message::ExpandLatinToggled),
                checkbox("Normalize numbers", app.control_form.normalize_numbers)
                    .on_toggle(Message::NormalizeNumbersToggled),
            ]
            .spacing(8)
            .into(),
        2 => column![
                checkbox(
                    "Show floating overlay when speaking",
                    app.control_form.overlay
                )
                .on_toggle(Message::OverlayToggled),
                text("The overlay floats above other windows with pause, skip and stop controls.")
                    .size(12)
                    .style(iced::widget::text::secondary),
            ]
            .spacing(8)
            .into(),
        3 => column![
                text(app.models_status.clone()).size(14),
                row![
                    button("Refresh").on_press(Message::RefreshModels),
                    button("Download missing models").on_press(Message::StartDownload),
                ]
                .spacing(8),
            ]
            .spacing(8)
            .into(),
        _ => {
            let voice_count_note = if app.control_form.filter_voices_by_lang {
                format!(
                    "Showing {} of {} voices \u{00b7} {}",
                    visible.len(),
                    KOKORO_VOICES.len(),
                    language_label(&app.control_form.lang),
                )
            } else {
                format!(
                    "{} voices available \u{2014} tick the box to narrow by language.",
                    KOKORO_VOICES.len()
                )
            };
            column![
                    pick_list(voice_labels, Some(current_voice), |label| {
                        let id = app
                            .control_form
                            .visible_voices()
                            .iter()
                            .find(|v| v.label == label)
                            .map(|v| v.id.to_string())
                            .or_else(|| {
                                KOKORO_VOICES
                                    .iter()
                                    .find(|v| v.label == label)
                                    .map(|v| v.id.to_string())
                            })
                            .unwrap_or(label);
                        Message::VoiceSelected(id)
                    }),
                    row![
                        pick_list(lang_labels, Some(current_lang), |label| {
                            let id = LANGUAGES
                                .iter()
                                .find(|l| l.label == label)
                                .map(|l| l.id.to_string())
                                .unwrap_or(label);
                            Message::LangSelected(id)
                        })
                        .width(Length::Fill),
                        checkbox(
                            "Filter by language",
                            app.control_form.filter_voices_by_lang
                        )
                        .on_toggle(Message::FilterVoicesToggled),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                    hint(voice_count_note),
                    text(format!("Speed: {speed:.2}\u{00d7}")),
                    slider(
                        50..=200,
                        app.control_form.speed_slider,
                        Message::SpeedChanged
                    ),
                    hint(speed_hint_for_value(speed)),
                ]
                .spacing(8)
                .into()
        }
    };

    container(
        column![
            column![
                text("Lexaloud").size(20),
                text("Local text-to-speech \u{2014} control panel")
                    .size(12)
                    .style(iced::widget::text::secondary),
            ]
            .spacing(2),
            tabs,
            container(scrollable(tab_body).height(Length::Fill))
                .padding(14)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(panel_style),
            row![
                text("Test speak:"),
                button("Speak test sentence").on_press(Message::TestSpeak),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            hint(app.control_form.status.clone()),
            row![
                horizontal_space(),
                button("Apply settings")
                    .style(button::primary)
                    .on_press(Message::ApplySettings),
                button("Close").on_press(Message::CloseControl),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .padding(16),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn view_overlay(app: &App) -> Element<'_, Message> {
    let label = if app.playback.state == "speaking" || app.playback.state == "paused" {
        if app.playback.current_sentence.is_empty() {
            "Preparing\u{2026}".to_string()
        } else {
            app.playback.current_sentence.clone()
        }
    } else {
        String::new()
    };
    let pause_label = if app.playback.state == "paused" {
        "\u{23f5}"
    } else {
        "\u{23f8}"
    };
    container(
        row![
            text(label).width(Length::Fill),
            button(pause_label).on_press(Message::OverlayPause),
            button("\u{23ed}").on_press(Message::OverlaySkip),
            button("\u{23f9}").on_press(Message::OverlayStop),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .padding(12),
    )
    .style(|_theme| container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            0.1, 0.1, 0.1, 0.85,
        ))),
        border: iced::Border {
            radius: 16.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn view_onboarding(app: &App) -> Element<'_, Message> {
    container(
        column![
            text("Welcome to Lexaloud").size(20),
            text(app.download_progress.status.clone()),
            progress_bar(0.0..=100.0, app.download_progress.percent as f32),
            text(app.download_progress.filename.clone()).size(12),
            row![
                horizontal_space(),
                button("Skip").on_press(Message::OnboardingSkip),
                button("Continue").on_press(Message::OnboardingContinue),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .padding(16)
        .align_x(Alignment::Start),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn view_warning(app: &App) -> Element<'_, Message> {
    let msg = app
        .warning_text
        .clone()
        .unwrap_or_else(|| "System tray not available on this desktop.".into());
    container(
        column![
            text("Lexaloud").size(18),
            text(msg),
            button("OK").on_press(Message::CloseWarning),
        ]
        .spacing(12)
        .padding(16),
    )
    .into()
}

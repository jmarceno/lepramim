//! Qt Quick bridge: exposes app state and actions to QML.

use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QString, QStringList};

use crate::config::Config;
use crate::models;
use crate::single_instance::SingleInstanceEvent;
use crate::ui::capture::{self, toggle_playback};
use crate::ui::client::{self, ApiResult};
use crate::ui::hotkeys::{HotkeyEvent, HotkeyManager};
use crate::ui::tray_service::{TrayEvent, TrayHandle, TraySharedState};
use crate::ui::tray_state::{TrayIconPhase, tray_icon_phase, tray_state_for_daemon};
use crate::ui::voices::{
    ControlForm, LANGUAGES, speed_from_slider, speed_to_slider, voice_short_label,
};

#[derive(Debug, Clone)]
struct DownloadProgress {
    filename: String,
    percent: u8,
    status: String,
}

fn is_download_terminal(p: &DownloadProgress) -> bool {
    p.filename.is_empty()
}

pub struct UiBootstrap {
    pub tray: TrayHandle,
    pub hotkeys: HotkeyManager,
    pub daemon_child: Arc<Mutex<Option<std::process::Child>>>,
    pub single_rx: Option<crossbeam_channel::Receiver<SingleInstanceEvent>>,
    pub show_control: bool,
    pub force_overlay: bool,
}

static BOOTSTRAP: OnceLock<Mutex<Option<UiBootstrap>>> = OnceLock::new();

pub fn install_bootstrap(bootstrap: UiBootstrap) {
    let cell = BOOTSTRAP.get_or_init(|| Mutex::new(None));
    *cell.lock().expect("bootstrap lock") = Some(bootstrap);
}

fn take_bootstrap() -> Option<UiBootstrap> {
    BOOTSTRAP
        .get()
        .and_then(|cell| cell.lock().ok().and_then(|mut g| g.take()))
}

/// True while QML has not yet consumed the bootstrap payload.
///
/// After a successful startup this flips to false within a second or two.
/// The startup watchdog treats "still pending" as a QML load failure.
pub(crate) fn is_bootstrap_pending() -> bool {
    match BOOTSTRAP.get() {
        None => true,
        Some(cell) => cell.lock().map(|g| g.is_some()).unwrap_or(false),
    }
}

struct Runtime {
    tray: TrayHandle,
    hotkeys: HotkeyManager,
    daemon_child: Arc<Mutex<Option<std::process::Child>>>,
    single_rx: Option<crossbeam_channel::Receiver<SingleInstanceEvent>>,
    force_overlay: bool,
    base_config: Config,
    form: ControlForm,
    preparing_speech: bool,
    download_rx: Option<crossbeam_channel::Receiver<DownloadProgress>>,
    warn_rx: Option<crossbeam_channel::Receiver<String>>,
    tray_state: TraySharedState,
    daemon_spawned: bool,
}

#[cxx_qt::bridge]
mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, control_visible)]
        #[qproperty(bool, overlay_visible)]
        #[qproperty(bool, onboarding_visible)]
        #[qproperty(bool, warning_visible)]
        #[qproperty(bool, quit_requested)]
        #[qproperty(bool, engine_running)]
        #[qproperty(bool, fast_polling)]
        #[qproperty(bool, download_active)]
        #[qproperty(bool, playback_active)]
        #[qproperty(bool, playback_paused)]
        #[qproperty(bool, filter_voices_by_lang)]
        #[qproperty(bool, overlay_enabled)]
        #[qproperty(bool, dedupe_mathjax)]
        #[qproperty(bool, strip_markdown)]
        #[qproperty(bool, strip_numeric_citations)]
        #[qproperty(bool, expand_latin)]
        #[qproperty(bool, normalize_numbers)]
        #[qproperty(bool, strip_parenthetical_citations)]
        #[qproperty(bool, expand_academic)]
        #[qproperty(bool, normalize_urls)]
        #[qproperty(bool, normalize_math_symbols)]
        #[qproperty(bool, pdf_cleanup)]
        #[qproperty(bool, sre_latex_enabled)]
        #[qproperty(i32, control_tab)]
        #[qproperty(i32, voice_index)]
        #[qproperty(i32, language_index)]
        #[qproperty(i32, download_percent)]
        #[qproperty(f64, speed)]
        #[qproperty(QString, page_title)]
        #[qproperty(QString, page_subtitle)]
        #[qproperty(QString, status_message)]
        #[qproperty(QString, playback_status_label)]
        #[qproperty(QString, playback_status_detail)]
        #[qproperty(QString, acceleration_label)]
        #[qproperty(QString, current_sentence)]
        #[qproperty(QString, models_status)]
        #[qproperty(QString, download_status)]
        #[qproperty(QString, download_filename)]
        #[qproperty(QString, warning_text)]
        #[qproperty(QStringList, voice_labels)]
        #[qproperty(QStringList, language_labels)]
        type AppController = super::AppControllerRust;

        #[qinvokable]
        fn bootstrap(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "pollInput"]
        fn poll_input(self: Pin<&mut Self>);

        #[qinvokable]
        fn tick(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "selectTab"]
        fn set_control_tab_inv(self: Pin<&mut Self>, tab: i32);

        #[qinvokable]
        #[cxx_name = "selectVoiceAt"]
        fn select_voice_at(self: Pin<&mut Self>, index: i32);

        #[qinvokable]
        #[cxx_name = "selectLanguageAt"]
        fn select_language_at(self: Pin<&mut Self>, index: i32);

        #[qinvokable]
        #[cxx_name = "applyFilterVoicesByLang"]
        fn set_filter_voices_by_lang_inv(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        #[cxx_name = "applySpeed"]
        fn set_speed_inv(self: Pin<&mut Self>, speed: f64);

        #[qinvokable]
        #[cxx_name = "applyOverlayEnabled"]
        fn set_overlay_enabled_inv(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        #[cxx_name = "applyDedupeMathjax"]
        fn set_dedupe_mathjax_inv(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        #[cxx_name = "applyStripMarkdown"]
        fn set_strip_markdown_inv(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        #[cxx_name = "applyStripNumericCitations"]
        fn set_strip_numeric_citations_inv(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        #[cxx_name = "applyExpandLatin"]
        fn set_expand_latin_inv(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        #[cxx_name = "applyNormalizeNumbers"]
        fn set_normalize_numbers_inv(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        #[cxx_name = "applyStripParentheticalCitations"]
        fn set_strip_parenthetical_citations_inv(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        #[cxx_name = "applyExpandAcademic"]
        fn set_expand_academic_inv(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        #[cxx_name = "applyNormalizeUrls"]
        fn set_normalize_urls_inv(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        #[cxx_name = "applyNormalizeMathSymbols"]
        fn set_normalize_math_symbols_inv(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        #[cxx_name = "applyPdfCleanup"]
        fn set_pdf_cleanup_inv(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        #[cxx_name = "applySreLatexEnabled"]
        fn set_sre_latex_enabled_inv(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        #[cxx_name = "applySettings"]
        fn apply_settings(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "testVoice"]
        fn test_voice(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "readSelection"]
        fn read_selection(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "hideControl"]
        fn hide_control(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "refreshModels"]
        fn refresh_models(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "startDownload"]
        fn start_download(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "skipOnboarding"]
        fn skip_onboarding(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "continueOnboarding"]
        fn continue_onboarding(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "dismissWarning"]
        fn dismiss_warning(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "overlayBack"]
        fn overlay_back(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "overlayToggle"]
        fn overlay_toggle(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "overlaySkip"]
        fn overlay_skip(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "overlayStop"]
        fn overlay_stop(self: Pin<&mut Self>);
    }
}

pub struct AppControllerRust {
    control_visible: bool,
    overlay_visible: bool,
    onboarding_visible: bool,
    warning_visible: bool,
    quit_requested: bool,
    engine_running: bool,
    fast_polling: bool,
    download_active: bool,
    playback_active: bool,
    playback_paused: bool,
    filter_voices_by_lang: bool,
    overlay_enabled: bool,
    dedupe_mathjax: bool,
    strip_markdown: bool,
    strip_numeric_citations: bool,
    expand_latin: bool,
    normalize_numbers: bool,
    strip_parenthetical_citations: bool,
    expand_academic: bool,
    normalize_urls: bool,
    normalize_math_symbols: bool,
    pdf_cleanup: bool,
    sre_latex_enabled: bool,
    control_tab: i32,
    voice_index: i32,
    language_index: i32,
    download_percent: i32,
    speed: f64,
    page_title: QString,
    page_subtitle: QString,
    status_message: QString,
    playback_status_label: QString,
    playback_status_detail: QString,
    acceleration_label: QString,
    current_sentence: QString,
    models_status: QString,
    download_status: QString,
    download_filename: QString,
    warning_text: QString,
    voice_labels: QStringList,
    language_labels: QStringList,
    runtime: Option<Runtime>,
    onboarding_skipped: bool,
    playback_state: String,
}

impl Default for AppControllerRust {
    fn default() -> Self {
        Self {
            control_visible: false,
            overlay_visible: false,
            onboarding_visible: false,
            warning_visible: false,
            quit_requested: false,
            engine_running: false,
            fast_polling: false,
            download_active: false,
            playback_active: false,
            playback_paused: false,
            filter_voices_by_lang: false,
            overlay_enabled: false,
            dedupe_mathjax: true,
            strip_markdown: true,
            strip_numeric_citations: true,
            expand_latin: true,
            normalize_numbers: true,
            strip_parenthetical_citations: false,
            expand_academic: true,
            normalize_urls: true,
            normalize_math_symbols: true,
            pdf_cleanup: true,
            sre_latex_enabled: false,
            control_tab: 0,
            voice_index: 0,
            language_index: 0,
            download_percent: 0,
            speed: 1.0,
            page_title: QString::from("Voice"),
            page_subtitle: QString::from(
                "Choose how Lepramim sounds when reading highlighted text.",
            ),
            status_message: QString::default(),
            playback_status_label: QString::from("Ready"),
            playback_status_detail: QString::from("Waiting for highlighted text"),
            acceleration_label: QString::from("CPU"),
            current_sentence: QString::default(),
            models_status: QString::from("No model status available."),
            download_status: QString::default(),
            download_filename: QString::default(),
            warning_text: QString::default(),
            voice_labels: QStringList::default(),
            language_labels: QStringList::default(),
            runtime: None,
            onboarding_skipped: false,
            playback_state: "idle".into(),
        }
    }
}

fn qstring_list_from_labels(labels: impl IntoIterator<Item = String>) -> QStringList {
    let mut list = QStringList::default();
    for label in labels {
        list.append(QString::from(&*label));
    }
    list
}

impl qobject::AppController {
    fn bootstrap(mut self: Pin<&mut Self>) {
        let Some(boot) = take_bootstrap() else {
            tracing::warn!("bootstrap called with no payload installed; UI stays uninitialized");
            return;
        };
        let base_config = crate::config::load_config(None::<&std::path::Path>);
        let form = ControlForm::load_from_config(&base_config);
        let missing = models::artifacts_missing();
        let overlay_enabled = boot.force_overlay || base_config.advanced.overlay;

        {
            let mut rust = self.as_mut().rust_mut();
            rust.runtime = Some(Runtime {
                tray: boot.tray,
                hotkeys: boot.hotkeys,
                daemon_child: boot.daemon_child,
                single_rx: boot.single_rx,
                force_overlay: boot.force_overlay,
                base_config,
                form: form.clone(),
                preparing_speech: false,
                download_rx: None,
                warn_rx: None,
                tray_state: TraySharedState::default(),
                daemon_spawned: false,
            });
            rust.onboarding_skipped = false;
        }

        self.as_mut().set_control_visible(boot.show_control);
        self.as_mut().set_onboarding_visible(missing);
        tracing::info!(
            show_control = boot.show_control,
            models_missing = missing,
            overlay_enabled,
            "Qt bootstrap complete"
        );
        self.as_mut().set_overlay_enabled(overlay_enabled);
        self.as_mut().sync_form_to_properties();
        self.as_mut().refresh_page_chrome();
        self.as_mut().rebuild_voice_lists();
        self.as_mut().rebuild_language_lists();

        if !missing {
            self.as_mut().spawn_daemon();
        } else {
            self.as_mut()
                .set_download_status(QString::from("Downloading the Kokoro speech model…"));
        }
    }

    fn sync_form_to_properties(mut self: Pin<&mut Self>) {
        let form = self
            .as_mut()
            .rust_mut()
            .runtime
            .as_ref()
            .map(|r| r.form.clone())
            .unwrap_or_default();
        self.as_mut()
            .set_filter_voices_by_lang(form.filter_voices_by_lang);
        self.as_mut().set_overlay_enabled(form.overlay);
        self.as_mut().set_dedupe_mathjax(form.dedupe_mathjax);
        self.as_mut().set_strip_markdown(form.strip_markdown);
        self.as_mut()
            .set_strip_numeric_citations(form.strip_numeric_citations);
        self.as_mut().set_expand_latin(form.expand_latin);
        self.as_mut().set_normalize_numbers(form.normalize_numbers);
        self.as_mut()
            .set_strip_parenthetical_citations(form.strip_parenthetical_citations);
        self.as_mut().set_expand_academic(form.expand_academic);
        self.as_mut().set_normalize_urls(form.normalize_urls);
        self.as_mut()
            .set_normalize_math_symbols(form.normalize_math_symbols);
        self.as_mut().set_pdf_cleanup(form.pdf_cleanup);
        self.as_mut().set_sre_latex_enabled(form.sre_latex_enabled);
        self.as_mut()
            .set_speed(speed_from_slider(form.speed_slider));
        self.as_mut()
            .set_status_message(QString::from(&*form.status));
    }

    fn form_from_properties(&self) -> ControlForm {
        let mut form = self
            .rust()
            .runtime
            .as_ref()
            .map(|r| r.form.clone())
            .unwrap_or_default();
        form.filter_voices_by_lang = *self.filter_voices_by_lang();
        form.overlay = *self.overlay_enabled();
        form.dedupe_mathjax = *self.dedupe_mathjax();
        form.strip_markdown = *self.strip_markdown();
        form.strip_numeric_citations = *self.strip_numeric_citations();
        form.expand_latin = *self.expand_latin();
        form.normalize_numbers = *self.normalize_numbers();
        form.strip_parenthetical_citations = *self.strip_parenthetical_citations();
        form.expand_academic = *self.expand_academic();
        form.normalize_urls = *self.normalize_urls();
        form.normalize_math_symbols = *self.normalize_math_symbols();
        form.pdf_cleanup = *self.pdf_cleanup();
        form.sre_latex_enabled = *self.sre_latex_enabled();
        form.speed_slider = speed_to_slider(*self.speed());
        form
    }

    fn refresh_page_chrome(mut self: Pin<&mut Self>) {
        let tab = *self.control_tab();
        let (title, subtitle) = match tab {
            1 => (
                "Preprocessor",
                "Clean highlighted text before it is spoken.",
            ),
            2 => ("Advanced", "Extra cleanup options and Speech Rule Engine."),
            3 => ("Models", "Speech model files on this machine."),
            _ => (
                "Voice",
                "Choose how Lepramim sounds when reading highlighted text.",
            ),
        };
        self.as_mut().set_page_title(QString::from(title));
        self.as_mut().set_page_subtitle(QString::from(subtitle));
    }

    fn rebuild_language_lists(mut self: Pin<&mut Self>) {
        let labels = LANGUAGES.iter().map(|l| l.label.to_string());
        let list = qstring_list_from_labels(labels);
        let lang = self
            .rust()
            .runtime
            .as_ref()
            .map(|r| r.form.lang.clone())
            .unwrap_or_else(|| "en-us".into());
        let index = LANGUAGES.iter().position(|l| l.id == lang).unwrap_or(0) as i32;
        self.as_mut().set_language_labels(list);
        self.as_mut().set_language_index(index);
    }

    fn rebuild_voice_lists(mut self: Pin<&mut Self>) {
        let form = self
            .rust()
            .runtime
            .as_ref()
            .map(|r| r.form.clone())
            .unwrap_or_default();
        let voices = form.visible_voices();
        let labels = voices
            .iter()
            .map(|v| voice_short_label(v.label).to_string());
        let list = qstring_list_from_labels(labels);
        let index = voices.iter().position(|v| v.id == form.voice).unwrap_or(0) as i32;
        self.as_mut().set_voice_labels(list);
        self.as_mut().set_voice_index(index);
    }

    fn persist_form(mut self: Pin<&mut Self>) {
        let form = self.form_from_properties();
        if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
            rt.form = form;
        }
    }

    fn spawn_daemon(mut self: Pin<&mut Self>) {
        let Some(child_slot) = self.rust().runtime.as_ref().map(|r| r.daemon_child.clone()) else {
            return;
        };
        if self
            .rust()
            .runtime
            .as_ref()
            .map(|r| r.daemon_spawned)
            .unwrap_or(false)
        {
            return;
        }
        if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
            rt.daemon_spawned = true;
        }
        let (warn_tx, warn_rx) = crossbeam_channel::unbounded();
        if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
            rt.warn_rx = Some(warn_rx);
        }
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("tokio");
            let result = rt.block_on(spawn_daemon_async(child_slot));
            if let Err(e) = result {
                tracing::error!("daemon spawn failed: {e}");
                let _ = warn_tx.send(e);
            }
        });
        // Load config shortly after spawn attempt
        std::thread::sleep(Duration::from_millis(400));
        self.as_mut().reload_config_from_daemon();
    }

    fn reload_config_from_daemon(mut self: Pin<&mut Self>) {
        let r = client::get_config();
        if !r.is_success() {
            return;
        }
        if let Ok(cfg) = serde_json::from_value::<Config>(r.json) {
            let form = ControlForm::load_from_config(&cfg);
            if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
                rt.base_config = cfg.clone();
                rt.form = form;
            }
            self.as_mut().set_overlay_enabled(cfg.advanced.overlay);
            self.as_mut().sync_form_to_properties();
            self.as_mut().rebuild_voice_lists();
            self.as_mut().rebuild_language_lists();
        }
    }

    fn poll_input(mut self: Pin<&mut Self>) {
        let events: Vec<TrayEvent> = {
            let mut rust = self.as_mut().rust_mut();
            let Some(rt) = rust.runtime.as_mut() else {
                return;
            };
            let mut out = Vec::new();
            while let Ok(ev) = rt.tray.rx.try_recv() {
                out.push(ev);
            }
            out
        };
        for ev in events {
            self.as_mut().handle_tray(ev);
            if *self.quit_requested() {
                // Qt.quit() (via QML) does not reliably stop this app's event
                // loop, so exit from Rust: stop the daemon gracefully, then
                // terminate. D-Bus names auto-release; the stale app.sock
                // self-heals on next launch.
                let child = self.rust().runtime.as_ref().map(|r| r.daemon_child.clone());
                if let Some(child) = child {
                    quit_via_rust_shutdown(child);
                } else {
                    std::process::exit(0);
                }
                return;
            }
        }

        let hotkeys: Vec<HotkeyEvent> = {
            let Some(rt) = self.rust().runtime.as_ref() else {
                return;
            };
            let mut out = Vec::new();
            while let Ok(ev) = rt.hotkeys.receiver().try_recv() {
                out.push(ev);
            }
            out
        };
        for ev in hotkeys {
            match ev {
                HotkeyEvent::SpeakSelection => {
                    std::thread::sleep(Duration::from_millis(250));
                    self.as_mut().speak_captured();
                }
                HotkeyEvent::Toggle => toggle_playback(),
            }
        }

        let show_control = {
            let Some(rt) = self.rust().runtime.as_ref() else {
                return;
            };
            let Some(rx) = rt.single_rx.as_ref() else {
                return;
            };
            let mut show = false;
            while let Ok(ev) = rx.try_recv() {
                match ev {
                    SingleInstanceEvent::ShowControl => show = true,
                }
            }
            show
        };
        if show_control {
            self.as_mut().set_control_visible(true);
        }
    }

    fn tick(mut self: Pin<&mut Self>) {
        // Spawn / daemon warnings (e.g. daemon failed to become ready).
        // Surfaced in the warning window like the old desktop shell did.
        let mut warnings: Vec<String> = Vec::new();
        {
            let mut rust = self.as_mut().rust_mut();
            let Some(rt) = rust.runtime.as_mut() else {
                return;
            };
            if let Some(rx) = rt.warn_rx.as_ref() {
                while let Ok(w) = rx.try_recv() {
                    warnings.push(w);
                }
            }
        }
        for w in warnings {
            self.as_mut().set_warning_text(QString::from(&*w));
            self.as_mut().set_warning_visible(true);
        }
        // Download progress
        let mut updates: Vec<DownloadProgress> = Vec::new();
        let mut terminal: Option<DownloadProgress> = None;
        {
            let mut rust = self.as_mut().rust_mut();
            let Some(rt) = rust.runtime.as_mut() else {
                return;
            };
            while let Some(rx) = rt.download_rx.as_ref() {
                match rx.try_recv() {
                    Ok(p) => {
                        if is_download_terminal(&p) {
                            terminal = Some(p);
                            rt.download_rx = None;
                            break;
                        }
                        updates.push(p);
                    }
                    Err(crossbeam_channel::TryRecvError::Empty) => break,
                    Err(crossbeam_channel::TryRecvError::Disconnected) => {
                        rt.download_rx = None;
                        break;
                    }
                }
            }
        }
        for p in updates {
            self.as_mut()
                .set_download_filename(QString::from(&*p.filename));
            self.as_mut().set_download_percent(i32::from(p.percent));
            self.as_mut().set_download_status(QString::from(&*p.status));
        }
        if let Some(p) = terminal {
            self.as_mut().set_download_active(false);
            self.as_mut().set_download_percent(100);
            self.as_mut().set_download_status(QString::from(&*p.status));
            if !p.status.starts_with("Download failed") {
                if *self.onboarding_visible() && !self.rust().onboarding_skipped {
                    self.as_mut().set_onboarding_visible(false);
                    self.as_mut().spawn_daemon();
                } else if !models::artifacts_missing() {
                    let need = self
                        .rust()
                        .runtime
                        .as_ref()
                        .and_then(|r| r.daemon_child.lock().ok())
                        .map(|g| g.is_none())
                        .unwrap_or(true);
                    if need {
                        if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
                            rt.daemon_spawned = false;
                        }
                        self.as_mut().spawn_daemon();
                    }
                }
            }
        }

        self.as_mut().poll_daemon_state();
    }

    fn poll_daemon_state(mut self: Pin<&mut Self>) {
        let health = client::get_healthz();
        let active = health.is_success();
        self.as_mut().set_engine_running(active);
        let state_result = if active {
            client::get_state()
        } else {
            ApiResult {
                status_code: 0,
                json: serde_json::json!({ "state": "idle" }),
                error: String::new(),
                raw_body: Vec::new(),
            }
        };
        let state_str = state_result
            .json
            .get("state")
            .and_then(|v| v.as_str())
            .unwrap_or("idle")
            .to_string();
        let sentence = state_result
            .json
            .get("current_sentence")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let providers: Vec<String> = state_result
            .json
            .get("session_providers")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();

        let preparing = self
            .rust()
            .runtime
            .as_ref()
            .map(|r| r.preparing_speech)
            .unwrap_or(false);
        let force_overlay = self
            .rust()
            .runtime
            .as_ref()
            .map(|r| r.force_overlay)
            .unwrap_or(false);
        let overlay_enabled = *self.overlay_enabled() || force_overlay;

        if state_str == "speaking" {
            if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
                rt.preparing_speech = false;
            }
        }

        self.as_mut().rust_mut().playback_state = state_str.clone();
        self.as_mut()
            .set_current_sentence(QString::from(&*sentence));
        self.as_mut().set_playback_paused(state_str == "paused");
        self.as_mut()
            .set_playback_active(state_str == "speaking" || state_str == "paused" || preparing);
        let (label, detail): (String, String) = match state_str.as_str() {
            "speaking" => ("Speaking".into(), sentence.clone()),
            "paused" => ("Paused".into(), sentence.clone()),
            _ if preparing => ("Preparing".into(), "Warming up speech…".into()),
            _ => ("Ready".into(), "Waiting for highlighted text".into()),
        };
        self.as_mut()
            .set_playback_status_label(QString::from(&*label));
        self.as_mut()
            .set_playback_status_detail(QString::from(if detail.is_empty() {
                "Waiting for highlighted text"
            } else {
                &*detail
            }));

        let accel = if providers.iter().any(|p| p.contains("CUDA")) {
            "CUDA"
        } else {
            "CPU"
        };
        self.as_mut().set_acceleration_label(QString::from(accel));

        let fast = preparing || state_str == "speaking" || state_str == "paused";
        self.as_mut().set_fast_polling(fast);

        let should_show_overlay =
            overlay_enabled && (state_str == "speaking" || state_str == "paused");
        self.as_mut().set_overlay_visible(should_show_overlay);

        // Tray update
        let cpu_fallback = !providers.is_empty() && !providers.iter().any(|p| p.contains("CUDA"));
        let s = tray_state_for_daemon(&state_str, active, cpu_fallback);
        let icon_phase = if s.icon_running {
            tray_icon_phase(&state_str, preparing, &sentence)
        } else {
            TrayIconPhase::Idle
        };
        if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
            let breath_mix = if icon_phase == TrayIconPhase::Preparing {
                rt.tray_state.breath_mix
            } else {
                0.0
            };
            rt.tray_state = TraySharedState {
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
            rt.tray.update(&rt.tray_state);
        }
    }

    fn handle_tray(mut self: Pin<&mut Self>, ev: TrayEvent) {
        match ev {
            TrayEvent::ToggleDaemon => {
                if client::get_healthz().is_success() {
                    let _ = client::post_shutdown();
                    if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
                        if let Ok(mut guard) = rt.daemon_child.lock() {
                            if let Some(ref mut c) = *guard {
                                let _ = c.wait();
                            }
                            *guard = None;
                        }
                        rt.daemon_spawned = false;
                    }
                    self.as_mut().set_engine_running(false);
                } else if !models::artifacts_missing() {
                    if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
                        rt.daemon_spawned = false;
                    }
                    self.as_mut().spawn_daemon();
                } else {
                    self.as_mut().set_onboarding_visible(true);
                }
            }
            TrayEvent::SpeakSelection => {
                std::thread::sleep(Duration::from_millis(300));
                self.as_mut().speak_captured();
            }
            TrayEvent::PauseResume => toggle_playback(),
            TrayEvent::StopPlayback => {
                let _ = client::post_stop();
            }
            TrayEvent::OpenControl => self.as_mut().set_control_visible(true),
            TrayEvent::ToggleAutostart => {
                let path = crate::platform::service::autostart_path();
                if path.is_file() {
                    let _ = crate::platform::service::remove_autostart();
                } else {
                    let _ = crate::platform::service::write_autostart(
                        &crate::platform::service::resolve_binary_path(),
                    );
                }
                if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
                    rt.tray_state.autostart_checked =
                        crate::platform::service::autostart_path().is_file();
                    rt.tray.update(&rt.tray_state);
                }
            }
            TrayEvent::Quit => {
                self.as_mut().set_quit_requested(true);
            }
        }
    }

    fn speak_captured(mut self: Pin<&mut Self>) {
        let cap = capture::capture_highlighted_text();
        if cap.text.is_empty() {
            capture::notify_empty_selection();
            return;
        }
        if cap.truncated {
            capture::notify_truncated_selection();
        }
        if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
            rt.preparing_speech = true;
        }
        let r = client::post_speak(&cap.text, "replace");
        if !r.is_success() || r.is_daemon_down() {
            if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
                rt.preparing_speech = false;
            }
            capture::notify_speak_result(&r);
        }
    }

    fn set_control_tab_inv(mut self: Pin<&mut Self>, tab: i32) {
        self.as_mut().set_control_tab(tab.clamp(0, 3));
        self.as_mut().refresh_page_chrome();
        if tab == 3 {
            self.as_mut().refresh_models();
        }
    }

    fn select_voice_at(mut self: Pin<&mut Self>, index: i32) {
        let form = self
            .rust()
            .runtime
            .as_ref()
            .map(|r| r.form.clone())
            .unwrap_or_default();
        let voices = form.visible_voices();
        if let Some(v) = voices.get(index as usize) {
            if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
                rt.form.voice = v.id.to_string();
                rt.form.unknown_voice_note = false;
            }
            self.as_mut().set_voice_index(index);
        }
    }

    fn select_language_at(mut self: Pin<&mut Self>, index: i32) {
        if let Some(lang) = LANGUAGES.get(index as usize) {
            if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
                rt.form.lang = lang.id.to_string();
                rt.form.ensure_voice_matches_filter();
            }
            self.as_mut().set_language_index(index);
            self.as_mut().rebuild_voice_lists();
        }
    }

    fn set_filter_voices_by_lang_inv(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_filter_voices_by_lang(enabled);
        if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
            rt.form.filter_voices_by_lang = enabled;
            rt.form.ensure_voice_matches_filter();
        }
        self.as_mut().rebuild_voice_lists();
    }

    fn set_speed_inv(mut self: Pin<&mut Self>, speed: f64) {
        self.as_mut().set_speed(speed.clamp(0.5, 2.0));
        if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
            rt.form.speed_slider = speed_to_slider(speed);
        }
    }

    fn set_overlay_enabled_inv(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_overlay_enabled(enabled);
        if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
            rt.form.overlay = enabled;
        }
    }

    fn set_dedupe_mathjax_inv(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_dedupe_mathjax(enabled);
        self.as_mut().persist_form();
    }
    fn set_strip_markdown_inv(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_strip_markdown(enabled);
        self.as_mut().persist_form();
    }
    fn set_strip_numeric_citations_inv(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_strip_numeric_citations(enabled);
        self.as_mut().persist_form();
    }
    fn set_expand_latin_inv(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_expand_latin(enabled);
        self.as_mut().persist_form();
    }
    fn set_normalize_numbers_inv(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_normalize_numbers(enabled);
        self.as_mut().persist_form();
    }
    fn set_strip_parenthetical_citations_inv(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_strip_parenthetical_citations(enabled);
        self.as_mut().persist_form();
    }
    fn set_expand_academic_inv(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_expand_academic(enabled);
        self.as_mut().persist_form();
    }
    fn set_normalize_urls_inv(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_normalize_urls(enabled);
        self.as_mut().persist_form();
    }
    fn set_normalize_math_symbols_inv(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_normalize_math_symbols(enabled);
        self.as_mut().persist_form();
    }
    fn set_pdf_cleanup_inv(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_pdf_cleanup(enabled);
        self.as_mut().persist_form();
    }
    fn set_sre_latex_enabled_inv(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_sre_latex_enabled(enabled);
        self.as_mut().persist_form();
    }

    fn apply_settings(mut self: Pin<&mut Self>) {
        self.as_mut().persist_form();
        let (merged, voice, lang, speed) = {
            let Some(rt) = self.rust().runtime.as_ref() else {
                return;
            };
            let merged = rt.form.merge_into_config(&rt.base_config);
            (
                merged,
                rt.form.voice.clone(),
                rt.form.lang.clone(),
                speed_from_slider(rt.form.speed_slider),
            )
        };
        let r = client::post_config(&merged);
        if r.is_success() {
            self.as_mut().set_status_message(QString::from(&*format!(
                "Saved voice={voice}, lang={lang}, speed={speed:.2}×; it applies on the next playback start."
            )));
            if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
                rt.base_config = merged;
            }
            self.as_mut().reload_config_from_daemon();
        } else {
            self.as_mut().set_status_message(QString::from(&*format!(
                "Saving config failed: {}",
                r.error
            )));
        }
    }

    fn test_voice(mut self: Pin<&mut Self>) {
        if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
            rt.preparing_speech = true;
        }
        let r = client::post_speak("The quick brown fox jumps over the lazy dog.", "replace");
        if r.is_success() {
            self.as_mut()
                .set_status_message(QString::from("Test speak sent."));
        } else {
            if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
                rt.preparing_speech = false;
            }
            self.as_mut()
                .set_status_message(QString::from(&*format!("Test speak failed: {}", r.error)));
            capture::notify_speak_result(&r);
        }
    }

    fn read_selection(mut self: Pin<&mut Self>) {
        self.as_mut().speak_captured();
    }

    fn hide_control(mut self: Pin<&mut Self>) {
        self.as_mut().set_control_visible(false);
    }

    fn refresh_models(mut self: Pin<&mut Self>) {
        let r = client::get_models_status();
        if r.is_success() {
            self.as_mut()
                .set_models_status(QString::from(&*format_models_status(&r.json)));
        }
    }

    fn start_download(mut self: Pin<&mut Self>) {
        if *self.download_active() {
            return;
        }
        self.as_mut().set_download_active(true);
        let (tx, rx) = crossbeam_channel::unbounded();
        if let Some(rt) = self.as_mut().rust_mut().runtime.as_mut() {
            rt.download_rx = Some(rx);
        }
        std::thread::spawn(move || {
            let cb = |filename: &str, percent: u8| {
                let _ = tx.send(DownloadProgress {
                    filename: filename.to_string(),
                    percent,
                    status: format!("Downloading {filename}…"),
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
    }

    fn skip_onboarding(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().onboarding_skipped = true;
        self.as_mut().set_onboarding_visible(false);
        if !*self.download_active() && models::artifacts_missing() {
            self.as_mut().start_download();
        }
    }

    fn continue_onboarding(mut self: Pin<&mut Self>) {
        if models::artifacts_missing() && !*self.download_active() {
            self.as_mut().start_download();
            return;
        }
        if !models::artifacts_missing() {
            self.as_mut().set_onboarding_visible(false);
            self.as_mut().spawn_daemon();
        }
    }

    fn dismiss_warning(mut self: Pin<&mut Self>) {
        self.as_mut().set_warning_visible(false);
        self.as_mut().set_warning_text(QString::default());
    }

    fn overlay_back(self: Pin<&mut Self>) {
        let _ = client::post_back();
    }
    fn overlay_toggle(self: Pin<&mut Self>) {
        let _ = client::post_toggle();
    }
    fn overlay_skip(self: Pin<&mut Self>) {
        let _ = client::post_skip();
    }
    fn overlay_stop(self: Pin<&mut Self>) {
        let _ = client::post_stop();
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

/// Graceful quit initiated from Rust (tray Quit menu).
///
/// QML `Qt.quit()` does not reliably stop this app's Qt event loop, so the
/// quit path must not depend on it: stop the daemon, reap the child, then
/// terminate the process. Runs on a worker thread; fires once.
fn quit_via_rust_shutdown(daemon_child: Arc<Mutex<Option<std::process::Child>>>) {
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    tracing::info!("quit requested; stopping daemon and exiting");
    std::thread::Builder::new()
        .name("lepramim-quit".into())
        .spawn(move || {
            let _ = client::post_shutdown();
            let rt = tokio::runtime::Runtime::new().expect("tokio");
            rt.block_on(async {
                for _ in 0..25 {
                    if crate::api::uds_get("/healthz").await.is_err() {
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
            std::process::exit(0);
        })
        .ok();
}

pub async fn spawn_daemon_async(
    daemon_child: Arc<Mutex<Option<std::process::Child>>>,
) -> Result<(), String> {
    if crate::api::uds_get("/healthz").await.is_ok() {
        return Ok(());
    }
    let bin =
        std::env::current_exe().unwrap_or_else(|_| crate::platform::service::resolve_binary_path());
    let rt = crate::config::runtime_dir().join("lepramim");
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
        if crate::api::uds_get("/healthz").await.is_ok() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_terminal_requires_empty_filename() {
        assert!(is_download_terminal(&DownloadProgress {
            filename: String::new(),
            percent: 100,
            status: "done".into(),
        }));
        assert!(!is_download_terminal(&DownloadProgress {
            filename: "a.onnx".into(),
            percent: 100,
            status: "Downloading".into(),
        }));
    }
}

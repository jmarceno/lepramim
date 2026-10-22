//! A tiny always-on-top Wayland overlay.
//!
//! Ordinary toplevel windows cannot choose where a Wayland compositor puts
//! them, which is why the Qt overlay window ended up wherever the compositor
//! felt like placing it. The `zwlr_layer_shell_v1` protocol is the supported
//! way to ask for a surface anchored to an edge of the screen, above normal
//! windows. This module creates such a surface (bottom edge, centered,
//! click-through) and paints a small pill into a `wl_shm` buffer with
//! `usvg`/`resvg`, which are already dependencies of the app.
//!
//! The overlay runs on its own thread and its own Wayland connection; the Qt
//! GUI thread only pushes state updates through a channel. States:
//!
//! * `Processing` — a phrase is being synthesized ("Processing" + "Preparing
//!   audio…"), pulsing amber dot.
//! * `Speaking` — the phrase is being read, the sentence marquee-scrolls
//!   inside the pill.
//! * `Paused` — playback paused, amber/green dot muted.
//!
//! The compositor window is click-through (empty input region) and never
//! takes keyboard focus, so it cannot get in the user's way.

use std::fmt::Write as _;
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState, Region};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
    LayerSurfaceConfigure,
};
use smithay_client_toolkit::shm::{Shm, ShmHandler, slot::SlotPool};
use smithay_client_toolkit::{
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    registry_handlers,
};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_output, wl_registry, wl_shm, wl_surface};
use wayland_client::{Connection, Dispatch, QueueHandle};

/// Logical pill size. Keep this small: the overlay only signals activity.
const WIDTH: u32 = 360;
const HEIGHT: u32 = 34;
/// Distance between the pill and the bottom edge of the output.
const BOTTOM_MARGIN: i32 = 84;
/// Where the scrolling sentence starts (after the pulsing dot + status text).
const PHRASE_X: f32 = 104.0;
/// Width of the clipped sentence viewport.
const PHRASE_CLIP_W: f32 = 242.0;
/// Marquee speed in logical pixels per second.
const MARQUEE_SPEED: f32 = 45.0;
/// Pause at both ends of the marquee before it starts moving/loops.
const MARQUEE_HOLD: Duration = Duration::from_millis(1100);
/// Target frame interval while the overlay is visible (~22 fps).
const FRAME_INTERVAL: Duration = Duration::from_millis(45);
/// Longest sentence fragment rendered; guards pathological input.
const MAX_TEXT_CHARS: usize = 280;

const COLOR_BG: &str = "#1a1d22";
const COLOR_BORDER: &str = "#2a303b";
const COLOR_TEXT: &str = "#c5c9ce";
const COLOR_TEXT_DIM: &str = "#b9bfc8";
const COLOR_GREEN: &str = "#3ecf8e";
const COLOR_AMBER: &str = "#e8b339";
const COLOR_MUTED: &str = "#8b929a";

/// What the overlay is communicating right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Processing,
    Speaking,
    Paused,
}

/// One state push from the Qt GUI thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Update {
    pub visible: bool,
    pub phase: Phase,
    pub sentence: String,
}

impl Default for Update {
    fn default() -> Self {
        Self {
            visible: false,
            phase: Phase::Speaking,
            sentence: String::new(),
        }
    }
}

static SENDER: OnceLock<Mutex<Option<Sender<Update>>>> = OnceLock::new();
static FAILED: AtomicBool = AtomicBool::new(false);
static PROBE: OnceLock<bool> = OnceLock::new();

/// True when this compositor can host the native overlay.
///
/// Probed once: Wayland must be reachable and the layer-shell protocol
/// advertised. Set `LEPRAMIM_OVERLAY_FORCE_QML=1` to force the Qt fallback.
pub fn probe() -> bool {
    *PROBE.get_or_init(|| {
        if std::env::var_os("WAYLAND_DISPLAY").is_none() {
            return false;
        }
        if std::env::var_os("LEPRAMIM_OVERLAY_FORCE_QML").is_some() {
            return false;
        }
        probe_inner()
    })
}

fn probe_inner() -> bool {
    let Ok(conn) = Connection::connect_to_env() else {
        return false;
    };
    let Ok((globals, _queue)) = registry_queue_init::<Probe>(&conn) else {
        return false;
    };
    let list = globals.contents().clone_list();
    let has = |name: &str| list.iter().any(|g| g.interface == name);
    let ok = has("zwlr_layer_shell_v1") && has("wl_compositor") && has("wl_shm");
    if !ok {
        tracing::info!("native overlay unavailable: no layer-shell/wl_shm globals");
    }
    ok
}

/// Spawn the overlay thread ahead of the first phrase so the first overlay
/// appears instantly (fonts + Wayland setup happen once).
pub fn prewarm() {
    ensure_started();
}

/// True when the native overlay thread could not be started or died; the
/// controller then falls back to the small Qt window.
pub fn failed() -> bool {
    FAILED.load(Ordering::Relaxed)
}

/// Push a state update to the overlay thread (starting it on first use).
pub fn update(state: Update) {
    ensure_started();
    if let Some(cell) = SENDER.get() {
        if let Ok(guard) = cell.lock() {
            if let Some(tx) = guard.as_ref() {
                if tx.send(state).is_err() {
                    FAILED.store(true, Ordering::Relaxed);
                }
            }
        }
    }
}

fn ensure_started() {
    let cell = SENDER.get_or_init(|| {
        let (tx, rx) = crossbeam_channel::unbounded();
        let spawned = std::thread::Builder::new()
            .name("lepramim-overlay".into())
            .spawn(move || {
                if let Err(err) = run(rx) {
                    tracing::warn!("native overlay stopped: {err}");
                    FAILED.store(true, Ordering::Relaxed);
                }
            })
            .is_ok();
        if !spawned {
            tracing::warn!("native overlay unavailable: thread spawn failed");
            FAILED.store(true, Ordering::Relaxed);
        }
        Mutex::new(spawned.then_some(tx))
    });
    // Touch the cell so a failed spawn is observed early.
    if let Ok(guard) = cell.lock() {
        if guard.is_none() {
            FAILED.store(true, Ordering::Relaxed);
        }
    }
}

/// Minimal registry probe state (the global list is captured for us).
struct Probe;

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Probe {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

#[derive(Clone, Copy)]
enum Marquee {
    HoldStart(Instant),
    Moving,
    HoldEnd(Instant),
}

struct Overlay {
    registry_state: RegistryState,
    output_state: OutputState,
    compositor: CompositorState,
    shm: Shm,
    layer_shell: LayerShell,
    surface: Option<LayerSurface>,
    /// Kept alive for as long as the surface uses it (empty input region).
    #[allow(dead_code)]
    input_region: Option<Region>,
    pool: Option<SlotPool>,
    configured: bool,
    scale: i32,
    state: Update,
    options: usvg::Options<'static>,
    phrase_width: f32,
    marquee_offset: f32,
    marquee: Marquee,
    last_draw: Instant,
    started: Instant,
    dirty: bool,
}

impl Overlay {
    fn apply(&mut self, state: Update) {
        if state == self.state {
            return;
        }
        let text_changed = state.sentence != self.state.sentence || state.phase != self.state.phase;
        self.state = state;
        if text_changed {
            self.phrase_width = measure_phrase(&self.options, &self.phrase_text());
            self.marquee_offset = 0.0;
            self.marquee = Marquee::HoldStart(Instant::now() + MARQUEE_HOLD);
        }
        self.dirty = true;
    }

    fn phrase_text(&self) -> String {
        match self.state.phase {
            Phase::Processing => "Preparing audio…".to_string(),
            Phase::Speaking | Phase::Paused => {
                if self.state.sentence.trim().is_empty() {
                    "…".to_string()
                } else {
                    truncate(&normalize(&self.state.sentence), MAX_TEXT_CHARS)
                }
            }
        }
    }

    fn overflow(&self) -> f32 {
        if self.state.phase == Phase::Speaking {
            (self.phrase_width - PHRASE_CLIP_W).max(0.0)
        } else {
            0.0
        }
    }

    fn advance_marquee(&mut self, now: Instant) {
        let overflow = self.overflow();
        if overflow <= 0.5 {
            self.marquee_offset = 0.0;
            self.marquee = Marquee::HoldStart(now + MARQUEE_HOLD);
            return;
        }
        let dt = now
            .saturating_duration_since(self.last_draw)
            .as_secs_f32()
            .min(0.25);
        match self.marquee {
            Marquee::HoldStart(until) => {
                if now >= until {
                    self.marquee = Marquee::Moving;
                }
            }
            Marquee::Moving => {
                self.marquee_offset += MARQUEE_SPEED * dt;
                if self.marquee_offset >= overflow {
                    self.marquee_offset = overflow;
                    self.marquee = Marquee::HoldEnd(now + MARQUEE_HOLD);
                }
            }
            Marquee::HoldEnd(until) => {
                if now >= until {
                    self.marquee_offset = 0.0;
                    self.marquee = Marquee::HoldStart(now + MARQUEE_HOLD);
                }
            }
        }
    }

    fn ensure_surface(&mut self, qh: &QueueHandle<Self>) {
        if self.surface.is_some() {
            return;
        }
        let surface = self.compositor.create_surface(qh);
        // Overlay layer: stays above normal windows *and* fullscreen apps.
        // The fixed bottom margin keeps it floating above a bottom dock
        // instead of covering the desktop's own chrome.
        let layer = self.layer_shell.create_layer_surface(
            qh,
            surface,
            Layer::Overlay,
            Some("lepramim-overlay"),
            None,
        );
        // Anchoring only to the bottom centers the pill horizontally and
        // keeps it just above any bottom panel.
        layer.set_anchor(Anchor::BOTTOM);
        layer.set_size(WIDTH, HEIGHT);
        layer.set_margin(0, 0, BOTTOM_MARGIN, 0);
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        let _ = layer.set_buffer_scale(self.scale.max(1) as u32);
        if let Ok(region) = Region::new(&self.compositor) {
            layer.set_input_region(Some(region.wl_region()));
            self.input_region = Some(region);
        }
        // Initial commit with no buffer: the compositor replies with a
        // configure before we may map content.
        layer.commit();
        self.surface = Some(layer);
        self.configured = false;
        self.dirty = true;
    }

    fn drop_surface(&mut self) {
        self.surface = None;
        self.input_region = None;
        self.configured = false;
    }

    fn should_draw(&self, now: Instant) -> bool {
        self.state.visible
            && self.surface.is_some()
            && self.configured
            && (self.dirty || now.saturating_duration_since(self.last_draw) >= FRAME_INTERVAL)
    }

    fn poll_timeout_ms(&self) -> i32 {
        if self.state.visible && self.configured {
            20
        } else if self.state.visible {
            50
        } else {
            200
        }
    }

    fn draw(&mut self, now: Instant) {
        let Some(layer) = self.surface.clone() else {
            return;
        };
        if self.pool.is_none() {
            match SlotPool::new((WIDTH * HEIGHT * 4 * 2) as usize, &self.shm) {
                Ok(pool) => self.pool = Some(pool),
                Err(err) => {
                    tracing::warn!("native overlay: cannot create shm pool: {err}");
                    return;
                }
            }
        }

        self.advance_marquee(now);
        let svg = self.build_svg();
        let scale = self.scale.max(1) as u32;
        let buf_w = WIDTH * scale;
        let buf_h = HEIGHT * scale;
        let Some(pixmap) = render_svg(&self.options, &svg, buf_w, buf_h, scale) else {
            tracing::warn!("native overlay: SVG render failed");
            return;
        };
        let stride = (buf_w * 4) as i32;
        let Some(pool) = self.pool.as_mut() else {
            return;
        };
        let (buffer, canvas) = match pool.create_buffer(
            buf_w as i32,
            buf_h as i32,
            stride,
            wl_shm::Format::Argb8888,
        ) {
            Ok(pair) => pair,
            Err(err) => {
                tracing::warn!("native overlay: cannot create buffer: {err}");
                return;
            }
        };
        copy_bgra(pixmap.data(), canvas);
        let surface = layer.wl_surface();
        surface.damage(0, 0, WIDTH as i32, HEIGHT as i32);
        if let Err(err) = buffer.attach_to(surface) {
            tracing::warn!("native overlay: attach failed: {err}");
            return;
        }
        layer.commit();
        self.last_draw = now;
        self.dirty = false;
    }

    fn build_svg(&self) -> String {
        let (label, dot) = match self.state.phase {
            Phase::Processing => ("Processing", COLOR_AMBER),
            Phase::Speaking => ("Speaking", COLOR_GREEN),
            Phase::Paused => ("Paused", COLOR_MUTED),
        };
        // A slow breathing dot: a quiet "work is happening" signal.
        let pulse = if self.state.phase == Phase::Paused {
            0.7
        } else {
            let t = self.started.elapsed().as_secs_f32();
            (0.55 + 0.45 * (0.5 + 0.5 * (t * 3.6).sin())).clamp(0.3, 1.0)
        };
        let text = escape_xml(&self.phrase_text());
        let text_x = PHRASE_X - self.marquee_offset;
        let mut svg = String::with_capacity(1200);
        let _ = write!(
            svg,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}">"#
        );
        let _ = write!(
            svg,
            r#"<defs><clipPath id="pc"><rect x="{PHRASE_X}" y="5" width="{PHRASE_CLIP_W}" height="24"/></clipPath></defs>"#
        );
        let _ = write!(
            svg,
            r#"<rect x="0.5" y="0.5" width="{}" height="{}" rx="16.5" fill="{COLOR_BG}" fill-opacity="0.94" stroke="{COLOR_BORDER}" stroke-opacity="0.95"/>"#,
            WIDTH - 1,
            HEIGHT - 1
        );
        let _ = write!(
            svg,
            r#"<circle cx="16" cy="17" r="4" fill="{dot}" opacity="{pulse:.2}"/>"#
        );
        let _ = write!(
            svg,
            r#"<text x="28" y="21.5" font-family="sans-serif" font-size="11.5" font-weight="600" fill="{COLOR_TEXT}">{label}</text>"#
        );
        let _ = write!(
            svg,
            r#"<g clip-path="url(#pc)"><text x="{text_x:.1}" y="21.5" font-family="sans-serif" font-size="12" fill="{COLOR_TEXT_DIM}">{text}</text></g>"#
        );
        svg.push_str("</svg>");
        svg
    }
}

impl CompositorHandler for Overlay {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        new_factor: i32,
    ) {
        if self.surface.as_ref().map(|l| l.wl_surface()) != Some(surface) {
            return;
        }
        let factor = new_factor.max(1);
        if factor == self.scale {
            return;
        }
        self.scale = factor;
        if let Some(layer) = self.surface.as_ref() {
            let _ = layer.set_buffer_scale(factor as u32);
        }
        self.dirty = true;
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for Overlay {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for Overlay {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.drop_surface();
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        // We asked for (WIDTH, HEIGHT); a zero size means "you pick", which is
        // also fine because the buffer we attach defines the content size.
        let _ = configure.new_size;
        self.configured = true;
        self.dirty = true;
    }
}

impl ShmHandler for Overlay {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_compositor!(Overlay);
delegate_output!(Overlay);
delegate_shm!(Overlay);
delegate_layer!(Overlay);
delegate_registry!(Overlay);

impl ProvidesRegistryState for Overlay {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState];
}

fn run(rx: Receiver<Update>) -> Result<(), String> {
    let conn = Connection::connect_to_env().map_err(|e| format!("connect: {e}"))?;
    let (globals, mut queue) =
        registry_queue_init::<Overlay>(&conn).map_err(|e| format!("registry: {e}"))?;
    let qh = queue.handle();
    let compositor =
        CompositorState::bind(&globals, &qh).map_err(|e| format!("compositor: {e}"))?;
    let shm = Shm::bind(&globals, &qh).map_err(|e| format!("wl_shm: {e}"))?;
    let layer_shell = LayerShell::bind(&globals, &qh).map_err(|e| format!("layer-shell: {e}"))?;
    let output_state = OutputState::new(&globals, &qh);

    let mut app = Overlay {
        registry_state: RegistryState::new(&globals),
        output_state,
        compositor,
        shm,
        layer_shell,
        surface: None,
        input_region: None,
        pool: None,
        configured: false,
        scale: 1,
        state: Update::default(),
        options: font_options(),
        phrase_width: 0.0,
        marquee_offset: 0.0,
        marquee: Marquee::HoldStart(Instant::now()),
        last_draw: Instant::now(),
        started: Instant::now(),
        dirty: false,
    };

    loop {
        queue
            .dispatch_pending(&mut app)
            .map_err(|e| format!("dispatch: {e}"))?;

        while let Ok(state) = rx.try_recv() {
            app.apply(state);
        }
        if app.state.visible {
            app.ensure_surface(&qh);
        } else if app.surface.is_some() {
            app.drop_surface();
        }

        let now = Instant::now();
        if app.should_draw(now) {
            app.draw(now);
        }
        conn.flush().map_err(|e| format!("flush: {e}"))?;

        match conn.prepare_read() {
            Some(guard) => {
                let fd = guard.connection_fd().as_raw_fd();
                let mut pfd = libc::pollfd {
                    fd,
                    events: libc::POLLIN,
                    revents: 0,
                };
                let timeout = app.poll_timeout_ms();
                let rc = unsafe { libc::poll(&mut pfd, 1, timeout) };
                if rc > 0 && (pfd.revents & libc::POLLIN) != 0 {
                    let _ = guard.read();
                }
            }
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}

fn font_options() -> usvg::Options<'static> {
    let mut options = usvg::Options::default();
    options.font_family = "sans-serif".to_string();
    options.fontdb_mut().load_system_fonts();
    options
}

fn measure_phrase(options: &usvg::Options<'_>, text: &str) -> f32 {
    let mut svg = String::with_capacity(512);
    let _ = write!(
        svg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="4000" height="34"><text x="0" y="21.5" font-family="sans-serif" font-size="12" fill="#ffffff">{}</text></svg>"##,
        escape_xml(text)
    );
    match usvg::Tree::from_str(&svg, options) {
        Ok(tree) => tree.root().abs_bounding_box().width(),
        Err(_) => 0.0,
    }
}

fn render_svg(
    options: &usvg::Options<'_>,
    svg: &str,
    width: u32,
    height: u32,
    scale: u32,
) -> Option<resvg::tiny_skia::Pixmap> {
    let tree = usvg::Tree::from_str(svg, options).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
    let transform = resvg::tiny_skia::Transform::from_scale(scale as f32, scale as f32);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Some(pixmap)
}

/// tiny-skia stores premultiplied RGBA; wl_shm ARGB8888 is BGRA in memory.
fn copy_bgra(src: &[u8], dst: &mut [u8]) {
    for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        d[0] = s[2];
        d[1] = s[1];
        d[2] = s[0];
        d[3] = s[3];
    }
}

fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_space = false;
    for ch in text.chars() {
        let ch = if ch.is_whitespace() { ' ' } else { ch };
        if ch == ' ' {
            if last_space {
                continue;
            }
            last_space = true;
        } else {
            last_space = false;
        }
        out.push(ch);
    }
    out.trim().to_string()
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars).collect();
    out.push('…');
    out
}

fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping_keeps_text_safe_for_svg() {
        assert_eq!(
            escape_xml(r#"a < b & "c" 'd'"#),
            "a &lt; b &amp; &quot;c&quot; &apos;d&apos;"
        );
    }

    #[test]
    fn normalize_collapses_whitespace() {
        assert_eq!(normalize("  hello\n\tworld  "), "hello world");
    }

    #[test]
    fn truncate_adds_ellipsis_past_limit() {
        assert_eq!(truncate("abcdef", 3), "abc…");
        assert_eq!(truncate("abc", 3), "abc");
    }
}

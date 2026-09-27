//! `WebPlugin`: the page as a terminal.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy_app::{App, Plugin, PreUpdate};
use bevy_ecs::schedule::IntoScheduleConfigs;
use plurimus_core::ratatui_core::backend::Backend;
use plurimus_core::{CameraSystems, PresenterPlugin, TerminalRenderAppExt, TerminalSize};
use plurimus_term::{InputCapabilities, InputSystems, KeyCode, KeyModifiers, TermPlugin};
use ratzilla::backend::webgl2::{FontAtlasConfig, WebGl2BackendOptions};
use ratzilla::{CellSized, WebGl2Backend};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{HtmlCanvasElement, HtmlElement};

use super::backend::WebBackend;
use super::extract::{serve_requests, sync_size};
use super::input::{listen, pump_browser_events};
use super::measure::CellMeter;
use super::runner::{ExitTarget, run_on_animation_frames};
use super::{document, warn, window};
use crate::fit::{GridFit, Surface, font_px};
use crate::keys::DEFAULT_PASSTHROUGH;

/// The font family used when none is named.
const DEFAULT_FONT: &str = "monospace";

/// Any size loads a web font's face; this one is only asked for.
const FONT_PROBE_PX: u32 = 16;

/// Presents a plurimus app on a WebGL2 canvas in the page, with the
/// browser's keyboard, pointer, wheel, paste and focus as its input.
///
/// Add it after `CorePlugin`, in place of `CrosstermPlugin`. It installs the
/// app's runner, which updates once per animation frame and holds the first
/// update until the font has loaded, so an app calls `run()` as usual.
///
/// The canvas fills the element it is mounted in, `<body>` unless
/// [`parent`](Self::parent) names another, so that element needs a definite
/// size from the page's CSS - `html, body { height: 100% }` for the body.
pub struct WebPlugin {
    font: String,
    fit: GridFit,
    parent: Option<String>,
    passthrough: Vec<(KeyCode, KeyModifiers)>,
    font_loaded: Arc<AtomicBool>,
}

impl WebPlugin {
    /// The browser's default monospace font at [`GridFit::default`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            font: DEFAULT_FONT.to_owned(),
            fit: GridFit::default(),
            parent: None,
            passthrough: DEFAULT_PASSTHROUGH.to_vec(),
            font_loaded: Arc::default(),
        }
    }

    /// Draws with the CSS font `family`, which the page provides - a web
    /// font it declares with `@font-face`, or one installed locally. The
    /// first update waits for a declared face to load; a family the browser
    /// cannot find falls back to monospace, as CSS itself would.
    #[must_use]
    pub fn font(mut self, family: impl Into<String>) -> Self {
        self.font = family.into();
        self
    }

    /// Chooses the font size, and so the grid, by `fit`.
    #[must_use]
    pub const fn fit(mut self, fit: GridFit) -> Self {
        self.fit = fit;
        self
    }

    /// Mounts the canvas in the element with this `id` instead of `<body>`.
    #[must_use]
    pub fn parent(mut self, element_id: impl Into<String>) -> Self {
        self.parent = Some(element_id.into());
        self
    }

    /// Leaves these keys to the browser as well as the default reload and
    /// devtools keys. Every other key pressed while the canvas has focus is
    /// kept from the browser, so Tab, arrows, Space and Ctrl chords reach
    /// the app. A key matches only with exactly these modifiers held.
    #[must_use]
    pub fn passthrough(mut self, keys: impl IntoIterator<Item = (KeyCode, KeyModifiers)>) -> Self {
        self.passthrough.extend(keys);
        self
    }
}

impl Default for WebPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for WebPlugin {
    fn build(&self, app: &mut App) {
        console_error_panic_hook::set_once();
        if !app.is_plugin_added::<TermPlugin>() {
            app.add_plugins(TermPlugin);
        }
        // A browser reports every key's release, and a release lost to a
        // focus change is covered by the blur that caused it.
        app.insert_resource(
            InputCapabilities::none()
                .with_key_release(true)
                .with_modifier_keys(true),
        );
        load_font(&self.font, Arc::clone(&self.font_loaded));
        app.set_runner(run_on_animation_frames);
    }

    fn ready(&self, _: &App) -> bool {
        self.font_loaded.load(Ordering::Acquire)
    }

    /// Builds the backend now that the font is known. The presenter is added
    /// from here, which bevy accepts until every plugin has finished.
    fn finish(&self, app: &mut App) {
        let parent = parent_element(self.parent.as_deref());
        let surface = surface_of(&parent);
        let meter = CellMeter::new(&self.font);
        let px = font_px(self.fit, surface, |px| meter.as_ref()?.measure(px));
        let options = backend_options(self.parent.as_deref(), &self.font, px, surface);
        let (inner, canvas) = mount_backend(&parent, options);
        if let Some(meter) = &meter {
            warn_on_drift(meter, &inner, px * surface.pixel_ratio as f32);
        }
        let size = inner.size().unwrap_or_default();
        let (cell_width, cell_height) = inner.cell_size_css_px();
        let cell_css = (f64::from(cell_width), f64::from(cell_height));
        app.insert_resource(TerminalSize::new(size.width, size.height));
        app.insert_non_send(listen(&canvas, self.passthrough.clone(), cell_css));
        app.add_systems(
            PreUpdate,
            pump_browser_events
                .in_set(InputSystems::Pump)
                .before(CameraSystems::SyncSize),
        );
        app.insert_non_send(ExitTarget {
            canvas: canvas.clone(),
            title: document().title(),
        });
        let _ = canvas.focus();
        app.add_plugins(PresenterPlugin::new(WebBackend { inner, canvas }));
        app.add_extract_systems((sync_size, serve_requests));
    }
}

/// Starts loading the font's declared face, flagging `loaded` when the
/// attempt ends either way.
///
/// Only a failed load is reported: a family the page does not declare with
/// `@font-face` resolves with no faces whether it is installed or missing,
/// and a missing one falls back to monospace as CSS itself would.
fn load_font(family: &str, loaded: Arc<AtomicBool>) {
    let request = format!("{FONT_PROBE_PX}px '{family}'");
    let pending = JsFuture::from(document().fonts().load(&request));
    let family = family.to_owned();
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(error) = pending.await {
            warn(&format!(
                "plurimus_web: the font '{family}' failed to load ({error:?}); drawing in monospace"
            ));
        }
        loaded.store(true, Ordering::Release);
    });
}

fn parent_element(id: Option<&str>) -> HtmlElement {
    let Some(id) = id else {
        return document()
            .body()
            .expect("plurimus_web: the page has no <body> to mount the canvas in");
    };
    document()
        .get_element_by_id(id)
        .and_then(|element| element.dyn_into().ok())
        .unwrap_or_else(|| panic!("plurimus_web: no element with id {id:?} to mount the canvas in"))
}

fn surface_of(parent: &HtmlElement) -> Surface {
    Surface {
        width: f64::from(parent.client_width()),
        height: f64::from(parent.client_height()),
        pixel_ratio: window().device_pixel_ratio(),
    }
}

fn backend_options(
    parent_id: Option<&str>,
    family: &str,
    px: f32,
    surface: Surface,
) -> WebGl2BackendOptions {
    let options = WebGl2BackendOptions::new()
        .font_atlas_config(FontAtlasConfig::dynamic(&[family], px))
        .size((surface.width as u32, surface.height as u32))
        .disable_auto_css_resize();
    match parent_id {
        Some(id) => options.grid_id(id),
        None => options,
    }
}

/// Builds the renderer in `parent` and styles its canvas to fill it, so the
/// canvas follows the parent's size rather than keeping its first one.
fn mount_backend(
    parent: &HtmlElement,
    options: WebGl2BackendOptions,
) -> (WebGl2Backend, HtmlCanvasElement) {
    let backend = WebGl2Backend::new_with_options(options).unwrap_or_else(|error| {
        panic!("plurimus_web: the WebGL2 canvas could not be created: {error}")
    });
    let canvas: HtmlCanvasElement = parent
        .last_element_child()
        .and_then(|element| element.dyn_into().ok())
        .expect("plurimus_web: the renderer mounted no canvas");
    let _ = canvas.set_attribute("tabindex", "0");
    let style = canvas.style();
    for (property, value) in [
        ("display", "block"),
        ("width", "100%"),
        ("height", "100%"),
        ("outline", "none"),
    ] {
        let _ = style.set_property(property, value);
    }
    (backend, canvas)
}

/// The fit was solved against a copy of the renderer's measurement; if the
/// renderer's own cell differs, that copy has drifted from it upstream.
fn warn_on_drift(meter: &CellMeter, backend: &WebGl2Backend, physical_px: f32) {
    let (width, height) = backend.cell_size_px();
    let renderer = (width.round() as u32, height.round() as u32);
    let copy = meter.measure(physical_px);
    if copy != Some(renderer) {
        warn(&format!(
            "plurimus_web: the renderer's cell {renderer:?} differs from the one measured \
             for the fit {copy:?}; the grid may miss its GridFit"
        ));
    }
}

//! Crossterm backend for plurimus: terminal lifecycle (raw mode, alternate
//! screen, restore-on-exit), the presenter, and crossterm event translation.
//!
//! It takes over the terminal on build, pumps crossterm events into the
//! input contract, and hands core a backend to present through. Core stays
//! backend-generic and knows none of this, so an app can replace this crate
//! with a test harness or another terminal library without changing the
//! pipeline.

mod context;
mod events;

pub use context::{install_panic_hook, restore};
pub use ratatui_crossterm;

#[cfg(unix)]
use std::fs::File;
use std::io::{self, BufWriter, Stdout, Write};
use std::sync::Mutex;

use bevy_app::{App, Plugin, PreUpdate};
use bevy_ecs::prelude::IntoScheduleConfigs;
use plurimus_core::{CameraSystems, PresenterPlugin, TerminalRenderApp};
use plurimus_input::InputSystems;

/// Owns the terminal and presents composed frames via crossterm.
///
/// Requires [`plurimus_core::CorePlugin`] to be added first. Adding this
/// plugin takes over the terminal: raw mode and the alternate screen stay
/// active until the app exits, and a restore-on-panic hook is installed
/// (restoration is idempotent). The kitty keyboard protocol is enabled
/// whenever the terminal supports it, and the terminal's color support is
/// detected from the environment (see [`CrosstermPlugin::detect_color_depth`]).
///
/// The writer defaults to stdout; [`CrosstermPlugin::with_writer`] presents
/// through any writer instead (mirroring `CrosstermBackend<W: Write>`),
/// and [`CrosstermPlugin::tty`] opens the controlling terminal directly so
/// stdout stays free for other output. The writer only selects the output
/// stream: raw mode and restore always target the controlling terminal,
/// so the writer must reach that same terminal. Prefer buffered writers -
/// the backend queues many small writes per frame.
///
/// # Panics
///
/// Building the same plugin value twice panics: the writer is consumed by
/// the first build.
pub struct CrosstermPlugin<W: Write + Send + Sync + 'static = Stdout> {
    mouse: bool,
    paste: bool,
    detect_color_depth: bool,
    writer: Mutex<Option<W>>,
}

impl Default for CrosstermPlugin<Stdout> {
    fn default() -> Self {
        Self::with_writer(io::stdout())
    }
}

#[cfg(unix)]
impl CrosstermPlugin<BufWriter<File>> {
    /// Presents through the controlling terminal (`/dev/tty`), leaving
    /// stdout free - for processes that share fd 1 with other output.
    ///
    /// Build the plugin while stdout still reaches the terminal:
    /// crossterm's kitty-keyboard probe writes its query through stdout,
    /// so redirecting fd 1 first silently degrades input to synthesized
    /// key releases (and stalls startup on the probe timeout).
    ///
    /// # Errors
    ///
    /// Fails when the process has no controlling terminal.
    pub fn tty() -> io::Result<Self> {
        Ok(Self::with_writer(BufWriter::new(context::open_tty()?)))
    }
}

impl<W: Write + Send + Sync + 'static> CrosstermPlugin<W> {
    /// Presents through `writer`, which must reach the controlling
    /// terminal.
    #[must_use]
    pub fn with_writer(writer: W) -> Self {
        Self {
            mouse: true,
            paste: true,
            detect_color_depth: true,
            writer: Mutex::new(Some(writer)),
        }
    }

    /// Sets mouse capture ([`plurimus_input::MouseMessage`]); on by default.
    #[must_use]
    pub fn mouse(mut self, mouse: bool) -> Self {
        self.mouse = mouse;
        self
    }

    /// Sets bracketed paste ([`plurimus_input::PasteMessage`]); on by
    /// default.
    #[must_use]
    pub fn paste(mut self, paste: bool) -> Self {
        self.paste = paste;
        self
    }

    /// Detects the terminal's color support from `COLORTERM`/`TERM` and
    /// inserts the matching [`plurimus_core::ColorDepth`]; on by default.
    /// Inserting `ColorDepth` after this plugin overrides the detected
    /// value; disable instead when the app manages the resource
    /// dynamically.
    #[must_use]
    pub fn detect_color_depth(mut self, detect: bool) -> Self {
        self.detect_color_depth = detect;
        self
    }

    fn take_writer(&self) -> W {
        self.writer
            .lock()
            .expect("CrosstermPlugin writer lock")
            .take()
            .expect("CrosstermPlugin can only be built once")
    }
}

impl<W: Write + Send + Sync + 'static> Plugin for CrosstermPlugin<W> {
    fn build(&self, app: &mut App) {
        let (backend, size, capabilities, color_depth) =
            context::init(self.take_writer(), self.mouse, self.paste)
                .expect("failed to initialize the terminal (the writer must reach a tty)");
        context::install_panic_hook();
        if !app.is_plugin_added::<plurimus_input::InputPlugin>() {
            app.add_plugins(plurimus_input::InputPlugin);
        }
        app.insert_resource(size);
        app.insert_resource(capabilities);
        if self.detect_color_depth {
            app.insert_resource(color_depth);
        }
        app.add_systems(
            PreUpdate,
            events::pump_events
                .in_set(InputSystems::Pump)
                .before(CameraSystems::SyncSize),
        );
        app.add_plugins(PresenterPlugin::new(backend));
        app.sub_app_mut(TerminalRenderApp)
            .insert_resource(context::RestoreOnDrop);
    }
}

#[cfg(test)]
mod tests {
    use super::CrosstermPlugin;

    #[test]
    fn writer_generic_construction_hands_the_writer_to_build() {
        let plugin = CrosstermPlugin::with_writer(Vec::new())
            .mouse(false)
            .paste(false)
            .detect_color_depth(false);

        let writer: Vec<u8> = plugin.take_writer();

        assert!(writer.is_empty());
    }

    #[test]
    #[should_panic(expected = "built once")]
    fn second_build_panics() {
        let plugin = CrosstermPlugin::with_writer(Vec::new());
        let _ = plugin.take_writer();

        let _ = plugin.take_writer();
    }
}

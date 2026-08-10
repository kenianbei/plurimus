//! Core alone: no terminal contract, no crossterm, no ui.
//!
//! `plurimus_core` renders to any ratatui `Backend`, and a terminal is only
//! the most common one. This drives the whole pipeline - two cameras with
//! cell-space viewports, a hand-written [`TerminalWidget`], core's halfblock
//! subcell grid, compositing and colour downsampling to 16 colours - through
//! `TestBackend`, which holds the cells in memory.
//!
//! The example's own code needs nothing but core, which
//! `cargo check -p plurimus --no-default-features --examples` enforces. Run
//! it the same way:
//!
//! ```text
//! cargo run -p plurimus --no-default-features --example headless
//! ```
//!
//! The same shape drives a wgpu or DOM backend, or writes ANSI to a file.
//!
//! [`TerminalWidget`]: plurimus::core::TerminalWidget

mod widget;

#[cfg(test)]
mod tests;

use bevy_app::{App, AppExit};
use plurimus::core::ratatui_core::backend::TestBackend;
use plurimus::core::ratatui_core::layout::{Position, Rect};
use plurimus::core::{
    ColorDepth, CorePlugin, PresenterPlugin, TerminalCamera, TerminalContext, TerminalCursor,
    TerminalRenderApp, TerminalSize, UiArea, UiCamera, UiWidget, Viewport,
};

use widget::Waveform;

const SIZE: TerminalSize = TerminalSize { cols: 40, rows: 12 };
const TOP: Rect = Rect::new(0, 0, 40, 8);
const BOTTOM: Rect = Rect::new(0, 8, 40, 4);

fn main() -> AppExit {
    let mut app = demo();
    // No terminal means no event loop to wait for: drive a frame by hand
    // and read the result straight back out.
    app.update();
    print!("{}", composed(&app));
    AppExit::Success
}

/// The whole scene, so the tests drive what `main` draws rather than a
/// second copy of it that could drift.
fn demo() -> App {
    let mut app = App::new();
    app.add_plugins(CorePlugin);
    app.insert_resource(SIZE);
    // Not the default: a headless target picks its own colour space, and
    // this one gives the pipeline's downsampling something to do.
    app.insert_resource(ColorDepth::Ansi16);
    app.add_plugins(PresenterPlugin::new(TestBackend::new(SIZE.cols, SIZE.rows)));

    // Two cameras split the target the way they would split a terminal.
    let top = app
        .world_mut()
        .spawn(TerminalCamera {
            viewport: Viewport::Fixed(TOP),
            ..TerminalCamera::default()
        })
        .id();
    let bottom = app
        .world_mut()
        .spawn(TerminalCamera {
            order: 1,
            viewport: Viewport::Fixed(BOTTOM),
            ..TerminalCamera::default()
        })
        .id();
    // A `UiArea` is camera-local and clipped to its camera's viewport, so
    // each widget names its camera and fills it from the origin.
    app.world_mut().spawn((
        UiWidget::new(Waveform::new(4)),
        UiArea::Fixed(Rect::new(0, 0, TOP.width, TOP.height)),
        UiCamera(top),
    ));
    app.world_mut().spawn((
        UiWidget::new(Waveform::new(11)),
        UiArea::Fixed(Rect::new(0, 0, BOTTOM.width, BOTTOM.height)),
        UiCamera(bottom),
    ));
    // A cursor is target state rather than terminal state, so `TestBackend`
    // records it like any other backend would.
    app.world_mut().resource_mut::<TerminalCursor>().cell = Some(Position::new(0, 0));
    app
}

/// The cells the backend received, as text.
///
/// Read from the backend rather than from core's `FrameBuffer`: what
/// reached the backend is the thing this example claims works. Written out
/// by hand because `TestBackend`'s own `Display` quotes each row, which
/// suits a snapshot assertion and not a frame.
fn composed(app: &App) -> String {
    let backend = &app
        .sub_app(TerminalRenderApp)
        .world()
        .resource::<TerminalContext<TestBackend>>()
        .backend;
    let buffer = backend.buffer();
    let mut out = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            out.push_str(buffer[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

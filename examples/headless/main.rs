//! Core alone: no terminal contract, no crossterm, no ui.
//!
//! `plurimus_core` renders to any ratatui `Backend`, and a terminal is only
//! the most common one. This drives the whole pipeline - two cameras with
//! cell-space viewports, a hand-written [`TerminalWidget`], subcell raster
//! primitives, compositing and colour downsampling - through `TestBackend`,
//! which holds the cells in memory. Nothing here can reach a terminal, and
//! the example builds with `--no-default-features` to prove it.
//!
//! The same shape drives a wgpu or DOM backend, or writes ANSI to a file.

mod widget;

#[cfg(test)]
mod tests;

use bevy_app::{App, AppExit, Startup, Update};
use bevy_ecs::prelude::{Commands, Local, Res, ResMut};
use plurimus::core::ratatui_core::backend::TestBackend;
use plurimus::core::ratatui_core::layout::{Position, Rect};
use plurimus::core::{
    ColorDepth, CorePlugin, PresenterPlugin, TerminalCamera, TerminalCursor, TerminalSize, UiArea,
    UiWidget, Viewport,
};

use widget::Sparkline;

const SIZE: TerminalSize = TerminalSize { cols: 40, rows: 12 };
const FRAMES: u32 = 8;

fn main() -> AppExit {
    let mut app = App::new();
    app.add_plugins(CorePlugin);
    app.insert_resource(SIZE);
    // Nothing detects a terminal here, so the depth is the app's to state.
    app.insert_resource(ColorDepth::TrueColor);
    app.add_plugins(PresenterPlugin::new(TestBackend::new(SIZE.cols, SIZE.rows)));
    app.add_systems(Startup, spawn);
    app.add_systems(Update, (animate, stop_after_frames));
    app.run()
}

fn spawn(mut commands: Commands, mut cursor: ResMut<TerminalCursor>) {
    // Two cameras split the target the way they would split a terminal.
    commands.spawn(TerminalCamera {
        viewport: Viewport::Fixed(Rect::new(0, 0, 40, 8)),
        ..TerminalCamera::default()
    });
    commands.spawn(TerminalCamera {
        order: 1,
        viewport: Viewport::Fixed(Rect::new(0, 8, 40, 4)),
        ..TerminalCamera::default()
    });
    commands.spawn((
        UiWidget::new(Sparkline::new(0)),
        UiArea::Fixed(Rect::new(0, 0, 40, 8)),
    ));
    // A cursor is target state, not terminal state: TestBackend records it.
    cursor.cell = Some(Position::new(0, 0));
}

fn animate(mut widgets: bevy_ecs::prelude::Query<&mut UiWidget>, mut phase: Local<u16>) {
    *phase = phase.wrapping_add(1);
    for mut widget in &mut widgets {
        *widget = UiWidget::new(Sparkline::new(*phase));
    }
}

fn stop_after_frames(
    mut frames: Local<u32>,
    backend: Res<plurimus::core::TerminalContext<TestBackend>>,
    mut exit: bevy_ecs::prelude::MessageWriter<AppExit>,
) {
    *frames += 1;
    if *frames < FRAMES {
        return;
    }
    print!("{}", render_to_string(&backend.backend));
    exit.write(AppExit::Success);
}

/// The composed cells as text - what a real backend would have written.
fn render_to_string(backend: &TestBackend) -> String {
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

//! The render target's cell dimensions.
//!
//! [`TerminalSize`] is what every camera and buffer resolves against, and it
//! lives in both worlds: the main-world copy is what an app sets, and
//! extraction mirrors it into the render world. Named for the common case,
//! but it is target configuration rather than terminal contract - a headless
//! app renders at a size with no terminal in sight, which is why it is here
//! and the message reporting a real terminal's resize is not.

use bevy_ecs::prelude::{DetectChangesMut, Res, ResMut, Resource};
use bevy_math::Vec2;
use ratatui_core::layout::Rect;

use crate::extract::MainWorld;

/// Terminal dimensions in cells.
///
/// An app renders at whatever size it sets. Where a real terminal is
/// driving, `plurimus_term` owns the resize message and writes this in
/// [`CameraSystems::SyncSize`](crate::CameraSystems::SyncSize), so an app on
/// a terminal should treat it as read-only. The default stands in until
/// something reports otherwise.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct TerminalSize {
    /// Number of columns.
    pub cols: u16,
    /// Number of rows.
    pub rows: u16,
}

impl TerminalSize {
    /// A terminal of `cols` by `rows` cells.
    #[must_use]
    pub const fn new(cols: u16, rows: u16) -> Self {
        Self { cols, rows }
    }

    /// The full-terminal rectangle at the origin.
    #[must_use]
    pub const fn rect(&self) -> Rect {
        Rect::new(0, 0, self.cols, self.rows)
    }
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self { cols: 80, rows: 24 }
    }
}

/// Pixels per terminal cell, per axis: the one place bevy's pixels and the
/// terminal's cells meet.
///
/// A terminal camera's bevy `Camera` reports its viewport and target in these
/// pixels, so `bevy_ui` layout, picking and anything else reasoning in a
/// window's units sees a tree sized for a window - `8 x 16` puts a 560 px
/// card in 70 columns and a 16 px control in one row - laid out the same
/// here. The default is one pixel per cell.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct CellSize {
    /// Pixels per column.
    pub x: f32,
    /// Pixels per row.
    pub y: f32,
}

impl Default for CellSize {
    fn default() -> Self {
        Self { x: 1.0, y: 1.0 }
    }
}

impl CellSize {
    /// The cell as a vector, for scaling cell coordinates to pixels.
    #[must_use]
    pub const fn vec(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }
}

pub(crate) fn extract_size(main_world: Res<MainWorld>, mut size: ResMut<TerminalSize>) {
    size.set_if_neq(*main_world.resource::<TerminalSize>());
}

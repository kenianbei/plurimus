//! The render target's cell dimensions.
//!
//! [`TerminalSize`] is what every camera and buffer resolves against, and it
//! lives in both worlds: the main-world copy is what an app sets, and
//! extraction mirrors it into the render world. Named for the common case,
//! but it is target configuration rather than terminal contract - a headless
//! app renders at a size with no terminal in sight, which is why it is here
//! and the message reporting a real terminal's resize is not.

use bevy_ecs::prelude::{DetectChangesMut, Res, ResMut, Resource};
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
pub struct TerminalSize {
    /// Number of columns.
    pub cols: u16,
    /// Number of rows.
    pub rows: u16,
}

impl TerminalSize {
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

pub(crate) fn extract_size(main_world: Res<MainWorld>, mut size: ResMut<TerminalSize>) {
    size.set_if_neq(*main_world.resource::<TerminalSize>());
}

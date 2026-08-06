//! The terminal's cell dimensions, and the path a size change takes.
//!
//! [`TerminalSize`] is what every camera and buffer resolves against, and it
//! lives in both worlds: core applies incoming [`TerminalResized`] messages to
//! the main-world copy early in the frame, and extraction mirrors the result
//! into the render world. Backends report the change; nothing outside core
//! writes the size directly.

use bevy_ecs::message::Message;
use bevy_ecs::prelude::{DetectChangesMut, MessageReader, Res, ResMut, Resource};
use ratatui_core::layout::Rect;

use crate::extract::MainWorld;

/// Terminal dimensions in cells.
///
/// Read-only outside core: backends report size changes as
/// [`TerminalResized`] messages, and core applies them in
/// [`CameraSystems::SyncSize`](crate::CameraSystems::SyncSize). The default
/// only stands in until a backend reports the real size.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    /// Number of columns.
    pub cols: u16,
    /// Number of rows.
    pub rows: u16,
}

/// A terminal size change reported by a backend.
///
/// Backends write this from their event pump, ordered before
/// [`CameraSystems::SyncSize`](crate::CameraSystems::SyncSize) so the size
/// applies in the same frame.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalResized {
    /// Number of columns.
    pub cols: u16,
    /// Number of rows.
    pub rows: u16,
}

pub(crate) fn apply_terminal_resize(
    mut resizes: MessageReader<TerminalResized>,
    mut size: ResMut<TerminalSize>,
) {
    if let Some(resized) = resizes.read().last() {
        size.set_if_neq(TerminalSize {
            cols: resized.cols,
            rows: resized.rows,
        });
    }
}

impl TerminalSize {
    /// The full-terminal rectangle at the origin.
    #[must_use]
    pub fn rect(&self) -> Rect {
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

//! A real terminal changing size.
//!
//! `plurimus_core` owns [`TerminalSize`] because a render target has one
//! whether or not a terminal is involved; the message reporting that a
//! terminal's size *changed* is terminal contract, so it lives here and
//! writes core's resource.

use bevy_ecs::message::Message;
use bevy_ecs::prelude::{DetectChangesMut, MessageReader, ResMut};
use plurimus_core::TerminalSize;

/// A terminal size change reported by a backend.
///
/// Backends write this from their event pump, ordered before
/// [`CameraSystems::SyncSize`](plurimus_core::CameraSystems::SyncSize) so the
/// size applies in the same frame.
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

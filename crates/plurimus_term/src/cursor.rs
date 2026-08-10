//! The shape a terminal draws its caret with.
//!
//! `plurimus_core` owns where the cursor is, because ratatui's `Backend`
//! can move and hide one whatever it is driving. No `Backend` method sets a
//! *shape*, so that is a thing only a terminal understands, and it lives
//! here for a backend crate to serve.

use bevy_ecs::prelude::Resource;

/// Shape a terminal draws its cursor with.
///
/// Best-effort: a backend with no way to set the shape ignores this, and
/// every terminal is free to. [`Default`](Self::Default) asks for whatever
/// the user configured, which is the polite choice for anything that is not
/// specifically a text caret.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TerminalCursorStyle {
    /// Whatever the terminal was already using.
    #[default]
    Default,
    /// A blinking filled cell.
    BlinkingBlock,
    /// A steady filled cell.
    SteadyBlock,
    /// A blinking line under the cell.
    BlinkingUnderline,
    /// A steady line under the cell.
    SteadyUnderline,
    /// A blinking bar between cells.
    BlinkingBar,
    /// A steady bar between cells.
    SteadyBar,
}

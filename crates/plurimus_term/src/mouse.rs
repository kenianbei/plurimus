//! Pointer half of the input contract: mouse messages and cursor position.

use bevy_ecs::message::Message;
use bevy_ecs::prelude::Resource;
use bevy_input::mouse::MouseButton;
use plurimus_core::ratatui_core::layout::Position;

use crate::KeyModifiers;

/// A mouse event in cell coordinates.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseMessage {
    /// What happened.
    pub kind: MouseKind,
    /// Cell position.
    pub position: Position,
    /// Modifier state at event time.
    pub modifiers: KeyModifiers,
}

/// The kind of a mouse event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MouseKind {
    /// Button pressed.
    Down(MouseButton),
    /// Button released.
    Up(MouseButton),
    /// Moved with a button held.
    Drag(MouseButton),
    /// Moved with no button held.
    Moved,
    /// Scrolled up.
    ScrollUp,
    /// Scrolled down.
    ScrollDown,
    /// Scrolled left.
    ScrollLeft,
    /// Scrolled right.
    ScrollRight,
}

/// The latest known mouse position, in cells.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct CursorCell(pub Option<Position>);

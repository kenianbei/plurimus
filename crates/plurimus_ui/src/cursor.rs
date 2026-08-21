//! The terminal's own cursor, placed from whichever widget holds focus.
//!
//! A widget names the cell its caret occupies in its own content space and
//! this maps it onto the screen, so a scrolled widget does not invert an
//! offset the crate already owns.

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::prelude::{Component, Local, Query, Res, ResMut};
use bevy_input_focus::InputFocus;
use plurimus_core::TerminalCursor;
use plurimus_core::ratatui_core::layout::Position;
use plurimus_term::TerminalCursorStyle;

use crate::interaction::ComputedWidgetArea;
use crate::scroll::{ScrollOffset, screen_cell};

/// Where the terminal cursor sits inside this widget's content, and what
/// shape it takes there.
///
/// In content space, so a scrolled widget names the cell its caret occupies
/// rather than inverting its own offset. The widget holding focus wins, and
/// its cursor is hidden while the cell is scrolled out of view; a widget
/// that never holds focus never shows one.
///
/// The shape travels with the cell so two widgets can want different ones -
/// a bar while inserting, a block while overwriting - without the last one
/// to run keeping the terminal's shape after focus has left it.
///
/// An app placing a cursor that belongs to no widget - a prompt on a status
/// strip - sets [`TerminalCursor`] directly instead, in screen space.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[require(ComputedWidgetArea)]
#[non_exhaustive]
pub struct WidgetCursor {
    /// Cell in the widget's content space; `None` names no cell at all.
    ///
    /// A widget whose caret has nowhere to sit - an empty document, a
    /// selection spanning rather than pointing - says so with `None` rather
    /// than leaving the last cell standing or removing the component, which
    /// would discard the shape an app set and never re-insert it.
    pub cell: Option<Position>,
    /// Shape the terminal draws it with.
    pub style: TerminalCursorStyle,
}

impl WidgetCursor {
    /// A cursor at `cell` in the terminal's own shape.
    #[must_use]
    pub const fn new(cell: Position) -> Self {
        Self {
            cell: Some(cell),
            style: TerminalCursorStyle::Default,
        }
    }

    /// A cursor naming no cell, which is also [`Default`].
    #[must_use]
    pub const fn nowhere() -> Self {
        Self {
            cell: None,
            style: TerminalCursorStyle::Default,
        }
    }

    /// Sets the shape the caret asks the terminal for.
    #[must_use]
    pub const fn with_style(mut self, style: TerminalCursorStyle) -> Self {
        self.style = style;
        self
    }
}

/// Resolves the focused widget's cursor onto the screen.
///
/// Owns the terminal cursor only while a focused widget claims one, so an
/// app that sets [`TerminalCursor`] itself keeps it - the release happens
/// when this system's own cursor goes away, not on every frame nothing is
/// focused. The shape the app had is remembered and given back, so a
/// widget's caret does not outlive the focus that placed it.
pub(crate) fn sync_terminal_cursor(
    focus: Res<InputFocus>,
    widgets: Query<(&WidgetCursor, &ComputedWidgetArea, Option<&ScrollOffset>)>,
    mut cursor: ResMut<TerminalCursor>,
    mut style: ResMut<TerminalCursorStyle>,
    mut displaced: Local<Option<TerminalCursorStyle>>,
) {
    let placed = focus
        .get()
        .and_then(|entity| widgets.get(entity).ok())
        .and_then(|(widget, area, offset)| {
            let offset = offset.map_or(Position::ORIGIN, |offset| offset.0);
            Some((screen_cell(widget.cell?, area.0, offset)?, widget.style))
        });
    match placed {
        Some((cell, wanted)) => {
            // Remembered once, so releasing hands the app's own shape back
            // instead of leaving the terminal wearing a widget's caret.
            if displaced.is_none() {
                *displaced = Some(*style);
            }
            cursor.set_if_neq(TerminalCursor::at(cell));
            style.set_if_neq(wanted);
        }
        None => {
            if let Some(restored) = displaced.take() {
                cursor.set_if_neq(TerminalCursor::hidden());
                style.set_if_neq(restored);
            }
        }
    }
}

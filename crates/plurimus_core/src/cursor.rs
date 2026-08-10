//! The terminal's own cursor, as opposed to anything drawn into a cell.
//!
//! A widget can draw a caret by reversing a cell, and for most of them that
//! is right. A text field is the case where it is not: the terminal cursor
//! is what a screen reader follows and what an input method anchors its
//! composition to, and neither can see a styled cell.
//!
//! Position and visibility go through ratatui's [`Backend`], so the
//! presenter applies them whatever the backend is. Shape does not - no
//! `Backend` method sets it - so a backend crate that can honor
//! [`TerminalCursorStyle`] does it beside the presenter, and one that
//! cannot leaves the terminal's own cursor shape alone.
//!
//! [`Backend`]: ratatui_core::backend::Backend

use bevy_ecs::prelude::{Res, ResMut, Resource};
use ratatui_core::layout::Position;

use crate::extract::MainWorld;

/// Where the terminal cursor sits, in screen cells.
///
/// `None` hides it, which is the default and what an app that never sets a
/// cursor wants. Visibility is read off the position rather than carried
/// beside it, because a hidden cursor that still remembers a cell is a
/// state no terminal has.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TerminalCursor {
    /// Screen cell the cursor occupies; `None` hides it.
    pub cell: Option<Position>,
    /// Shape the cursor takes where a backend can set one.
    pub style: TerminalCursorStyle,
}

impl TerminalCursor {
    /// Puts the cursor at `cell`, keeping the current style.
    pub const fn show(&mut self, cell: Position) {
        self.cell = Some(cell);
    }

    /// Hides the cursor, keeping the style for the next time it shows.
    pub const fn hide(&mut self) {
        self.cell = None;
    }
}

/// Shape a terminal draws its cursor with.
///
/// Best-effort: a backend with no way to set the shape ignores this, and
/// every terminal is free to. [`Default`](Self::Default) asks for whatever
/// the user configured, which is the polite choice for anything that is not
/// specifically a text caret.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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

/// The cursor as the presenter last applied it, so an unchanged one costs
/// no escape sequence.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PreviousCursor(pub(crate) Option<TerminalCursor>);

pub(crate) fn extract_terminal_cursor(
    main_world: Res<MainWorld>,
    mut cursor: ResMut<TerminalCursor>,
) {
    if let Some(source) = main_world.get_resource::<TerminalCursor>() {
        *cursor = *source;
    }
}

#[cfg(test)]
mod tests {
    use bevy_app::App;
    use ratatui_core::backend::TestBackend;

    use super::*;
    use crate::present::{PresenterPlugin, TerminalContext};
    use crate::sub_app::TerminalRenderApp;
    use crate::{CorePlugin, TerminalSize};

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.insert_resource(TerminalSize { cols: 8, rows: 3 });
        app.add_plugins(PresenterPlugin::new(TestBackend::new(8, 3)));
        app
    }

    fn backend(app: &App) -> &TestBackend {
        &app.sub_app(TerminalRenderApp)
            .world()
            .resource::<TerminalContext<TestBackend>>()
            .backend
    }

    #[test]
    fn a_cursor_with_no_cell_stays_hidden() {
        let mut app = app();

        app.update();

        assert!(!backend(&app).cursor_visible(), "nothing asked for one");
    }

    #[test]
    fn setting_a_cell_shows_the_cursor_there() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<TerminalCursor>()
            .show(Position::new(5, 2));

        app.update();

        assert!(backend(&app).cursor_visible());
        assert_eq!(backend(&app).cursor_position(), Position::new(5, 2));
    }

    #[test]
    fn clearing_the_cell_hides_it_again() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<TerminalCursor>()
            .show(Position::new(1, 1));
        app.update();

        app.world_mut().resource_mut::<TerminalCursor>().hide();
        app.update();

        assert!(!backend(&app).cursor_visible());
    }

    // The presenter skips draw and flush entirely when no cell differs, and
    // a caret crossing a cell changes no cell's content - so a cursor
    // applied inside that early return would never move on an idle screen.
    #[test]
    fn the_cursor_moves_on_a_frame_that_draws_nothing() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<TerminalCursor>()
            .show(Position::new(0, 0));
        app.update();

        app.world_mut()
            .resource_mut::<TerminalCursor>()
            .show(Position::new(7, 1));
        app.update();

        assert_eq!(
            backend(&app).cursor_position(),
            Position::new(7, 1),
            "no cell changed between the two frames"
        );
    }
}

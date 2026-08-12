//! The terminal's own cursor, as opposed to anything drawn into a cell.
//!
//! A widget can draw a caret by reversing a cell, and for most of them that
//! is right. A text field is the case where it is not: the terminal cursor
//! is what a screen reader follows and what an input method anchors its
//! composition to, and neither can see a styled cell.
//!
//! Position and visibility go through ratatui's [`Backend`], so the
//! presenter applies them whatever the backend is - which is why they are
//! here. Shape does not: no `Backend` method sets one, so it is terminal
//! contract and lives in `plurimus_term` for a backend to serve.
//!
//! [`Backend`]: ratatui_core::backend::Backend

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::prelude::{Res, ResMut, Resource};
use ratatui_core::layout::Position;

use crate::extract::MainWorld;

/// Where the terminal cursor sits, in screen cells.
///
/// `None` hides it, which is the default and what an app that never sets a
/// cursor wants. Visibility is read off the position rather than carried
/// beside it, because a hidden cursor that still remembers a cell is a
/// state no terminal has.
///
/// An app writes this directly for a cursor that belongs to no widget - a
/// prompt on a status strip. With `plurimus_ui` installed it is also
/// written on the app's behalf whenever the focused widget carries a
/// `WidgetCursor`, and that wins for as long as it lasts: a widget caret
/// and an app caret cannot both have the one terminal cursor.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct TerminalCursor {
    /// Screen cell the cursor occupies; `None` hides it.
    pub cell: Option<Position>,
}

impl TerminalCursor {
    /// A cursor shown at `cell`.
    #[must_use]
    pub const fn at(cell: Position) -> Self {
        Self { cell: Some(cell) }
    }

    /// A hidden cursor, which is also [`Default`].
    #[must_use]
    pub const fn hidden() -> Self {
        Self { cell: None }
    }
}

/// The cell the presenter last applied, so an unchanged cursor costs no
/// escape sequence.
///
/// Advances only after a successful flush, which is why this is a resource
/// rather than change detection on [`TerminalCursor`]: a transient IO
/// failure is retried on the next frame instead of being swallowed with
/// the change tick, the same contract the frame diff has.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum PreviousCursor {
    /// Nothing applied yet, so any cursor differs from it.
    #[default]
    Unknown,
    /// The cell last applied; `None` is a cursor that was hidden.
    Applied(Option<Position>),
}

pub(crate) fn extract_terminal_cursor(
    main_world: Res<MainWorld>,
    mut cursor: ResMut<TerminalCursor>,
) {
    cursor.set_if_neq(*main_world.resource::<TerminalCursor>());
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
        app.world_mut().resource_mut::<TerminalCursor>().cell = Some(Position::new(5, 2));

        app.update();

        assert!(backend(&app).cursor_visible());
        assert_eq!(backend(&app).cursor_position(), Position::new(5, 2));
    }

    #[test]
    fn clearing_the_cell_hides_it_again() {
        let mut app = app();
        app.world_mut().resource_mut::<TerminalCursor>().cell = Some(Position::new(1, 1));
        app.update();

        app.world_mut().resource_mut::<TerminalCursor>().cell = None;
        app.update();

        assert!(!backend(&app).cursor_visible());
    }

    // A caret crossing a cell changes no cell's content, so a cursor
    // applied inside the frame diff would never move on an idle screen.
    #[test]
    fn the_cursor_moves_on_a_frame_that_draws_nothing() {
        let mut app = app();
        app.world_mut().resource_mut::<TerminalCursor>().cell = Some(Position::new(0, 0));
        app.update();

        app.world_mut().resource_mut::<TerminalCursor>().cell = Some(Position::new(7, 1));
        app.update();

        assert_eq!(
            backend(&app).cursor_position(),
            Position::new(7, 1),
            "no cell changed between the two frames"
        );
    }
}

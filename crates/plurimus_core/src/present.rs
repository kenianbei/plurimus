//! The present phase: the composed frame diffed against the last one and
//! written through a ratatui backend.
//!
//! [`PresenterPlugin`] is generic over [`Backend`], which is what lets one
//! pipeline drive a real terminal, a test harness, or anything else that
//! implements the trait. Each frame the presenter compares the
//! [`FrameBuffer`] against the previously flushed one and writes only the
//! cells that differ, so an unchanging screen costs almost nothing to hold.
//! Transient IO errors skip a frame instead of failing the app.

use std::sync::Mutex;

use bevy_app::{App, Plugin};
use bevy_ecs::error::Result as BevyResult;
use bevy_ecs::prelude::{Res, ResMut, Resource};
use ratatui_core::backend::Backend;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;

use crate::compositor::FrameBuffer;
use crate::cursor::{PreviousCursor, TerminalCursor};
use crate::sub_app::{TerminalRenderApp, TerminalRenderAppExt, TerminalRenderSystems};

/// Owns the backend that the presenter draws through.
///
/// It lives in the render sub-app, so the main world has no `Res` of it;
/// read it from an `App` through
/// [`TerminalRenderAppExt::terminal_backend`].
#[derive(Resource)]
#[non_exhaustive]
pub struct TerminalContext<B: Backend + Send + Sync + 'static> {
    /// The backend wired to the terminal.
    pub backend: B,
}

impl<B: Backend + Send + Sync + 'static> TerminalContext<B> {
    /// Owns `backend` for the presenter to draw through.
    #[must_use]
    pub const fn new(backend: B) -> Self {
        Self { backend }
    }
}

#[derive(Resource)]
pub(crate) struct PreviousFrame(pub(crate) Buffer);

impl Default for PreviousFrame {
    fn default() -> Self {
        Self(Buffer::empty(Rect::ZERO))
    }
}

/// Presents composed frames through `backend`: diffs each frame against
/// the previous one and issues a single draw + flush in
/// [`TerminalRenderSystems::Present`].
///
/// Backend crates add this with their concrete backend; a headless or
/// custom presenter can add it with any [`Backend`] implementation.
///
/// # Panics
///
/// Building the same plugin value twice panics: the backend is consumed
/// by the first build.
pub struct PresenterPlugin<B: Backend + Send + Sync + 'static> {
    backend: Mutex<Option<B>>,
}

impl<B: Backend + Send + Sync + 'static> PresenterPlugin<B> {
    /// Presents through `backend`.
    #[must_use]
    pub const fn new(backend: B) -> Self {
        Self {
            backend: Mutex::new(Some(backend)),
        }
    }

    fn take_backend(&self) -> B {
        self.backend
            .lock()
            .expect("PresenterPlugin backend lock")
            .take()
            .expect("PresenterPlugin can only be built once")
    }
}

impl<B> Plugin for PresenterPlugin<B>
where
    B: Backend + Send + Sync + 'static,
    B::Error: core::error::Error + Send + Sync + 'static,
{
    fn build(&self, app: &mut App) {
        let backend = self.take_backend();
        let sub_app = app.sub_app_mut(TerminalRenderApp);
        sub_app.insert_resource(TerminalContext::new(backend));
        sub_app.init_resource::<PreviousFrame>();
        sub_app.init_resource::<PreviousCursor>();
        app.add_terminal_systems(TerminalRenderSystems::Present, present::<B>);
    }
}

fn present<B>(
    mut terminal_context: ResMut<TerminalContext<B>>,
    frame: Res<FrameBuffer>,
    mut previous: ResMut<PreviousFrame>,
    cursor: Res<TerminalCursor>,
    mut previous_cursor: ResMut<PreviousCursor>,
) -> BevyResult
where
    B: Backend + Send + Sync + 'static,
    B::Error: core::error::Error + Send + Sync + 'static,
{
    let presented = present_to(
        &mut terminal_context.backend,
        &mut previous.0,
        &frame.0,
        *cursor,
        &mut previous_cursor,
    );
    match presented {
        Err(error) if !is_transient_io(&error) => Err(error.into()),
        _ => Ok(()),
    }
}

/// Draws the cells that changed and places the cursor, then flushes once.
///
/// Drawing moves the terminal's cursor to the last cell it writes, so a
/// frame that draws puts a shown cursor back even when it did not move, and
/// the one flush keeps the terminal from showing it where the drawing
/// stopped. When nothing differs the backend is left alone entirely - no
/// draw, no flush - which is what keeps an idle screen free. `previous` and
/// `applied` advance only after the flush, so a failed present leaves both
/// stale and the next frame retries it whole.
///
/// Returns whether anything was flushed; only the tests observe it.
fn present_to<B: Backend>(
    backend: &mut B,
    previous: &mut Buffer,
    frame: &Buffer,
    cursor: TerminalCursor,
    applied: &mut PreviousCursor,
) -> Result<bool, B::Error> {
    let drawn = draw_changes(backend, previous, frame)?;
    let placed = place_cursor(backend, cursor, *applied, drawn)?;
    if !drawn && !placed {
        return Ok(false);
    }
    backend.flush()?;
    if drawn {
        previous.clone_from(frame);
    }
    *applied = PreviousCursor::Applied(cursor.cell);
    Ok(true)
}

/// Draws the cells of `frame` that differ from `previous`, without
/// flushing, and reports whether there were any.
///
/// A change of terminal size invalidates the diff, so a differing area
/// clears the backend and restarts from an empty `previous`.
fn draw_changes<B: Backend>(
    backend: &mut B,
    previous: &mut Buffer,
    frame: &Buffer,
) -> Result<bool, B::Error> {
    if previous.area != frame.area {
        backend.clear()?;
        *previous = Buffer::empty(frame.area);
    }
    let mut updates = previous.diff_iter(frame).peekable();
    if updates.peek().is_none() {
        return Ok(false);
    }
    backend.draw(updates)?;
    Ok(true)
}

/// Moves, shows, or hides the terminal cursor when it differs from what was
/// last applied, or when `drawn` moved it, and reports whether it did.
///
/// Deliberately apart from the diff: a cursor crossing a cell changes no
/// cell's content, and drawing never shows a hidden cursor.
fn place_cursor<B: Backend>(
    backend: &mut B,
    cursor: TerminalCursor,
    applied: PreviousCursor,
    drawn: bool,
) -> Result<bool, B::Error> {
    let moved_by_draw = drawn && cursor.cell.is_some();
    if applied == PreviousCursor::Applied(cursor.cell) && !moved_by_draw {
        return Ok(false);
    }
    // Showing an already-shown cursor is its own escape and its own flush
    // on the crossterm backend, so a caret moving one cell asks only to
    // move.
    let was_hidden = !matches!(applied, PreviousCursor::Applied(Some(_)));
    match cursor.cell {
        Some(cell) => {
            backend.set_cursor_position(cell)?;
            if was_hidden {
                backend.show_cursor()?;
            }
        }
        None => backend.hide_cursor()?,
    }
    Ok(true)
}

fn is_transient_io(error: &(dyn core::error::Error + 'static)) -> bool {
    matches!(
        error
            .downcast_ref::<std::io::Error>()
            .map(std::io::Error::kind),
        Some(std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock)
    )
}

#[cfg(test)]
mod tests {
    use ratatui_core::backend::{Backend, TestBackend};
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::{Position, Rect};
    use ratatui_core::style::Style;

    use super::present_to;
    use crate::cursor::{PreviousCursor, TerminalCursor};

    fn present(backend: &mut TestBackend, previous: &mut Buffer, frame: &Buffer) -> bool {
        let mut applied = PreviousCursor::Applied(None);
        present_to(
            backend,
            previous,
            frame,
            TerminalCursor::hidden(),
            &mut applied,
        )
        .unwrap()
    }

    #[test]
    fn draws_initial_frame_then_skips_unchanged() {
        let mut backend = TestBackend::new(4, 1);
        let mut previous = Buffer::empty(Rect::ZERO);
        let mut frame = Buffer::empty(Rect::new(0, 0, 4, 1));
        frame.set_string(0, 0, "hi", Style::new());

        assert!(present(&mut backend, &mut previous, &frame));
        backend.assert_buffer_lines(["hi  "]);
        assert!(!present(&mut backend, &mut previous, &frame));
    }

    #[test]
    fn area_change_clears_and_redraws_fully() {
        let mut backend = TestBackend::new(4, 1);
        let mut previous = Buffer::empty(Rect::ZERO);
        let mut frame = Buffer::empty(Rect::new(0, 0, 4, 1));
        frame.set_string(0, 0, "abcd", Style::new());
        present(&mut backend, &mut previous, &frame);

        backend.resize(2, 1);
        let mut smaller = Buffer::empty(Rect::new(0, 0, 2, 1));
        smaller.set_string(0, 0, "xy", Style::new());

        assert!(present(&mut backend, &mut previous, &smaller));
        backend.assert_buffer_lines(["xy"]);
    }

    // A real terminal leaves its cursor after the last cell a draw writes;
    // the test backend does not, so the move is made by hand.
    #[test]
    fn a_frame_that_draws_puts_a_resting_cursor_back() {
        let mut backend = TestBackend::new(4, 2);
        let mut previous = Buffer::empty(Rect::ZERO);
        let mut applied = PreviousCursor::default();
        let caret = TerminalCursor::at(Position::new(1, 0));
        let mut frame = Buffer::empty(Rect::new(0, 0, 4, 2));
        present_to(&mut backend, &mut previous, &frame, caret, &mut applied).unwrap();

        backend.set_cursor_position(Position::new(3, 1)).unwrap();
        frame.set_string(0, 1, "x", Style::new());
        present_to(&mut backend, &mut previous, &frame, caret, &mut applied).unwrap();

        assert_eq!(backend.get_cursor_position().unwrap(), Position::new(1, 0));
    }
}

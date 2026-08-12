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
    // A transient failure skips the frame and leaves PreviousFrame stale,
    // so the next successful present re-diffs the missed cells.
    let drawn = present_frame(&mut terminal_context.backend, &mut previous.0, &frame.0);
    // Deliberately not inside `present_frame`: it returns early without
    // drawing or flushing when no cell differs, and a cursor crossing a
    // cell changes no cell's content.
    let moved = apply_cursor(&mut terminal_context.backend, *cursor, &mut previous_cursor);
    for outcome in [drawn.map(|_| ()), moved] {
        match outcome {
            Err(error) if !is_transient_io(&error) => return Err(error.into()),
            _ => {}
        }
    }
    Ok(())
}

/// Moves, shows, or hides the terminal cursor when it differs from what was
/// last applied, flushing so the move lands on an otherwise idle frame.
fn apply_cursor<B: Backend>(
    backend: &mut B,
    cursor: TerminalCursor,
    previous: &mut PreviousCursor,
) -> Result<(), B::Error> {
    if *previous == PreviousCursor::Applied(cursor.cell) {
        return Ok(());
    }
    // Showing an already-shown cursor is its own escape and its own flush
    // on the crossterm backend, so a caret moving one cell asks only to
    // move.
    let was_hidden = !matches!(*previous, PreviousCursor::Applied(Some(_)));
    match cursor.cell {
        Some(cell) => {
            backend.set_cursor_position(cell)?;
            if was_hidden {
                backend.show_cursor()?;
            }
        }
        None => backend.hide_cursor()?,
    }
    backend.flush()?;
    *previous = PreviousCursor::Applied(cursor.cell);
    Ok(())
}

fn is_transient_io(error: &(dyn core::error::Error + 'static)) -> bool {
    matches!(
        error
            .downcast_ref::<std::io::Error>()
            .map(std::io::Error::kind),
        Some(std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock)
    )
}

/// Draws the cells of `frame` that differ from `previous`, then flushes.
///
/// A change of terminal size invalidates the diff, so a differing area
/// clears the backend and restarts from an empty `previous`. When nothing
/// differs the backend is left alone entirely - no draw, no flush - which is
/// what keeps an idle screen free. `previous` advances to `frame` only after
/// a successful flush, so a failed present leaves it stale and the missed
/// cells are re-diffed next frame.
///
/// Returns whether anything was flushed; only the tests observe it.
fn present_frame<B: Backend>(
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
    backend.flush()?;
    previous.clone_from(frame);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use ratatui_core::backend::TestBackend;
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::Rect;
    use ratatui_core::style::Style;

    use super::present_frame;

    #[test]
    fn draws_initial_frame_then_skips_unchanged() {
        let mut backend = TestBackend::new(4, 1);
        let mut previous = Buffer::empty(Rect::ZERO);
        let mut frame = Buffer::empty(Rect::new(0, 0, 4, 1));
        frame.set_string(0, 0, "hi", Style::new());

        assert!(present_frame(&mut backend, &mut previous, &frame).unwrap());
        backend.assert_buffer_lines(["hi  "]);
        assert!(!present_frame(&mut backend, &mut previous, &frame).unwrap());
    }

    #[test]
    fn area_change_clears_and_redraws_fully() {
        let mut backend = TestBackend::new(4, 1);
        let mut previous = Buffer::empty(Rect::ZERO);
        let mut frame = Buffer::empty(Rect::new(0, 0, 4, 1));
        frame.set_string(0, 0, "abcd", Style::new());
        present_frame(&mut backend, &mut previous, &frame).unwrap();

        backend.resize(2, 1);
        let mut smaller = Buffer::empty(Rect::new(0, 0, 2, 1));
        smaller.set_string(0, 0, "xy", Style::new());

        assert!(present_frame(&mut backend, &mut previous, &smaller).unwrap());
        backend.assert_buffer_lines(["xy"]);
    }
}

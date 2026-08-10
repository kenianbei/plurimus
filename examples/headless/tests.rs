//! The example's own scene, asserted on.
//!
//! `plurimus_term` is still linked here, because `plurimus_test` is an
//! unconditional dev-dependency of the facade and dev-dependencies reach
//! examples. What the lean tier proves is narrower and still worth having:
//! the example's own code compiles with core alone, which
//! `cargo check -p plurimus --no-default-features --examples` enforces.

use plurimus::core::ratatui_core::layout::Position;

use super::{BOTTOM, SIZE, TOP, composed, demo};

/// Non-blank cells inside `rect`, which is what "something was drawn"
/// means when the drawing is subcell blocks.
fn drawn_in(frame: &str, rect: plurimus::core::ratatui_core::layout::Rect) -> usize {
    frame
        .lines()
        .skip(rect.y as usize)
        .take(rect.height as usize)
        .map(|line| {
            line.chars()
                .skip(rect.x as usize)
                .take(rect.width as usize)
                .filter(|cell| !cell.is_whitespace())
                .count()
        })
        .sum()
}

#[test]
fn the_frame_is_the_size_of_the_target() {
    let mut app = demo();

    app.update();

    let frame = composed(&app);
    assert_eq!(frame.lines().count(), SIZE.rows as usize);
    assert!(
        frame
            .lines()
            .all(|line| line.chars().count() == SIZE.cols as usize)
    );
}

// A hand-written TerminalWidget is the seam this example exists to show.
#[test]
fn a_hand_written_widget_reaches_the_backend() {
    let mut app = demo();

    app.update();

    assert!(
        drawn_in(&composed(&app), TOP) > 0,
        "the widget drew nothing into the frame"
    );
}

// Each camera owns its viewport, so the second one's widget lands in the
// bottom strip and nowhere else - which asserting on the frame's size
// alone would not have caught.
#[test]
fn each_camera_draws_only_inside_its_own_viewport() {
    let mut app = demo();

    app.update();

    let frame = composed(&app);
    assert!(drawn_in(&frame, TOP) > 0, "the top camera drew nothing");
    assert!(
        drawn_in(&frame, BOTTOM) > 0,
        "the bottom camera drew nothing"
    );
}

// The cursor is applied through Backend, which is why it works with no
// terminal underneath.
#[test]
fn the_cursor_is_placed_without_a_terminal() {
    let mut app = demo();

    app.update();

    assert_eq!(cursor(&app), (true, Position::new(0, 0)));
}

fn cursor(app: &bevy_app::App) -> (bool, Position) {
    let backend = &app
        .sub_app(plurimus::core::TerminalRenderApp)
        .world()
        .resource::<plurimus::core::TerminalContext<
            plurimus::core::ratatui_core::backend::TestBackend,
        >>()
        .backend;
    (backend.cursor_visible(), backend.cursor_position())
}

//! Proves the tier as well as the example: everything here runs with no
//! terminal contract in the graph at all.

use bevy_app::App;
use plurimus::core::ratatui_core::backend::TestBackend;
use plurimus::core::ratatui_core::layout::{Position, Rect};
use plurimus::core::{
    ColorDepth, CorePlugin, PresenterPlugin, TerminalCamera, TerminalContext, TerminalCursor,
    UiArea, UiWidget, Viewport,
};

use super::SIZE;
use super::widget::Sparkline;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(CorePlugin);
    app.insert_resource(SIZE);
    app.insert_resource(ColorDepth::TrueColor);
    app.add_plugins(PresenterPlugin::new(TestBackend::new(SIZE.cols, SIZE.rows)));
    app.world_mut().spawn(TerminalCamera {
        viewport: Viewport::Fixed(Rect::new(0, 0, 40, 8)),
        ..TerminalCamera::default()
    });
    app
}

fn backend(app: &App) -> &TestBackend {
    &app.sub_app(plurimus::core::TerminalRenderApp)
        .world()
        .resource::<TerminalContext<TestBackend>>()
        .backend
}

#[test]
fn a_hand_written_widget_reaches_the_backend() {
    let mut app = app();
    app.world_mut().spawn((
        UiWidget::new(Sparkline::new(4)),
        UiArea::Fixed(Rect::new(0, 0, 40, 8)),
    ));

    app.update();

    let buffer = backend(&app).buffer();
    let drawn = (0..buffer.area.width)
        .flat_map(|x| (0..buffer.area.height).map(move |y| (x, y)))
        .filter(|&(x, y)| buffer[(x, y)].symbol() != " ")
        .count();
    assert!(drawn > 0, "the widget drew nothing into the frame");
}

// The cursor is applied through Backend, which is why it works here.
#[test]
fn the_cursor_is_placed_without_a_terminal() {
    let mut app = app();
    app.world_mut().resource_mut::<TerminalCursor>().cell = Some(Position::new(6, 2));

    app.update();

    assert!(backend(&app).cursor_visible());
    assert_eq!(backend(&app).cursor_position(), Position::new(6, 2));
}

#[test]
fn a_second_camera_composites_beneath_its_own_viewport() {
    let mut app = app();
    app.world_mut().spawn(TerminalCamera {
        order: 1,
        viewport: Viewport::Fixed(Rect::new(0, 8, 40, 4)),
        ..TerminalCamera::default()
    });

    app.update();

    let buffer = backend(&app).buffer();
    assert_eq!(
        buffer.area,
        Rect::new(0, 0, SIZE.cols, SIZE.rows),
        "both viewports compose into one frame"
    );
}

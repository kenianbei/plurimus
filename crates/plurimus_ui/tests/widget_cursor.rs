//! The focused widget's caret placed on the terminal, through the scroll
//! offset and the widget's own area.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalCursor, TerminalSize};
use plurimus_term::TerminalCursorStyle;
use plurimus_ui::{ScrollArea, ScrollOffset, UiArea, UiPlugin, WidgetCursor};

const AREA: Rect = Rect::new(4, 2, 6, 3);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, UiPlugin));
    app.insert_resource(TerminalSize::new(20, 8));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_editor(app: &mut App, caret: Position) -> Entity {
    app.world_mut()
        .spawn((WidgetCursor::new(caret), UiArea::Fixed(AREA)))
        .id()
}

/// Focus after a frame has run: `bevy_input_focus` hands focus to the
/// virtual window in `PostStartup`, which would overwrite an earlier set.
fn focus(app: &mut App, entity: Entity) {
    app.update();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(entity, FocusCause::Pressed);
}

fn cursor(app: &App) -> Option<Position> {
    app.world().resource::<TerminalCursor>().cell
}

#[test]
fn a_focused_widgets_caret_lands_at_its_offset_from_the_area() {
    let mut app = app();
    let editor = spawn_editor(&mut app, Position::new(2, 1));
    focus(&mut app, editor);

    app.update();

    assert_eq!(cursor(&app), Some(Position::new(6, 3)));
}

#[test]
fn an_unfocused_widget_shows_no_cursor() {
    let mut app = app();
    spawn_editor(&mut app, Position::new(2, 1));

    app.update();

    assert_eq!(cursor(&app), None, "focus is what claims the cursor");
}

#[test]
fn losing_focus_takes_the_cursor_away_with_it() {
    let mut app = app();
    let editor = spawn_editor(&mut app, Position::new(0, 0));
    focus(&mut app, editor);
    app.update();
    assert!(cursor(&app).is_some());

    app.world_mut().resource_mut::<InputFocus>().clear();
    app.update();

    assert_eq!(cursor(&app), None);
}

#[test]
fn a_scrolled_widget_places_the_caret_through_its_offset() {
    let mut app = app();
    let editor = spawn_editor(&mut app, Position::new(3, 9));
    app.world_mut().entity_mut(editor).insert((
        ScrollArea::new(Size::new(20, 40)),
        ScrollOffset(Position::new(1, 8)),
    ));
    focus(&mut app, editor);

    app.update();

    assert_eq!(cursor(&app), Some(Position::new(6, 3)));
}

// A caret whose character is scrolled off screen has nowhere honest to sit,
// and drawing it at the nearest edge would put it beside a different one.
#[test]
fn a_caret_scrolled_out_of_view_is_hidden_rather_than_clamped() {
    let mut app = app();
    let editor = spawn_editor(&mut app, Position::new(0, 0));
    app.world_mut().entity_mut(editor).insert((
        ScrollArea::new(Size::new(20, 40)),
        ScrollOffset(Position::new(0, 20)),
    ));
    focus(&mut app, editor);

    app.update();

    assert_eq!(cursor(&app), None);
}

// The widget path owns the cursor only while a widget claims it, so a
// status-line prompt setting the resource directly is left alone.
#[test]
fn a_cursor_no_widget_claims_survives() {
    let mut app = app();
    app.update();

    app.world_mut().resource_mut::<TerminalCursor>().cell = Some(Position::new(0, 7));
    app.update();

    assert_eq!(cursor(&app), Some(Position::new(0, 7)));
}

// The shape travels with the cell, so focus moving off a bar-caret widget
// does not leave the terminal wearing its shape.
#[test]
fn the_shape_follows_the_widget_that_owns_the_caret() {
    let mut app = app();
    let editor = app
        .world_mut()
        .spawn((
            WidgetCursor::new(Position::new(0, 0)).with_style(TerminalCursorStyle::SteadyBar),
            UiArea::Fixed(AREA),
        ))
        .id();
    focus(&mut app, editor);

    app.update();

    assert_eq!(
        *app.world().resource::<TerminalCursorStyle>(),
        TerminalCursorStyle::SteadyBar
    );
}

// A widget whose caret has nowhere to sit says so, rather than leaving the
// last cell standing or dropping the component and the style with it.
#[test]
fn a_cursor_naming_no_cell_is_hidden_and_keeps_its_style() {
    let mut app = app();
    let editor = app
        .world_mut()
        .spawn((
            WidgetCursor::new(Position::new(2, 1)).with_style(TerminalCursorStyle::SteadyBar),
            UiArea::Fixed(AREA),
        ))
        .id();
    focus(&mut app, editor);
    app.update();
    assert_eq!(cursor(&app), Some(Position::new(6, 3)));

    app.world_mut()
        .entity_mut(editor)
        .get_mut::<WidgetCursor>()
        .expect("the widget keeps its cursor")
        .cell = None;
    app.update();

    assert_eq!(cursor(&app), None, "no cell to name means no cursor");
    assert_eq!(
        app.world().get::<WidgetCursor>(editor).map(|c| c.style),
        Some(TerminalCursorStyle::SteadyBar),
        "and the style an app set survives for when a cell returns"
    );
}

#[test]
fn naming_a_cell_again_places_the_caret_back() {
    let mut app = app();
    let editor = app
        .world_mut()
        .spawn((WidgetCursor::nowhere(), UiArea::Fixed(AREA)))
        .id();
    focus(&mut app, editor);
    app.update();
    assert_eq!(cursor(&app), None);

    app.world_mut()
        .entity_mut(editor)
        .get_mut::<WidgetCursor>()
        .expect("the widget keeps its cursor")
        .cell = Some(Position::new(2, 1));
    app.update();

    assert_eq!(cursor(&app), Some(Position::new(6, 3)));
}

// A widget's caret must not outlive the focus that placed it.
#[test]
fn losing_focus_gives_the_apps_own_shape_back() {
    let mut app = app();
    app.update();
    *app.world_mut().resource_mut::<TerminalCursorStyle>() = TerminalCursorStyle::SteadyUnderline;
    let editor = app
        .world_mut()
        .spawn((
            WidgetCursor::new(Position::new(0, 0)).with_style(TerminalCursorStyle::SteadyBar),
            UiArea::Fixed(AREA),
        ))
        .id();
    focus(&mut app, editor);
    app.update();
    assert_eq!(
        *app.world().resource::<TerminalCursorStyle>(),
        TerminalCursorStyle::SteadyBar,
        "the widget takes the shape while focused"
    );

    app.world_mut().resource_mut::<InputFocus>().clear();
    app.update();

    assert_eq!(
        *app.world().resource::<TerminalCursorStyle>(),
        TerminalCursorStyle::SteadyUnderline,
        "and gives back what the app had"
    );
    assert_eq!(cursor(&app), None);
}

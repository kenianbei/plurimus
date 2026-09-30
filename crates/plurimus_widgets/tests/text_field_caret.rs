//! The terminal's own cursor placed on a focused `EditableText`'s caret.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_input_focus::InputFocus;
use plurimus_core::ratatui_core::layout::{Position, Rect};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalCursor, TerminalSize};
use plurimus_term::{KeyCode, TerminalCursorStyle};
use plurimus_test::{press_key, set_focus};
use plurimus_ui::{StylistDisabled, UiArea, WidgetCursor};
use plurimus_widgets::{TextMask, WidgetsPlugin, editable_text};

const ROW: Rect = Rect::new(2, 1, 6, 1);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(12, 3));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_focused(app: &mut App, value: &str) -> Entity {
    let field = app
        .world_mut()
        .spawn((editable_text(value), UiArea::Fixed(ROW)))
        .id();
    set_focus(app, field);
    field
}

fn cursor(app: &mut App) -> Option<Position> {
    app.update();
    app.world().resource::<TerminalCursor>().cell
}

// The cursor is a char index, two here, while the wide cluster before it
// puts its column at three.
#[test]
fn the_cursor_lands_after_the_clusters_before_it() {
    let mut app = app();
    spawn_focused(&mut app, "\u{4E2D}b");
    assert_eq!(cursor(&mut app), Some(Position::new(ROW.x + 3, ROW.y)));
}

#[test]
fn a_cursor_past_the_window_sits_on_its_last_column() {
    let mut app = app();
    spawn_focused(&mut app, "longer than six");
    assert_eq!(
        cursor(&mut app),
        Some(Position::new(ROW.right() - 1, ROW.y))
    );
}

#[test]
fn a_wide_mask_glyph_moves_the_cursor_two_cells() {
    let mut app = app();
    let field = spawn_focused(&mut app, "ab");
    app.world_mut()
        .entity_mut(field)
        .insert(TextMask('\u{FF0A}'));
    assert_eq!(cursor(&mut app), Some(Position::new(ROW.x + 4, ROW.y)));
}

#[test]
fn a_field_losing_focus_takes_the_cursor_with_it() {
    let mut app = app();
    spawn_focused(&mut app, "ab");
    assert!(cursor(&mut app).is_some());

    app.world_mut().resource_mut::<InputFocus>().clear();

    assert_eq!(cursor(&mut app), None);
}

// Narrowing the row pins the window again, so the cell has to move in the
// frame the area changes rather than on the next edit.
#[test]
fn a_narrower_row_moves_the_cursor() {
    let mut app = app();
    let field = spawn_focused(&mut app, "abcd");
    assert_eq!(cursor(&mut app), Some(Position::new(ROW.x + 4, ROW.y)));

    let narrow = Rect::new(ROW.x, ROW.y, 3, 1);
    app.world_mut()
        .entity_mut(field)
        .insert(UiArea::Fixed(narrow));

    assert_eq!(
        cursor(&mut app),
        Some(Position::new(narrow.right() - 1, ROW.y))
    );
}

#[test]
fn a_shape_the_app_set_survives_an_edit() {
    let mut app = app();
    let field = spawn_focused(&mut app, "ab");
    app.world_mut()
        .entity_mut(field)
        .insert(WidgetCursor::nowhere().with_style(TerminalCursorStyle::SteadyBar));

    press_key(&mut app, KeyCode::Char('c'));

    assert_eq!(cursor(&mut app), Some(Position::new(ROW.x + 3, ROW.y)));
    assert_eq!(
        *app.world().resource::<TerminalCursorStyle>(),
        TerminalCursorStyle::SteadyBar
    );
}

// An app drawing the row itself places its own caret, so the cell the
// stylist last published must not stay standing once the app takes over.
#[test]
fn a_field_handed_to_the_app_releases_its_cursor() {
    let mut app = app();
    let field = spawn_focused(&mut app, "ab");
    assert!(cursor(&mut app).is_some());

    app.world_mut().entity_mut(field).insert(StylistDisabled);

    assert_eq!(cursor(&mut app), None);
}

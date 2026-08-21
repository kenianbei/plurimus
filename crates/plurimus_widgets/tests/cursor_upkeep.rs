//! Keeping a container's cursor live and visible, whoever moves it.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::ChildOf;
use plurimus_core::ratatui_core::layout::{Constraint, Rect, Size};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_ui::{ScrollArea, ScrollOffset, UiArea};
use plurimus_widgets::{
    ActiveDescendant, TableRow, TableSelection, WidgetsPlugin, list_item, listbox, table,
    table_header, table_row,
};

const AREA: Rect = Rect::new(0, 0, 12, 3);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(12, 6));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_list(app: &mut App, labels: &[&'static str]) -> (Entity, Vec<Entity>) {
    let list = app.world_mut().spawn((listbox(), UiArea::Fixed(AREA))).id();
    let rows = labels
        .iter()
        .map(|label| {
            app.world_mut()
                .spawn((list_item(*label), ChildOf(list)))
                .id()
        })
        .collect();
    (list, rows)
}

fn active(app: &App, container: Entity) -> Option<Entity> {
    app.world().get::<ActiveDescendant>(container).unwrap().0
}

fn offset(app: &App, container: Entity) -> u16 {
    app.world()
        .get::<ScrollOffset>(container)
        .map_or(0, |offset| offset.0.y)
}

fn set_active(app: &mut App, container: Entity, row: Option<Entity>) {
    app.world_mut()
        .entity_mut(container)
        .get_mut::<ActiveDescendant>()
        .expect("the container keeps its cursor")
        .0 = row;
}

// Filtering a list is despawning its rows and spawning new ones, which
// leaves the cursor naming an entity that is gone.
#[test]
fn a_despawned_cursor_row_re_points_to_the_first_survivor() {
    let mut app = app();
    let (list, rows) = spawn_list(&mut app, &["a", "b", "c"]);
    set_active(&mut app, list, Some(rows[2]));
    app.update();

    app.world_mut().entity_mut(rows[2]).despawn();
    app.update();

    assert_eq!(active(&app, list), Some(rows[0]));
}

#[test]
fn a_cursor_on_a_surviving_row_is_left_alone() {
    let mut app = app();
    let (list, rows) = spawn_list(&mut app, &["a", "b", "c"]);
    set_active(&mut app, list, Some(rows[1]));
    app.update();

    app.world_mut().entity_mut(rows[2]).despawn();
    app.update();

    assert_eq!(active(&app, list), Some(rows[1]));
}

// An app that cleared the cursor meant it; only a dangling one is repaired.
#[test]
fn an_empty_cursor_stays_empty_when_rows_change() {
    let mut app = app();
    let (list, _) = spawn_list(&mut app, &["a", "b"]);
    set_active(&mut app, list, None);
    app.update();

    app.world_mut().spawn((list_item("c"), ChildOf(list)));
    app.update();

    assert_eq!(active(&app, list), None);
}

#[test]
fn losing_every_row_clears_the_cursor() {
    let mut app = app();
    let (list, rows) = spawn_list(&mut app, &["a"]);
    set_active(&mut app, list, Some(rows[0]));
    app.update();

    app.world_mut().entity_mut(rows[0]).despawn();
    app.update();

    assert_eq!(active(&app, list), None);
}

// The reveal belongs to the cursor, so a writer that is not the list's own
// key handler scrolls too.
#[test]
fn writing_the_cursor_directly_scrolls_it_into_view() {
    let mut app = app();
    let (list, rows) = spawn_list(&mut app, &["a", "b", "c", "d", "e", "f"]);
    app.world_mut()
        .entity_mut(list)
        .insert(ScrollArea::new(Size::new(12, 6)));
    app.update();
    assert_eq!(offset(&app, list), 0);

    set_active(&mut app, list, Some(rows[5]));
    app.update();

    assert!(
        offset(&app, list) > 0,
        "the last row of six cannot be visible in three lines at offset zero"
    );
}

#[test]
fn a_repaired_cursor_scrolls_back_into_view() {
    let mut app = app();
    let (list, rows) = spawn_list(&mut app, &["a", "b", "c", "d", "e", "f"]);
    app.world_mut()
        .entity_mut(list)
        .insert(ScrollArea::new(Size::new(12, 6)));
    set_active(&mut app, list, Some(rows[5]));
    app.update();
    assert!(offset(&app, list) > 0);

    app.world_mut().entity_mut(rows[5]).despawn();
    app.update();

    assert_eq!(active(&app, list), Some(rows[0]));
    assert_eq!(
        offset(&app, list),
        0,
        "the repair moved the cursor, so the reveal follows it back"
    );
}

#[test]
fn a_tables_cursor_is_repaired_past_its_header() {
    let mut app = app();
    let table_entity = app
        .world_mut()
        .spawn((
            table([Constraint::Fill(1)]),
            TableSelection::Row,
            UiArea::Fixed(AREA),
        ))
        .id();
    app.world_mut()
        .spawn((table_header(["h"]), ChildOf(table_entity)));
    let first = app
        .world_mut()
        .spawn((table_row(["a"]), ChildOf(table_entity)))
        .id();
    let second = app
        .world_mut()
        .spawn((table_row(["b"]), ChildOf(table_entity)))
        .id();
    set_active(&mut app, table_entity, Some(second));
    app.update();

    app.world_mut().entity_mut(second).despawn();
    app.update();

    assert_eq!(
        active(&app, table_entity),
        Some(first),
        "the header is not a row the cursor can land on"
    );
    assert!(app.world().get::<TableRow>(first).is_some());
}

//! What a `Table` does with keys and clicks: cursor movement in each
//! selection mode, remapped bindings, and column hit testing.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, Resource};
use bevy_input::keyboard::Key;
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::layout::{Constraint, Rect};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_input::KeyCode;
use plurimus_test::{click, press_key};
use plurimus_ui::{Checked, InteractionDisabled, UiArea, ValueChange};
use plurimus_widgets::ActiveDescendant;
use plurimus_widgets::{
    ActiveColumn, TableAction, TableCursor, TableHeaderClick, TableKeys, TableMultiSelect,
    TablePosition, TableSelection, WidgetsPlugin, table, table_footer, table_header, table_row,
    table_self_update,
};

const AREA: Rect = Rect::new(0, 0, 20, 6);

#[derive(Resource, Default)]
struct Selected(Vec<TablePosition>);

#[derive(Resource, Default)]
struct HeaderClicks(Vec<usize>);

fn app(mode: TableSelection) -> (App, Entity, [Entity; 3]) {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize { cols: 20, rows: 6 });
    app.init_resource::<Selected>();
    app.init_resource::<HeaderClicks>();
    app.world_mut().spawn(TerminalCamera::default());
    app.add_observer(
        |change: On<ValueChange<TablePosition>>,
         mut selected: bevy_ecs::prelude::ResMut<Selected>| {
            selected.0.push(change.value);
        },
    );
    app.add_observer(
        |click: On<TableHeaderClick>, mut clicks: bevy_ecs::prelude::ResMut<HeaderClicks>| {
            clicks.0.push(click.column);
        },
    );

    let world = app.world_mut();
    let table = world
        .spawn((
            table([Constraint::Length(6), Constraint::Length(6)]),
            mode,
            UiArea::Fixed(AREA),
        ))
        .id();
    world.spawn((table_header(["name", "date"]), ChildOf(table)));
    let rows = [
        world
            .spawn((table_row(["ann", "may"]), ChildOf(table)))
            .id(),
        world.spawn((table_row(["bo", "jun"]), ChildOf(table))).id(),
        world.spawn((table_row(["cy", "jul"]), ChildOf(table))).id(),
    ];
    world.spawn((table_footer(["total", "3"]), ChildOf(table)));
    world
        .resource_mut::<InputFocus>()
        .set(table, FocusCause::Pressed);
    app.update();
    (app, table, rows)
}

fn cursor(app: &App, table: Entity) -> Option<Entity> {
    app.world().entity(table).get::<ActiveDescendant>()?.0
}

fn column(app: &App, table: Entity) -> Option<usize> {
    app.world().entity(table).get::<ActiveColumn>()?.0
}

#[test]
fn the_arrows_walk_the_body_rows() {
    let (mut app, table, rows) = app(TableSelection::Row);

    press_key(&mut app, KeyCode::Down);
    assert_eq!(
        cursor(&app, table),
        Some(rows[0]),
        "the first press lands on the first row"
    );
    press_key(&mut app, KeyCode::Down);
    assert_eq!(
        cursor(&app, table),
        Some(rows[1]),
        "and the next moves down"
    );
    press_key(&mut app, KeyCode::Up);
    assert_eq!(cursor(&app, table), Some(rows[0]), "and up again");
}

#[test]
fn home_and_end_jump_to_the_ends_of_the_body() {
    let (mut app, table, rows) = app(TableSelection::Row);

    press_key(&mut app, KeyCode::End);
    assert_eq!(
        cursor(&app, table),
        Some(rows[2]),
        "End is the last body row, not the footer"
    );
    press_key(&mut app, KeyCode::Home);
    assert_eq!(
        cursor(&app, table),
        Some(rows[0]),
        "Home is the first, not the header"
    );
}

#[test]
fn a_page_key_moves_by_the_visible_body_height() {
    let (mut app, table, rows) = app(TableSelection::Row);

    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::PageDown);
    assert_eq!(
        cursor(&app, table),
        Some(rows[2]),
        "four body rows fit in six, so a page clamps to the last row"
    );
    press_key(&mut app, KeyCode::PageUp);
    assert_eq!(cursor(&app, table), Some(rows[0]), "and back to the first");
}

#[test]
fn the_bindings_are_the_apps_to_replace() {
    let (mut app, table, rows) = app(TableSelection::Row);
    app.world_mut().entity_mut(table).insert(TableKeys(vec![
        (Key::Character("j".into()), TableAction::RowNext),
        (Key::Character("k".into()), TableAction::RowPrev),
    ]));

    press_key(&mut app, KeyCode::Char('j'));
    press_key(&mut app, KeyCode::Char('j'));
    assert_eq!(cursor(&app, table), Some(rows[1]), "j walks down");
    press_key(&mut app, KeyCode::Char('k'));
    assert_eq!(cursor(&app, table), Some(rows[0]), "k walks up");

    press_key(&mut app, KeyCode::Down);
    assert_eq!(
        cursor(&app, table),
        Some(rows[0]),
        "and the arrows no longer move a table that unbound them"
    );
}

#[test]
fn enter_selects_the_cursor_row() {
    let (mut app, _, rows) = app(TableSelection::Row);

    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Enter);

    let selected = &app.world().resource::<Selected>().0;
    assert_eq!(
        selected.last(),
        Some(&TablePosition {
            row: Some(rows[0]),
            column: None
        }),
        "row mode reports the row and no column"
    );
}

#[test]
fn column_mode_walks_columns_and_reports_no_row() {
    let (mut app, table, _) = app(TableSelection::Column);

    press_key(&mut app, KeyCode::Right);
    assert_eq!(
        column(&app, table),
        Some(0),
        "the first press lands on the first column"
    );
    press_key(&mut app, KeyCode::Right);
    assert_eq!(column(&app, table), Some(1), "and the next moves right");
    press_key(&mut app, KeyCode::Enter);

    let selected = &app.world().resource::<Selected>().0;
    assert_eq!(
        selected.last(),
        Some(&TablePosition {
            row: None,
            column: Some(1)
        }),
        "column mode reports the column and no row"
    );
}

#[test]
fn cell_mode_reports_both_coordinates() {
    let (mut app, _, rows) = app(TableSelection::Cell);

    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Right);
    press_key(&mut app, KeyCode::Enter);

    let selected = &app.world().resource::<Selected>().0;
    assert_eq!(
        selected.last(),
        Some(&TablePosition {
            row: Some(rows[0]),
            column: Some(0)
        }),
        "cell mode reports the row and the column together"
    );
}

#[test]
fn a_row_mode_table_ignores_the_column_keys() {
    let (mut app, table, _) = app(TableSelection::Row);

    press_key(&mut app, KeyCode::Right);
    assert_eq!(
        column(&app, table),
        None,
        "a row cursor has no column to move"
    );
}

#[test]
fn a_click_selects_the_row_under_the_pointer() {
    let (mut app, table, rows) = app(TableSelection::Row);

    click(&mut app, 2, 2);
    assert_eq!(
        cursor(&app, table),
        Some(rows[1]),
        "the header takes the first line, so the second body row is at y=2"
    );
}

#[test]
fn a_click_on_the_footer_selects_nothing() {
    let (mut app, table, _) = app(TableSelection::Row);

    click(&mut app, 2, 5);
    assert_eq!(
        cursor(&app, table),
        None,
        "the footer band is not a row, even where a longer body would reach"
    );
}

// With a zero-width cursor symbol the gutter never appears, so these
// boundaries are the column layout alone: two six-cell columns, one of
// spacing between them.
#[test]
fn a_click_finds_the_column_it_landed_in() {
    let (mut app, table, _) = app(TableSelection::Cell);
    app.world_mut()
        .entity_mut(table)
        .insert(TableCursor("".into()));

    for (x, expected) in [(0, 0), (5, 0), (7, 1), (12, 1)] {
        click(&mut app, x, 1);
        assert_eq!(
            column(&app, table),
            Some(expected),
            "x={x} lands in column {expected}"
        );
    }
}

#[test]
fn the_cursor_gutter_shifts_the_columns_it_is_drawn_in() {
    let (mut app, table, _) = app(TableSelection::Cell);

    click(&mut app, 7, 1);
    assert_eq!(
        column(&app, table),
        Some(1),
        "with no cursor yet, no gutter is reserved"
    );

    // A row is selected now, so ratatui reserves the symbol's two cells and
    // every column sits two further right.
    click(&mut app, 7, 1);
    assert_eq!(
        column(&app, table),
        Some(0),
        "the same x now lands in the first column, shifted by the gutter"
    );
}

#[test]
fn an_empty_cursor_symbol_reserves_no_gutter() {
    let (mut app, table, _) = app(TableSelection::Cell);
    app.world_mut()
        .entity_mut(table)
        .insert(TableCursor("".into()));

    click(&mut app, 7, 1);
    click(&mut app, 7, 1);
    assert_eq!(
        column(&app, table),
        Some(1),
        "a zero-width symbol leaves the columns where they were"
    );
}

#[test]
fn a_header_click_reports_its_column_and_selects_nothing() {
    let (mut app, table, _) = app(TableSelection::Row);

    click(&mut app, 8, 0);

    assert_eq!(
        app.world().resource::<HeaderClicks>().0.last(),
        Some(&1),
        "the header reports the column under the pointer"
    );
    assert_eq!(cursor(&app, table), None, "and moves no cursor");
}

#[test]
fn a_disabled_table_takes_neither_keys_nor_clicks() {
    let (mut app, table, _) = app(TableSelection::Row);
    app.world_mut()
        .entity_mut(table)
        .insert(InteractionDisabled);

    press_key(&mut app, KeyCode::Down);
    click(&mut app, 2, 2);

    assert_eq!(cursor(&app, table), None, "no cursor moved");
    assert!(
        app.world().resource::<Selected>().0.is_empty(),
        "and nothing was selected"
    );
}

fn checked_rows(app: &App, rows: [Entity; 3]) -> Vec<bool> {
    rows.iter()
        .map(|&row| app.world().entity(row).contains::<Checked>())
        .collect()
}

#[test]
fn self_update_moves_checked_among_the_rows() {
    let (mut app, _, rows) = app(TableSelection::Row);
    app.add_observer(table_self_update);

    click(&mut app, 2, 1);
    assert_eq!(checked_rows(&app, rows), [true, false, false]);
    click(&mut app, 2, 2);
    assert_eq!(
        checked_rows(&app, rows),
        [false, true, false],
        "single select moves it rather than adding"
    );
}

#[test]
fn multi_select_toggles_instead_of_moving() {
    let (mut app, table, rows) = app(TableSelection::Row);
    app.add_observer(table_self_update);
    app.world_mut().entity_mut(table).insert(TableMultiSelect);

    click(&mut app, 2, 1);
    click(&mut app, 2, 2);
    assert_eq!(
        checked_rows(&app, rows),
        [true, true, false],
        "both rows stay checked"
    );
    click(&mut app, 2, 2);
    assert_eq!(
        checked_rows(&app, rows),
        [true, false, false],
        "and clicking one again clears it"
    );
}

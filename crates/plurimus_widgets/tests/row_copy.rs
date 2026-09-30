//! A focused list box or table copies its rows on `Ctrl+c`: the cursor
//! row, or every checked row of a multi-select container, and with nothing
//! to copy the key reaches the container's ancestors.

use bevy_app::App;
use bevy_ecs::bundle::Bundle;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::FocusedInput;
use plurimus_core::ratatui_core::layout::{Constraint, Rect};
use plurimus_core::ratatui_core::text::Text;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::{KeyCode, ModifierKey};
use plurimus_test::{clipboard_writes, press_chord, press_key, set_focus};
use plurimus_ui::{Checked, InteractionDisabled, KeyBinding, UiArea};
use plurimus_widgets::{
    ActiveColumn, ActiveDescendant, ListBoxAction, ListBoxKeys, ListBoxMultiSelect, ListItemText,
    ListItemTrailing, Marked, TableMultiSelect, TableSelection, WidgetsPlugin, list_item, listbox,
    table, table_footer, table_header, table_row,
};

const AREA: Rect = Rect::new(0, 0, 20, 6);

#[derive(Resource, Default)]
struct Propagated(Vec<Key>);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(20, 6));
    app.init_resource::<Propagated>();
    app.world_mut().spawn(TerminalCamera::default());
    app
}

/// A form that records every key but a modifier propagating into it.
fn spawn_form(app: &mut App) -> Entity {
    let form = app.world_mut().spawn_empty().id();
    app.world_mut().entity_mut(form).observe(
        |input: On<FocusedInput<KeyboardInput>>, mut seen: ResMut<Propagated>| {
            let key = &input.input.logical_key;
            let is_modifier = matches!(key, Key::Control | Key::Shift | Key::Alt);
            if input.input.state.is_pressed() && !is_modifier {
                seen.0.push(key.clone());
            }
        },
    );
    form
}

/// A focused list inside a form, holding `rows`.
fn spawn_list(app: &mut App, rows: Vec<impl Bundle>) -> (Entity, Vec<Entity>) {
    let form = spawn_form(app);
    let world = app.world_mut();
    let list = world
        .spawn((listbox(), UiArea::Fixed(AREA), ChildOf(form)))
        .id();
    let rows = rows
        .into_iter()
        .map(|row| world.spawn((row, ChildOf(list))).id())
        .collect();
    set_focus(app, list);
    app.update();
    (list, rows)
}

fn spawn_greek(app: &mut App) -> (Entity, Vec<Entity>) {
    spawn_list(app, ["alpha", "beta", "gamma"].map(list_item).into())
}

fn point_at(app: &mut App, list: Entity, row: Entity) {
    app.world_mut()
        .entity_mut(list)
        .insert(ActiveDescendant(Some(row)));
}

fn copy(app: &mut App) {
    press_chord(app, ModifierKey::ControlLeft, KeyCode::Char('c'));
}

fn propagated(app: &App) -> &[Key] {
    &app.world().resource::<Propagated>().0
}

/// Nothing was copied, and the key reached the form.
fn assert_passed_on(app: &mut App) {
    assert!(clipboard_writes(app).is_empty());
    assert_eq!(propagated(app), [Key::Character("c".into())]);
}

#[test]
fn ctrl_c_copies_the_cursor_row() {
    let mut app = app();
    let (list, rows) = spawn_greek(&mut app);
    point_at(&mut app, list, rows[1]);

    copy(&mut app);

    assert_eq!(clipboard_writes(&mut app), ["beta"]);
    assert!(propagated(&app).is_empty(), "{:?}", propagated(&app));
}

#[test]
fn a_multi_select_list_copies_its_checked_rows_in_order() {
    let mut app = app();
    let (list, rows) = spawn_greek(&mut app);
    app.world_mut().entity_mut(list).insert(ListBoxMultiSelect);
    app.world_mut().entity_mut(rows[2]).insert(Checked);
    app.world_mut().entity_mut(rows[0]).insert(Checked);
    point_at(&mut app, list, rows[1]);

    copy(&mut app);

    assert_eq!(clipboard_writes(&mut app), ["alpha\ngamma"]);
}

#[test]
fn a_single_select_list_copies_the_cursor_row_past_its_checked_one() {
    let mut app = app();
    let (list, rows) = spawn_greek(&mut app);
    app.world_mut().entity_mut(rows[0]).insert(Checked);
    point_at(&mut app, list, rows[1]);

    copy(&mut app);

    assert_eq!(clipboard_writes(&mut app), ["beta"]);
}

#[test]
fn a_multi_line_row_copies_every_line() {
    let mut app = app();
    let row = (list_item("label"), ListItemText(Text::from("one\ntwo")));
    let (list, rows) = spawn_list(&mut app, vec![row]);
    point_at(&mut app, list, rows[0]);

    copy(&mut app);

    assert_eq!(clipboard_writes(&mut app), ["one\ntwo"]);
}

#[test]
fn trailing_text_and_marked_rows_are_left_out() {
    let mut app = app();
    let (list, rows) = spawn_greek(&mut app);
    app.world_mut().entity_mut(list).insert(ListBoxMultiSelect);
    app.world_mut()
        .entity_mut(rows[1])
        .insert(ListItemTrailing("^K".into()));
    app.world_mut().entity_mut(rows[0]).insert(Marked);
    point_at(&mut app, list, rows[1]);

    copy(&mut app);

    assert_eq!(clipboard_writes(&mut app), ["beta"]);
}

#[test]
fn a_list_with_no_cursor_row_copies_nothing_and_propagates() {
    let mut app = app();
    spawn_greek(&mut app);

    copy(&mut app);

    assert_passed_on(&mut app);
}

#[test]
fn an_empty_list_copies_nothing_and_propagates() {
    let mut app = app();
    spawn_list(&mut app, Vec::<()>::new());

    copy(&mut app);

    assert_passed_on(&mut app);
}

#[test]
fn a_disabled_list_copies_nothing_and_propagates() {
    let mut app = app();
    let (list, rows) = spawn_greek(&mut app);
    point_at(&mut app, list, rows[0]);
    app.world_mut().entity_mut(list).insert(InteractionDisabled);
    app.update();

    copy(&mut app);

    assert_passed_on(&mut app);
}

#[test]
fn copy_follows_a_remapped_binding() {
    let mut app = app();
    let (list, rows) = spawn_greek(&mut app);
    point_at(&mut app, list, rows[2]);
    let keys = ListBoxKeys(vec![(
        KeyBinding::new(Key::Character("y".into())),
        ListBoxAction::Copy,
    )]);
    app.world_mut().entity_mut(list).insert(keys);

    press_key(&mut app, KeyCode::Char('y'));
    copy(&mut app);

    assert_eq!(clipboard_writes(&mut app), ["gamma"]);
    assert_eq!(propagated(&app), [Key::Character("c".into())]);
}

#[test]
fn copying_leaves_the_cursor_where_it_was() {
    let mut app = app();
    let (list, rows) = spawn_greek(&mut app);
    point_at(&mut app, list, rows[2]);

    copy(&mut app);

    let active = app.world().get::<ActiveDescendant>(list).unwrap();
    assert_eq!(active.0, Some(rows[2]));
}

/// A focused table inside a form, banded by a header and a footer around
/// three body rows.
fn spawn_table(app: &mut App, selection: Option<TableSelection>) -> (Entity, [Entity; 3]) {
    let form = spawn_form(app);
    let world = app.world_mut();
    let table = world
        .spawn((
            table([Constraint::Length(6), Constraint::Length(6)]),
            UiArea::Fixed(AREA),
            ChildOf(form),
        ))
        .id();
    if let Some(selection) = selection {
        world.entity_mut(table).insert(selection);
    }
    world.spawn((table_header(["name", "date"]), ChildOf(table)));
    let rows = [["ann", "may"], ["bo", "jun"], ["cy", "jul"]]
        .map(|cells| world.spawn((table_row(cells), ChildOf(table))).id());
    world.spawn((table_footer(["all", "3"]), ChildOf(table)));
    set_focus(app, table);
    app.update();
    (table, rows)
}

fn point_at_cell(app: &mut App, table: Entity, row: Option<Entity>, column: usize) {
    app.world_mut()
        .entity_mut(table)
        .insert((ActiveDescendant(row), ActiveColumn(Some(column))));
}

#[test]
fn a_row_table_copies_the_cursor_row_tab_separated() {
    let mut app = app();
    let (table, rows) = spawn_table(&mut app, Some(TableSelection::Row));
    point_at(&mut app, table, rows[1]);

    copy(&mut app);

    assert_eq!(clipboard_writes(&mut app), ["bo\tjun"]);
    assert!(propagated(&app).is_empty(), "{:?}", propagated(&app));
}

#[test]
fn a_column_table_copies_the_body_column_without_its_bands() {
    let mut app = app();
    let (table, _) = spawn_table(&mut app, Some(TableSelection::Column));
    point_at_cell(&mut app, table, None, 1);

    copy(&mut app);

    assert_eq!(clipboard_writes(&mut app), ["may\njun\njul"]);
}

#[test]
fn a_cell_table_copies_the_one_cell() {
    let mut app = app();
    let (table, rows) = spawn_table(&mut app, Some(TableSelection::Cell));
    point_at_cell(&mut app, table, Some(rows[2]), 0);

    copy(&mut app);

    assert_eq!(clipboard_writes(&mut app), ["cy"]);
}

#[test]
fn a_multi_select_cell_table_copies_the_column_cell_of_each_checked_row() {
    let mut app = app();
    let (table, rows) = spawn_table(&mut app, Some(TableSelection::Cell));
    app.world_mut().entity_mut(table).insert(TableMultiSelect);
    app.world_mut().entity_mut(rows[0]).insert(Checked);
    app.world_mut().entity_mut(rows[2]).insert(Checked);
    point_at_cell(&mut app, table, Some(rows[1]), 1);

    copy(&mut app);

    assert_eq!(clipboard_writes(&mut app), ["may\njul"]);
}

#[test]
fn a_column_table_with_no_column_copies_nothing_and_propagates() {
    let mut app = app();
    spawn_table(&mut app, Some(TableSelection::Column));

    copy(&mut app);

    assert_passed_on(&mut app);
}

#[test]
fn a_row_table_with_no_cursor_row_copies_nothing_and_propagates() {
    let mut app = app();
    spawn_table(&mut app, Some(TableSelection::Row));

    copy(&mut app);

    assert_passed_on(&mut app);
}

#[test]
fn a_table_without_a_selection_copies_nothing() {
    let mut app = app();
    spawn_table(&mut app, None);

    copy(&mut app);

    assert_passed_on(&mut app);
}

#[test]
fn a_disabled_table_copies_nothing_and_propagates() {
    let mut app = app();
    let (table, rows) = spawn_table(&mut app, Some(TableSelection::Row));
    point_at(&mut app, table, rows[0]);
    app.world_mut()
        .entity_mut(table)
        .insert(InteractionDisabled);
    app.update();

    copy(&mut app);

    assert_passed_on(&mut app);
}

#[test]
fn copying_leaves_the_table_cursor_where_it_was() {
    let mut app = app();
    let (table, rows) = spawn_table(&mut app, Some(TableSelection::Row));
    point_at(&mut app, table, rows[2]);

    copy(&mut app);

    assert_eq!(clipboard_writes(&mut app), ["cy\tjul"]);
    let active = app.world().get::<ActiveDescendant>(table).unwrap();
    assert_eq!(active.0, Some(rows[2]));
}

// A table switched from cell to row selection keeps its old column.
#[test]
fn a_row_table_copies_the_whole_row_past_a_leftover_column() {
    let mut app = app();
    let (table, rows) = spawn_table(&mut app, Some(TableSelection::Row));
    point_at_cell(&mut app, table, Some(rows[0]), 1);

    copy(&mut app);

    assert_eq!(clipboard_writes(&mut app), ["ann\tmay"]);
}

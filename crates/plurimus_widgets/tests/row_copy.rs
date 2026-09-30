//! A focused list box copies its rows on `Ctrl+c`: the cursor row, or every
//! checked row of a multi-select list, and with nothing to copy the key
//! reaches the list's ancestors.

use bevy_app::App;
use bevy_ecs::bundle::Bundle;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::FocusedInput;
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::ratatui_core::text::Text;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::{InputCapabilities, KeyCode, KeyModifiers};
use plurimus_test::{clipboard_writes, press_key, press_key_with, set_focus};
use plurimus_ui::{Checked, InteractionDisabled, KeyBinding, UiArea};
use plurimus_widgets::{
    ActiveDescendant, ListBoxAction, ListBoxKeys, ListBoxMultiSelect, ListItemText,
    ListItemTrailing, Marked, WidgetsPlugin, list_item, listbox,
};

const AREA: Rect = Rect::new(0, 0, 20, 6);

#[derive(Resource, Default)]
struct Propagated(Vec<Key>);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    // The legacy tier reads a modifier from the bits on the key, which is
    // all `press_key_with` sends.
    app.insert_resource(InputCapabilities::none());
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
    press_key_with(
        app,
        KeyCode::Char('c'),
        KeyModifiers::default().with_ctrl(true),
    );
}

fn propagated(app: &App) -> &[Key] {
    &app.world().resource::<Propagated>().0
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

    assert!(clipboard_writes(&mut app).is_empty());
    assert_eq!(propagated(&app), [Key::Character("c".into())]);
}

#[test]
fn an_empty_list_copies_nothing_and_propagates() {
    let mut app = app();
    spawn_list(&mut app, Vec::<()>::new());

    copy(&mut app);

    assert!(clipboard_writes(&mut app).is_empty());
    assert_eq!(propagated(&app), [Key::Character("c".into())]);
}

#[test]
fn a_disabled_list_copies_nothing_and_propagates() {
    let mut app = app();
    let (list, rows) = spawn_greek(&mut app);
    point_at(&mut app, list, rows[0]);
    app.world_mut().entity_mut(list).insert(InteractionDisabled);
    app.update();

    copy(&mut app);

    assert!(clipboard_writes(&mut app).is_empty());
    assert_eq!(propagated(&app), [Key::Character("c".into())]);
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

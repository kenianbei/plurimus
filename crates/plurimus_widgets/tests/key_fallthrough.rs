//! A bound key that changes nothing is not consumed: it reaches the
//! widget's ancestors, which is how an arrow at an edge moves focus on.

use bevy_app::App;
use bevy_ecs::bundle::Bundle;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input::keyboard::{Key, KeyboardInput};
use bevy_input_focus::FocusedInput;
use plurimus_core::ratatui_core::layout::{Constraint, Rect};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::KeyCode;
use plurimus_test::{press_key, repeat_key, set_focus};
use plurimus_ui::{UiArea, ValueChange};
use plurimus_widgets::{
    ActiveColumn, ActiveDescendant, TablePosition, TableSelection, WidgetsPlugin, list_item,
    listbox, slider, table, table_row,
};

const AREA: Rect = Rect::new(0, 0, 20, 6);

#[derive(Resource, Default)]
struct Propagated(Vec<Key>);

#[derive(Resource, Default)]
struct Changes(usize);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(20, 6));
    app.world_mut().spawn(TerminalCamera::default());
    app.init_resource::<Propagated>();
    app.init_resource::<Changes>();
    app.add_observer(|_: On<ValueChange<Entity>>, mut seen: ResMut<Changes>| seen.0 += 1);
    app.add_observer(|_: On<ValueChange<f32>>, mut seen: ResMut<Changes>| seen.0 += 1);
    app.add_observer(|_: On<ValueChange<TablePosition>>, mut seen: ResMut<Changes>| seen.0 += 1);
    app
}

/// `widget` inside a form that records every key propagating past it,
/// focused, with `rows` beneath it.
fn spawn_in_form<R: Bundle>(app: &mut App, widget: impl Bundle, rows: Vec<R>) -> Entity {
    let form = app.world_mut().spawn_empty().id();
    app.world_mut().entity_mut(form).observe(
        |input: On<FocusedInput<KeyboardInput>>, mut seen: ResMut<Propagated>| {
            if input.input.state.is_pressed() {
                seen.0.push(input.input.logical_key.clone());
            }
        },
    );
    let widget = app
        .world_mut()
        .spawn((widget, UiArea::Fixed(AREA), ChildOf(form)))
        .id();
    for row in rows {
        app.world_mut().spawn((row, ChildOf(widget)));
    }
    set_focus(app, widget);
    app.update();
    widget
}

fn spawn_table(app: &mut App, mode: TableSelection) -> Entity {
    let rows = vec![table_row(["ann", "may"]), table_row(["bo", "jun"])];
    let columns = [Constraint::Length(6), Constraint::Length(6)];
    spawn_in_form(app, (table(columns), mode), rows)
}

fn propagated(app: &App) -> &[Key] {
    &app.world().resource::<Propagated>().0
}

fn changes(app: &App) -> usize {
    app.world().resource::<Changes>().0
}

fn cursor(app: &App, widget: Entity) -> Option<Entity> {
    app.world().get::<ActiveDescendant>(widget).unwrap().0
}

#[test]
fn a_list_passes_on_what_moves_its_cursor_nowhere() {
    let mut app = app();
    let list = spawn_in_form(&mut app, listbox(), vec![list_item("a"), list_item("b")]);

    press_key(&mut app, KeyCode::Up);
    press_key(&mut app, KeyCode::Up);
    press_key(&mut app, KeyCode::End);
    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::PageDown);

    assert_eq!(
        propagated(&app),
        [Key::ArrowUp, Key::ArrowDown, Key::PageDown]
    );
    assert!(cursor(&app, list).is_some());
}

// A held Enter's repeats select nothing, and must not reach a form's
// submit either.
#[test]
fn a_list_passes_enter_on_only_while_it_has_no_cursor() {
    let mut app = app();
    spawn_in_form(&mut app, listbox(), vec![list_item("a"), list_item("b")]);

    press_key(&mut app, KeyCode::Enter);
    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Enter);
    repeat_key(&mut app, KeyCode::Enter);

    assert_eq!(propagated(&app), [Key::Enter]);
    assert_eq!(changes(&app), 1);
}

#[test]
fn a_row_table_passes_the_side_arrows_on_and_keeps_its_cursor() {
    let mut app = app();
    let table = spawn_table(&mut app, TableSelection::Row);
    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Down);
    let second = cursor(&app, table);

    press_key(&mut app, KeyCode::Left);
    press_key(&mut app, KeyCode::Right);

    assert_eq!(propagated(&app), [Key::ArrowLeft, Key::ArrowRight]);
    assert_eq!(cursor(&app, table), second);
}

#[test]
fn a_cell_table_at_its_first_cell_passes_up_and_left_on() {
    let mut app = app();
    let table = spawn_table(&mut app, TableSelection::Cell);
    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Right);

    press_key(&mut app, KeyCode::Left);
    press_key(&mut app, KeyCode::Up);

    assert_eq!(propagated(&app), [Key::ArrowLeft, Key::ArrowUp]);
    assert_eq!(app.world().get::<ActiveColumn>(table).unwrap().0, Some(0));
}

#[test]
fn a_slider_at_its_bound_passes_the_arrow_on_and_reports_nothing() {
    let mut app = app();
    spawn_in_form(&mut app, slider(0.0, 1.0, 0.0), Vec::<()>::new());

    press_key(&mut app, KeyCode::Left);
    assert_eq!(propagated(&app), [Key::ArrowLeft]);
    assert_eq!(changes(&app), 0);

    press_key(&mut app, KeyCode::Right);
    assert_eq!(propagated(&app), [Key::ArrowLeft]);
    assert_eq!(changes(&app), 1);
}

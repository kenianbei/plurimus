//! The cursor row of a container driven from a focus held somewhere else.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::ChildOf;
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::buffer::Buffer;
use plurimus_core::ratatui_core::layout::{Constraint, Rect};
use plurimus_core::ratatui_core::style::Modifier;
use plurimus_core::{CorePlugin, FrameBuffer, TerminalCamera, TerminalRenderApp, TerminalSize};
use plurimus_ui::UiArea;
use plurimus_widgets::{
    ActiveDescendant, TableSelection, WidgetsPlugin, list_item, listbox, table, table_row,
};

const WIDTH: u16 = 10;
const AREA: Rect = Rect::new(0, 0, WIDTH, 3);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(WIDTH, 3));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn frame(app: &mut App) -> Buffer {
    app.update();
    app.sub_app(TerminalRenderApp)
        .world()
        .resource::<FrameBuffer>()
        .0
        .clone()
}

// The theme's focused patch is bold; a resting row is not.
fn bold_rows(buffer: &Buffer) -> Vec<u16> {
    (0..AREA.height)
        .filter(|y| {
            buffer
                .cell((2, *y))
                .unwrap()
                .style()
                .add_modifier
                .contains(Modifier::BOLD)
        })
        .collect()
}

fn set_active(app: &mut App, container: Entity, row: Option<Entity>) {
    app.world_mut()
        .entity_mut(container)
        .get_mut::<ActiveDescendant>()
        .expect("the container keeps its cursor")
        .0 = row;
}

fn spawn_list(app: &mut App) -> (Entity, Vec<Entity>) {
    let list = app.world_mut().spawn((listbox(), UiArea::Fixed(AREA))).id();
    let rows = ["one", "two", "three"]
        .into_iter()
        .map(|label| {
            app.world_mut()
                .spawn((list_item(label), ChildOf(list)))
                .id()
        })
        .collect();
    (list, rows)
}

// `ActiveDescendant` exists so another widget can drive a list; a cursor
// nobody can see is the pattern contradicting itself.
#[test]
fn a_list_driven_from_elsewhere_still_shows_its_cursor() {
    let mut app = app();
    let (list, rows) = spawn_list(&mut app);
    let elsewhere = app.world_mut().spawn(()).id();
    app.update();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(elsewhere, FocusCause::Pressed);
    set_active(&mut app, list, Some(rows[1]));

    let buffer = frame(&mut app);

    assert_eq!(bold_rows(&buffer), vec![1]);
}

#[test]
fn a_list_with_no_cursor_highlights_nothing() {
    let mut app = app();
    let (list, _) = spawn_list(&mut app);
    let elsewhere = app.world_mut().spawn(()).id();
    app.update();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(elsewhere, FocusCause::Pressed);
    set_active(&mut app, list, None);

    let buffer = frame(&mut app);

    assert!(bold_rows(&buffer).is_empty());
}

#[test]
fn a_focused_list_is_unchanged() {
    let mut app = app();
    let (list, rows) = spawn_list(&mut app);
    app.update();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(list, FocusCause::Pressed);
    set_active(&mut app, list, Some(rows[0]));

    let buffer = frame(&mut app);

    assert_eq!(bold_rows(&buffer), vec![0]);
}

#[test]
fn a_table_driven_from_elsewhere_still_shows_its_cursor() {
    let mut app = app();
    let table_entity = app
        .world_mut()
        .spawn((
            table([Constraint::Fill(1)]),
            TableSelection::Row,
            UiArea::Fixed(AREA),
        ))
        .id();
    let rows: Vec<Entity> = ["one", "two"]
        .into_iter()
        .map(|cell| {
            app.world_mut()
                .spawn((table_row([cell]), ChildOf(table_entity)))
                .id()
        })
        .collect();
    let elsewhere = app.world_mut().spawn(()).id();
    app.update();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(elsewhere, FocusCause::Pressed);
    set_active(&mut app, table_entity, Some(rows[1]));

    let buffer = frame(&mut app);

    assert_eq!(bold_rows(&buffer), vec![1]);
}

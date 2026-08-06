//! ListBox keyboard navigation and selection flows, fully headless.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_input::{KeyCode, MouseButton, MouseKind};
use plurimus_test::{composed_frame, press_key, send_mouse};
use plurimus_ui::{Checked, ScrollArea, ScrollOffset, UiArea, UiOrder, ValueChange};
use plurimus_widgets::{
    ActiveDescendant, ListBoxMultiSelect, WidgetsPlugin, button, list_item, listbox,
    listbox_self_update,
};

#[derive(Resource, Default)]
struct Selections(Vec<(Entity, Entity)>);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize { cols: 12, rows: 4 });
    app.init_resource::<Selections>();
    app.add_observer(
        |change: On<ValueChange<Entity>>, mut log: ResMut<Selections>| {
            log.0.push((change.source, change.value));
        },
    );
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_listbox(app: &mut App) -> (Entity, [Entity; 3]) {
    let world = app.world_mut();
    let container = world
        .spawn((listbox(), UiArea::Fixed(Rect::new(0, 0, 12, 4))))
        .id();
    let items = [
        world.spawn((list_item("alpha"), ChildOf(container))).id(),
        world.spawn((list_item("beta"), ChildOf(container))).id(),
        world.spawn((list_item("gamma"), ChildOf(container))).id(),
    ];
    world
        .resource_mut::<InputFocus>()
        .set(container, FocusCause::Pressed);
    (container, items)
}

fn spawn_scrolling_listbox(app: &mut App) -> (Entity, Vec<Entity>) {
    let world = app.world_mut();
    let container = world
        .spawn((
            listbox(),
            UiArea::Fixed(Rect::new(0, 0, 12, 3)),
            ScrollArea::new(Size::new(1, 1)),
        ))
        .id();
    let items = (0..6)
        .map(|index| {
            world
                .spawn((list_item(format!("item {index}")), ChildOf(container)))
                .id()
        })
        .collect();
    world
        .resource_mut::<InputFocus>()
        .set(container, FocusCause::Pressed);
    (container, items)
}

fn active(app: &App, container: Entity) -> Option<Entity> {
    app.world().get::<ActiveDescendant>(container).unwrap().0
}

#[test]
fn keyboard_moves_active_descendant() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);

    press_key(&mut app, KeyCode::Down);
    assert_eq!(active(&app, container), Some(items[0]));
    press_key(&mut app, KeyCode::Down);
    assert_eq!(active(&app, container), Some(items[1]));
    press_key(&mut app, KeyCode::Up);
    assert_eq!(active(&app, container), Some(items[0]));
    press_key(&mut app, KeyCode::Up);
    assert_eq!(active(&app, container), Some(items[0]));
    press_key(&mut app, KeyCode::End);
    assert_eq!(active(&app, container), Some(items[2]));
    press_key(&mut app, KeyCode::Down);
    assert_eq!(active(&app, container), Some(items[2]));
    press_key(&mut app, KeyCode::Home);
    assert_eq!(active(&app, container), Some(items[0]));
}

#[test]
fn enter_selects_the_active_descendant() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);

    press_key(&mut app, KeyCode::Enter);
    assert!(app.world().resource::<Selections>().0.is_empty());

    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Enter);
    assert_eq!(
        app.world().resource::<Selections>().0,
        [(container, items[1])]
    );
}

#[test]
fn single_select_self_update_moves_checked() {
    let mut app = app();
    app.add_observer(listbox_self_update);
    let (_, items) = spawn_listbox(&mut app);

    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Enter);
    app.update();
    assert!(app.world().get::<Checked>(items[0]).is_some());

    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Enter);
    app.update();
    assert!(app.world().get::<Checked>(items[0]).is_none());
    assert!(app.world().get::<Checked>(items[1]).is_some());
}

#[test]
fn multi_select_self_update_toggles_checked() {
    let mut app = app();
    app.add_observer(listbox_self_update);
    let (container, items) = spawn_listbox(&mut app);
    app.world_mut()
        .entity_mut(container)
        .insert(ListBoxMultiSelect);

    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Enter);
    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Enter);
    app.update();
    assert!(app.world().get::<Checked>(items[0]).is_some());
    assert!(app.world().get::<Checked>(items[1]).is_some());

    press_key(&mut app, KeyCode::Up);
    press_key(&mut app, KeyCode::Enter);
    app.update();
    assert!(app.world().get::<Checked>(items[0]).is_none());
    assert!(app.world().get::<Checked>(items[1]).is_some());
}

#[test]
fn click_selects_the_clicked_row() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);

    send_mouse(&mut app, MouseKind::Moved, 2, 1);
    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), 2, 1);

    assert_eq!(active(&app, container), Some(items[1]));
    assert_eq!(
        app.world().resource::<Selections>().0,
        [(container, items[1])]
    );
}

#[test]
fn press_on_an_overlay_does_not_reach_the_listbox_beneath() {
    let mut app = app();
    let (container, _) = spawn_listbox(&mut app);
    app.world_mut().spawn((
        button("ok"),
        UiArea::Fixed(Rect::new(0, 0, 12, 4)),
        UiOrder(1),
    ));

    send_mouse(&mut app, MouseKind::Moved, 2, 1);
    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), 2, 1);

    assert_eq!(active(&app, container), None);
    assert!(app.world().resource::<Selections>().0.is_empty());
}

#[test]
fn click_resolves_rows_through_scroll_offset() {
    let mut app = app();
    let (container, items) = spawn_scrolling_listbox(&mut app);
    app.update();

    send_mouse(&mut app, MouseKind::ScrollDown, 2, 1);
    send_mouse(&mut app, MouseKind::ScrollDown, 2, 1);
    assert_eq!(
        app.world().get::<ScrollOffset>(container).unwrap().0,
        Position::new(0, 2)
    );

    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), 2, 0);
    assert_eq!(active(&app, container), Some(items[2]));
}

#[test]
fn keyboard_keeps_the_active_row_visible() {
    let mut app = app();
    let (container, items) = spawn_scrolling_listbox(&mut app);
    app.update();

    press_key(&mut app, KeyCode::End);
    assert_eq!(active(&app, container), Some(items[5]));
    assert_eq!(
        app.world().get::<ScrollOffset>(container).unwrap().0,
        Position::new(0, 3)
    );

    press_key(&mut app, KeyCode::Home);
    assert_eq!(
        app.world().get::<ScrollOffset>(container).unwrap().0,
        Position::new(0, 0)
    );
}

#[test]
fn listbox_renders_rows_and_highlight() {
    let mut app = app();
    app.add_observer(listbox_self_update);
    let (_, _) = spawn_listbox(&mut app);

    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Enter);
    press_key(&mut app, KeyCode::Down);
    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}

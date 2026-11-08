//! `ListBox` keyboard navigation and selection flows, fully headless.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input::keyboard::Key;
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::MouseButton;
use plurimus_test::{
    click, click_button, move_to, press_at, press_key, release_at, repeat_key, scroll_at,
    send_press_at, set_focus,
};
use plurimus_ui::{
    InteractionDisabled, ScrollArea, ScrollOffset, Selected, UiArea, UiOrder, ValueChange,
};
use plurimus_widgets::{
    ActiveDescendant, ListItem, ReselectListRow, WidgetsPlugin, button, list_item, listbox,
    listbox_update_selection,
};

#[derive(Resource, Default)]
struct Selections(Vec<(Entity, Entity)>);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(12, 4));
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
    set_focus(app, container);
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
    set_focus(app, container);
    (container, items)
}

fn active(app: &App, container: Entity) -> Option<Entity> {
    app.world().get::<ActiveDescendant>(container).unwrap().0
}

#[test]
fn keyboard_moves_active_descendant() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);

    press_key(&mut app, Key::ArrowDown);
    assert_eq!(active(&app, container), Some(items[0]));
    press_key(&mut app, Key::ArrowDown);
    assert_eq!(active(&app, container), Some(items[1]));
    press_key(&mut app, Key::ArrowUp);
    assert_eq!(active(&app, container), Some(items[0]));
    press_key(&mut app, Key::ArrowUp);
    assert_eq!(
        active(&app, container),
        Some(items[2]),
        "Up wraps to the last"
    );
    press_key(&mut app, Key::ArrowDown);
    assert_eq!(
        active(&app, container),
        Some(items[0]),
        "and Down to the first"
    );
    press_key(&mut app, Key::End);
    assert_eq!(active(&app, container), Some(items[2]));
    press_key(&mut app, Key::Home);
    assert_eq!(active(&app, container), Some(items[0]));
    press_key(&mut app, Key::ArrowRight);
    assert_eq!(
        active(&app, container),
        Some(items[1]),
        "the side arrows move too"
    );
}

// Upstream's keys answer a press and never a repeat.
#[test]
fn a_held_key_repeats_nothing() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);
    press_key(&mut app, Key::ArrowDown);
    press_key(&mut app, Key::Enter);
    let selections = app.world().resource::<Selections>().0.len();

    repeat_key(&mut app, Key::Enter);
    assert_eq!(
        app.world().resource::<Selections>().0.len(),
        selections,
        "a repeated Enter selects nothing further"
    );

    repeat_key(&mut app, Key::ArrowDown);
    assert_eq!(
        active(&app, container),
        Some(items[0]),
        "nor does a repeated arrow move the cursor"
    );
}

// The cursor still names the row it was on after that row stops being one,
// so selecting would report an entity the list no longer contains.
#[test]
fn selecting_a_list_with_no_rows_left_reports_nothing() {
    let mut app = app();
    let (_, items) = spawn_listbox(&mut app);
    press_key(&mut app, Key::ArrowDown);
    for item in items {
        app.world_mut().entity_mut(item).remove::<ListItem>();
    }

    press_key(&mut app, Key::Enter);

    assert!(
        app.world().resource::<Selections>().0.is_empty(),
        "no selection was reported for a row that is gone"
    );
}

// Upstream checks each row's own flag, which the disabled container writes
// down onto every row it holds; its arrows still walk the rows.
#[test]
fn a_disabled_listbox_selects_nothing() {
    let mut app = app();
    let (container, _) = spawn_listbox(&mut app);
    app.world_mut()
        .entity_mut(container)
        .insert(InteractionDisabled);

    press_key(&mut app, Key::ArrowDown);
    press_key(&mut app, Key::Enter);

    assert_eq!(app.world().resource::<Selections>().0, []);
}

#[test]
fn enter_selects_the_active_descendant() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);

    press_key(&mut app, Key::Enter);
    assert_eq!(app.world().resource::<Selections>().0, []);

    assert_eq!(
        active(&app, container),
        Some(items[0]),
        "gaining focus gave the list a cursor"
    );
    press_key(&mut app, Key::ArrowDown);
    press_key(&mut app, Key::Enter);
    assert_eq!(
        app.world().resource::<Selections>().0,
        [(container, items[1])]
    );
}

#[derive(Resource, Default)]
struct Reselections(Vec<Entity>);

#[test]
fn enter_on_the_selected_row_reselects_it() {
    let mut app = app();
    app.add_observer(listbox_update_selection);
    app.init_resource::<Reselections>();
    app.add_observer(|on: On<ReselectListRow>, mut log: ResMut<Reselections>| {
        log.0.push(on.row);
    });
    let (_, items) = spawn_listbox(&mut app);

    press_key(&mut app, Key::ArrowDown);
    press_key(&mut app, Key::Enter);
    press_key(&mut app, Key::Enter);

    assert_eq!(app.world().resource::<Selections>().0.len(), 1);
    assert_eq!(app.world().resource::<Reselections>().0, [items[0]]);
}

#[test]
fn losing_focus_clears_the_cursor_and_regaining_it_restores_the_selection() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);
    app.world_mut().entity_mut(items[2]).insert(Selected);
    app.update();
    press_key(&mut app, Key::ArrowDown);
    let elsewhere = app.world_mut().spawn(()).id();

    set_focus(&mut app, elsewhere);
    app.update();
    assert_eq!(active(&app, container), None);

    set_focus(&mut app, container);
    app.update();
    assert_eq!(active(&app, container), Some(items[2]));
}

#[test]
fn the_stock_updater_moves_selected() {
    let mut app = app();
    app.add_observer(listbox_update_selection);
    let (_, items) = spawn_listbox(&mut app);

    press_key(&mut app, Key::ArrowDown);
    press_key(&mut app, Key::Enter);
    app.update();
    assert!(app.world().get::<Selected>(items[0]).is_some());

    press_key(&mut app, Key::ArrowDown);
    press_key(&mut app, Key::Enter);
    app.update();
    assert!(app.world().get::<Selected>(items[0]).is_none());
    assert!(app.world().get::<Selected>(items[1]).is_some());
}

#[test]
fn click_selects_the_clicked_row() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);

    click(&mut app, 2, 1);

    assert_eq!(active(&app, container), Some(items[1]));
    assert_eq!(
        app.world().resource::<Selections>().0,
        [(container, items[1])],
        "once - upstream's own row click finds no row at a cell"
    );
}

#[test]
fn a_click_on_the_selected_row_reselects_and_on_a_disabled_row_selects_nothing() {
    let mut app = app();
    app.init_resource::<Reselections>();
    app.add_observer(|on: On<ReselectListRow>, mut log: ResMut<Reselections>| {
        log.0.push(on.row);
    });
    let (container, items) = spawn_listbox(&mut app);
    app.world_mut().entity_mut(items[0]).insert(Selected);
    app.world_mut()
        .entity_mut(items[2])
        .insert(InteractionDisabled);

    click(&mut app, 2, 0);
    click(&mut app, 2, 2);

    assert_eq!(app.world().resource::<Reselections>().0, [items[0]]);
    assert_eq!(app.world().resource::<Selections>().0, []);
    assert_eq!(
        active(&app, container),
        Some(items[2]),
        "a disabled row still takes the cursor"
    );
}

// The press writes the cursor before focus arrives, so focusing the list
// keeps the row the press landed on rather than restoring its first.
#[test]
fn a_press_on_an_unfocused_list_puts_the_cursor_where_it_landed() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);
    let elsewhere = app.world_mut().spawn(()).id();
    set_focus(&mut app, elsewhere);
    app.update();

    press_at(&mut app, 2, 2);
    app.update();

    assert_eq!(active(&app, container), Some(items[2]));
}

// Selecting usually closes what was clicked, and closing on the way down
// despawns the entity `bevy_picking` is still owed a release for.
#[test]
fn a_press_moves_the_cursor_without_selecting() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);

    press_at(&mut app, 2, 1);

    assert_eq!(active(&app, container), Some(items[1]));
    assert_eq!(app.world().resource::<Selections>().0, []);
}

#[test]
fn a_drag_across_rows_selects_the_one_it_ends_on() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);

    press_at(&mut app, 2, 0);
    release_at(&mut app, 2, 2);

    assert_eq!(active(&app, container), Some(items[2]));
    assert_eq!(
        app.world().resource::<Selections>().0,
        [(container, items[2])]
    );
}

// The highlight must not disagree with what letting go would select.
#[test]
fn a_held_pointer_drags_the_cursor_with_it() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);

    press_at(&mut app, 2, 0);
    assert_eq!(active(&app, container), Some(items[0]));

    move_to(&mut app, 2, 2);

    assert_eq!(active(&app, container), Some(items[2]));
    assert_eq!(app.world().resource::<Selections>().0, []);
}

#[test]
fn a_release_below_every_row_selects_nothing() {
    let mut app = app();
    spawn_listbox(&mut app);

    click(&mut app, 2, 3);

    assert_eq!(app.world().resource::<Selections>().0, []);
}

#[test]
fn press_on_an_overlay_does_not_reach_the_listbox_beneath() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);
    app.world_mut().spawn((
        button("ok"),
        UiArea::Fixed(Rect::new(0, 0, 12, 4)),
        UiOrder(1),
    ));

    press_at(&mut app, 2, 1);

    assert_ne!(active(&app, container), Some(items[1]));
    assert_eq!(app.world().resource::<Selections>().0, []);
}

#[test]
fn click_resolves_rows_through_scroll_offset() {
    let mut app = app();
    let (container, items) = spawn_scrolling_listbox(&mut app);
    app.update();

    scroll_at(&mut app, 2, 1, 1);
    scroll_at(&mut app, 2, 1, 1);
    assert_eq!(
        app.world().get::<ScrollOffset>(container).unwrap().0,
        Position::new(0, 2)
    );

    send_press_at(&mut app, 2, 0);
    assert_eq!(active(&app, container), Some(items[2]));
}

#[test]
fn keyboard_keeps_the_active_row_visible() {
    let mut app = app();
    let (container, items) = spawn_scrolling_listbox(&mut app);
    app.update();

    press_key(&mut app, Key::End);
    assert_eq!(active(&app, container), Some(items[5]));
    assert_eq!(
        app.world().get::<ScrollOffset>(container).unwrap().0,
        Position::new(0, 3)
    );

    press_key(&mut app, Key::Home);
    assert_eq!(
        app.world().get::<ScrollOffset>(container).unwrap().0,
        Position::new(0, 0)
    );
}

#[test]
fn a_secondary_click_moves_no_cursor_and_selects_nothing() {
    let mut app = app();
    let (container, _) = spawn_listbox(&mut app);
    app.update();
    let before = active(&app, container);

    click_button(&mut app, MouseButton::Right, 2, 1);

    assert_eq!(active(&app, container), before);
    assert_eq!(app.world().resource::<Selections>().0, []);
}

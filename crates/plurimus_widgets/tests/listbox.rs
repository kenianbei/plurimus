//! `ListBox` keyboard navigation and selection flows, fully headless.

use bevy_app::App;
use bevy_ecs::bundle::Bundle;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input_focus::{FocusCause, InputFocus};
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::ratatui_core::text::{Line, Text};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::{KeyCode, MouseButton, MouseKind};
use plurimus_test::{click, press_key, repeat_key, send_mouse};
use plurimus_ui::{
    Checked, InteractionDisabled, ScrollArea, ScrollOffset, UiArea, UiOrder, ValueChange,
};
use plurimus_widgets::{
    ActiveDescendant, Key, ListBoxAction, ListBoxKeys, ListBoxMultiSelect, ListItem, ListItemText,
    WidgetsPlugin, button, list_item, listbox, listbox_self_update,
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
fn the_bindings_are_the_apps_to_replace() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);
    app.world_mut()
        .entity_mut(container)
        .insert(ListBoxKeys(vec![
            (Key::Character("j".into()), ListBoxAction::Down),
            (Key::Character("k".into()), ListBoxAction::Up),
        ]));

    press_key(&mut app, KeyCode::Char('j'));
    press_key(&mut app, KeyCode::Char('j'));
    assert_eq!(active(&app, container), Some(items[1]), "j walks down");
    press_key(&mut app, KeyCode::Char('k'));
    assert_eq!(active(&app, container), Some(items[0]), "k walks up");

    press_key(&mut app, KeyCode::Down);
    assert_eq!(
        active(&app, container),
        Some(items[0]),
        "and the arrows no longer move a list that unbound them"
    );
}

#[test]
fn the_first_binding_for_a_key_is_the_one_that_wins() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);
    app.world_mut()
        .entity_mut(container)
        .insert(ListBoxKeys(vec![
            (Key::End, ListBoxAction::Down),
            (Key::End, ListBoxAction::Last),
        ]));

    press_key(&mut app, KeyCode::End);
    press_key(&mut app, KeyCode::End);
    assert_eq!(
        active(&app, container),
        Some(items[1]),
        "End moved one row down rather than to the last, so the earlier \
         binding won"
    );
}

#[test]
fn a_page_key_moves_by_the_visible_height() {
    let mut app = app();
    let (container, items) = spawn_scrolling_listbox(&mut app);
    app.update();

    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::PageDown);
    assert_eq!(
        active(&app, container),
        Some(items[3]),
        "three rows are visible, so a page is three rows down"
    );
    press_key(&mut app, KeyCode::PageUp);
    assert_eq!(active(&app, container), Some(items[0]), "and back again");

    press_key(&mut app, KeyCode::PageUp);
    assert_eq!(
        active(&app, container),
        Some(items[0]),
        "paging past the first row stops at it"
    );
}

// Autorepeat moves a cursor but must not re-select, which is the rule
// `is_activate_key` applies to every other widget.
#[test]
fn a_held_key_repeats_movement_but_not_selection() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);
    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Enter);
    let selections = app.world().resource::<Selections>().0.len();

    repeat_key(&mut app, KeyCode::Enter);
    assert_eq!(
        app.world().resource::<Selections>().0.len(),
        selections,
        "a repeated Enter selects nothing further"
    );

    repeat_key(&mut app, KeyCode::Down);
    assert_eq!(
        active(&app, container),
        Some(items[1]),
        "but a repeated arrow still moves the cursor"
    );
}

// The cursor still names the row it was on after that row stops being one,
// so selecting would report an entity the list no longer contains.
#[test]
fn selecting_a_list_with_no_rows_left_reports_nothing() {
    let mut app = app();
    let (_, items) = spawn_listbox(&mut app);
    press_key(&mut app, KeyCode::Down);
    for item in items {
        app.world_mut().entity_mut(item).remove::<ListItem>();
    }

    press_key(&mut app, KeyCode::Enter);

    assert!(
        app.world().resource::<Selections>().0.is_empty(),
        "no selection was reported for a row that is gone"
    );
}

#[test]
fn a_disabled_listbox_takes_no_keys() {
    let mut app = app();
    let (container, _) = spawn_listbox(&mut app);
    app.world_mut()
        .entity_mut(container)
        .insert(InteractionDisabled);

    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Enter);

    assert_eq!(active(&app, container), None, "no cursor moved");
    assert!(
        app.world().resource::<Selections>().0.is_empty(),
        "and nothing was selected"
    );
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

    click(&mut app, 2, 1);

    assert_eq!(active(&app, container), Some(items[1]));
    assert_eq!(
        app.world().resource::<Selections>().0,
        [(container, items[1])]
    );
}

// Selecting usually closes what was clicked, and closing on the way down
// despawns the entity the pointer router is still owed a release for.
#[test]
fn a_press_moves_the_cursor_without_selecting() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);

    send_mouse(&mut app, MouseKind::Moved, 2, 1);
    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), 2, 1);

    assert_eq!(active(&app, container), Some(items[1]));
    assert!(app.world().resource::<Selections>().0.is_empty());
}

#[test]
fn a_drag_across_rows_selects_the_one_it_ends_on() {
    let mut app = app();
    let (container, items) = spawn_listbox(&mut app);

    send_mouse(&mut app, MouseKind::Moved, 2, 0);
    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), 2, 0);
    send_mouse(&mut app, MouseKind::Up(MouseButton::Left), 2, 2);

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

    send_mouse(&mut app, MouseKind::Moved, 2, 0);
    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), 2, 0);
    assert_eq!(active(&app, container), Some(items[0]));

    send_mouse(&mut app, MouseKind::Drag(MouseButton::Left), 2, 2);

    assert_eq!(active(&app, container), Some(items[2]));
    assert!(app.world().resource::<Selections>().0.is_empty());
}

#[test]
fn a_release_below_every_row_selects_nothing() {
    let mut app = app();
    let (container, _) = spawn_listbox(&mut app);

    click(&mut app, 2, 3);

    assert!(app.world().resource::<Selections>().0.is_empty());
    assert_eq!(active(&app, container), None);
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

fn spawn_tall_listbox(app: &mut App) -> (Entity, [Entity; 3]) {
    let world = app.world_mut();
    let container = world
        .spawn((
            listbox(),
            UiArea::Fixed(Rect::new(0, 0, 12, 4)),
            ScrollArea::new(Size::new(1, 1)),
        ))
        .id();
    let items = [
        world.spawn((tall_item("one", 2), ChildOf(container))).id(),
        world.spawn((tall_item("two", 3), ChildOf(container))).id(),
        world.spawn((list_item("three"), ChildOf(container))).id(),
    ];
    world
        .resource_mut::<InputFocus>()
        .set(container, FocusCause::Pressed);
    (container, items)
}

fn tall_item(label: &str, lines: usize) -> impl Bundle {
    let text = Text::from(vec![Line::from(label.to_owned()); lines]);
    (list_item(label.to_owned()), ListItemText(text))
}

fn extent(app: &App, container: Entity) -> u16 {
    app.world()
        .get::<ScrollArea>(container)
        .expect("a scroll area")
        .content_size
        .height
}

#[test]
fn the_scroll_extent_sums_row_heights() {
    let mut app = app();
    let (container, _) = spawn_tall_listbox(&mut app);
    app.update();

    assert_eq!(
        extent(&app, container),
        6,
        "two plus three plus a single-line row"
    );
}

#[test]
fn editing_a_rows_text_resizes_the_extent() {
    let mut app = app();
    let (container, items) = spawn_tall_listbox(&mut app);
    app.update();

    app.world_mut()
        .entity_mut(items[0])
        .insert(ListItemText(Text::from(vec![Line::from("one"); 4])));
    app.update();

    assert_eq!(
        extent(&app, container),
        8,
        "the row grew by two without the children changing"
    );
}

#[test]
fn a_click_anywhere_in_a_tall_row_selects_it() {
    let mut app = app();
    let (container, items) = spawn_tall_listbox(&mut app);
    app.update();

    for (line, expected) in [(0, items[0]), (1, items[0]), (2, items[1])] {
        send_mouse(&mut app, MouseKind::Down(MouseButton::Left), 2, line);
        assert_eq!(
            active(&app, container),
            Some(expected),
            "the click on line {line} lands in the row that spans it"
        );
    }
}

#[test]
fn a_tall_row_is_revealed_whole() {
    let mut app = app();
    let (container, _) = spawn_tall_listbox(&mut app);
    app.update();

    press_key(&mut app, KeyCode::Down);
    press_key(&mut app, KeyCode::Down);
    app.update();

    // The cursor is on the three-line row spanning lines 2 to 4, and the
    // view is 4 tall: its first line is already visible, so anything that
    // reveals less than the whole row scrolls nowhere.
    assert_eq!(
        app.world().get::<ScrollOffset>(container).unwrap().0,
        Position::new(0, 1),
        "the whole row was revealed, not just its first line"
    );
}

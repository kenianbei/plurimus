//! `ListBox` rows taller than one line: the extent, the click and the reveal
//! measure by height rather than by count.

use bevy_app::App;
use bevy_ecs::bundle::Bundle;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::ChildOf;
use bevy_input::keyboard::Key;
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::ratatui_core::text::{Line, Text};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_test::{press_key, send_press_at, set_focus};
use plurimus_ui::{ScrollArea, ScrollOffset, UiArea};
use plurimus_widgets::{
    ActiveDescendant, ListItem, ListItemText, WidgetsPlugin, list_item, listbox,
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(12, 4));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn active(app: &App, container: Entity) -> Option<Entity> {
    app.world().get::<ActiveDescendant>(container).unwrap().0
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
    set_focus(app, container);
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

// `Changed` never fires for a component that goes, so these reach the extent
// only through the removal forwarding.
#[test]
fn a_row_losing_its_text_or_its_row_resizes_the_extent() {
    let mut app = app();
    let (container, items) = spawn_tall_listbox(&mut app);
    app.update();

    app.world_mut()
        .entity_mut(items[0])
        .remove::<ListItemText>();
    app.update();
    assert_eq!(extent(&app, container), 5, "the two-line row became one");

    app.world_mut().entity_mut(items[1]).remove::<ListItem>();
    app.update();
    assert_eq!(
        extent(&app, container),
        2,
        "the three-line child stopped being a row"
    );
}

#[test]
fn a_click_anywhere_in_a_tall_row_selects_it() {
    let mut app = app();
    let (container, items) = spawn_tall_listbox(&mut app);
    app.update();

    for (line, expected) in [(0, items[0]), (1, items[0]), (2, items[1])] {
        send_press_at(&mut app, 2, line);
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

    press_key(&mut app, Key::ArrowDown);
    app.update();

    // Focus put the cursor on the first row, so one step lands it on the
    // three-line row spanning lines 2 to 4, and the view is 4 tall: its first
    // line is already visible, so anything that reveals less than the whole
    // row scrolls nowhere.
    assert_eq!(
        app.world().get::<ScrollOffset>(container).unwrap().0,
        Position::new(0, 1),
        "the whole row was revealed, not just its first line"
    );
}

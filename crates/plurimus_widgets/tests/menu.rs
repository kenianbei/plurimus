//! Menu open/close, keyboard, and focus-loss flows, fully headless: the
//! model is upstream's, answered by the crate's popup driver.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input_focus::InputFocus;
use bevy_input_focus::tab_navigation::TabGroup;
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_core::{Hidden, Propagate};
use plurimus_term::KeyModifiers;
use plurimus_test::{
    click, composed_frame, press_at, press_key, press_key_with, scroll_at, write_key, write_repeat,
};
use plurimus_ui::Key;
use plurimus_ui::{
    ComputedWidgetArea, Hovered, InteractionDisabled, Pressed, ScrollArea, ScrollOffset, UiArea,
    UiWidget,
};
use plurimus_widgets::ratatui_widgets::paragraph::Paragraph;
use plurimus_widgets::{Activate, WidgetsPlugin, button, menu_button, menu_item, menu_popup};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(20, 8));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

struct Menu {
    button: Entity,
    popup: Entity,
    items: Vec<Entity>,
}

fn spawn_menu(app: &mut App) -> Menu {
    spawn_menu_of(app, &["Open", "Save"])
}

/// A menu of `labels`. Two rows is the ordinary fixture; three is what the
/// direction assertions need, since with two, stepping either way lands on
/// the same row.
fn spawn_menu_of(app: &mut App, labels: &[&'static str]) -> Menu {
    let world = app.world_mut();
    let button = world
        .spawn((menu_button("File"), UiArea::Fixed(Rect::new(1, 0, 8, 1))))
        .id();
    let popup = world.spawn((menu_popup(button), ChildOf(button))).id();
    let items = labels
        .iter()
        .map(|label| world.spawn((menu_item(*label), ChildOf(popup))).id())
        .collect();
    Menu {
        button,
        popup,
        items,
    }
}

fn focused(app: &App) -> Option<Entity> {
    app.world().resource::<InputFocus>().get()
}

fn is_open(app: &App, menu: &Menu) -> bool {
    !app.world()
        .entity(menu.popup)
        .contains::<Propagate<Hidden>>()
}

fn spawn_scroller(app: &mut App) -> Entity {
    app.world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("content")),
            UiArea::Fixed(Rect::new(0, 1, 20, 6)),
            ScrollArea::new(Size::new(20, 40)),
        ))
        .id()
}

fn offset_of(app: &App, scroller: Entity) -> Position {
    app.world().get::<ScrollOffset>(scroller).unwrap().0
}

// A tick moves no focus, and the menu is open while focus is inside it.
#[test]
fn wheel_outside_an_open_menu_scrolls_and_leaves_it_open() {
    let mut app = app();
    let menu = spawn_menu(&mut app);
    let scroller = spawn_scroller(&mut app);
    click(&mut app, 2, 0);
    assert!(is_open(&app, &menu));

    scroll_at(&mut app, 15, 6, 1);

    assert!(is_open(&app, &menu));
    assert_eq!(offset_of(&app, scroller), Position::new(0, 1));
}

#[test]
fn wheel_over_an_open_menu_scrolls_nothing_and_keeps_it_open() {
    let mut app = app();
    let menu = spawn_menu(&mut app);
    let scroller = spawn_scroller(&mut app);
    click(&mut app, 2, 0);

    scroll_at(&mut app, 2, 1, 1);

    assert!(is_open(&app, &menu), "a tick on the menu leaves it open");
    assert_eq!(offset_of(&app, scroller), Position::new(0, 0));
}

#[test]
fn wheel_scrolls_normally_with_no_menu_open() {
    let mut app = app();
    spawn_menu(&mut app);
    let scroller = spawn_scroller(&mut app);

    scroll_at(&mut app, 15, 6, 1);

    assert_eq!(offset_of(&app, scroller), Position::new(0, 1));
}

#[test]
fn click_toggles_menu_and_focuses_first_item() {
    let mut app = app();
    let menu = spawn_menu(&mut app);
    app.update();
    assert!(!is_open(&app, &menu));

    click(&mut app, 2, 0);
    assert!(is_open(&app, &menu));
    assert_eq!(focused(&app), Some(menu.items[0]));

    click(&mut app, 2, 0);
    assert!(!is_open(&app, &menu));
    assert_eq!(focused(&app), Some(menu.button));
}

#[derive(Resource, Default)]
struct Activated(Vec<Entity>);

#[test]
fn item_click_emits_activate_and_closes() {
    let mut app = app();
    app.init_resource::<Activated>();
    let menu = spawn_menu(&mut app);
    for &item in &menu.items {
        app.world_mut().entity_mut(item).observe(
            |activate: On<Activate>, mut activated: ResMut<Activated>| {
                activated.0.push(activate.entity);
            },
        );
    }
    click(&mut app, 2, 0);
    app.update();

    click(&mut app, 3, 2);
    assert_eq!(app.world().resource::<Activated>().0, vec![menu.items[0]]);
    assert!(!is_open(&app, &menu));
    assert_eq!(focused(&app), Some(menu.button), "and focus went home");
}

#[test]
fn a_press_on_an_item_focuses_it_and_keeps_the_menu_open() {
    let mut app = app();
    let menu = spawn_menu(&mut app);
    click(&mut app, 2, 0);
    app.update();

    press_at(&mut app, 3, 3);

    assert_eq!(focused(&app), Some(menu.items[1]));
    assert!(is_open(&app, &menu));
}

// The frame has no TabIndex, and its first above is the button: without
// the popup stopping the walk, the press would focus the button and the
// focus leaving the items would close the menu.
#[test]
fn a_press_on_the_popup_frame_keeps_the_menu_open() {
    let mut app = app();
    let menu = spawn_menu(&mut app);
    click(&mut app, 2, 0);
    app.update();
    let frame = app.world().get::<ComputedWidgetArea>(menu.popup).unwrap().0;

    click(&mut app, frame.x, frame.y);
    app.update();

    assert!(is_open(&app, &menu));
    assert_eq!(focused(&app), Some(menu.items[0]));
}

#[test]
fn up_on_the_button_opens_at_the_last_item() {
    let mut app = app();
    let menu = spawn_menu_of(&mut app, &["Open", "Save", "Quit"]);
    app.update();
    plurimus_test::set_focus(&mut app, menu.button);

    press_key(&mut app, Key::ArrowUp);

    assert!(is_open(&app, &menu));
    assert_eq!(focused(&app), Some(menu.items[2]));
}

#[test]
fn arrows_wrap_enter_activates_escape_closes() {
    let mut app = app();
    app.init_resource::<Activated>();
    let menu = spawn_menu(&mut app);
    let items = &menu.items;
    app.world_mut().entity_mut(items[1]).observe(
        |activate: On<Activate>, mut activated: ResMut<Activated>| {
            activated.0.push(activate.entity);
        },
    );
    click(&mut app, 2, 0);

    press_key(&mut app, Key::ArrowDown);
    assert_eq!(focused(&app), Some(items[1]));
    press_key(&mut app, Key::ArrowDown);
    assert_eq!(focused(&app), Some(items[0]));
    press_key(&mut app, Key::ArrowUp);
    assert_eq!(focused(&app), Some(items[1]));

    press_key(&mut app, Key::Enter);
    assert_eq!(app.world().resource::<Activated>().0, vec![items[1]]);
    assert!(!is_open(&app, &menu));

    click(&mut app, 2, 0);
    assert!(is_open(&app, &menu));
    press_key(&mut app, Key::Escape);
    assert!(!is_open(&app, &menu));
    assert_eq!(focused(&app), Some(menu.button));
}

// The popup is a modal tab group, so Tab cycles its items either way and
// never leaves the menu open behind focus elsewhere.
#[test]
fn tab_either_way_cycles_the_items_of_the_open_menu() {
    let mut app = app();
    let group = app.world_mut().spawn(TabGroup::new(0)).id();
    app.world_mut().spawn((
        button("other"),
        UiArea::Fixed(Rect::new(10, 0, 6, 1)),
        ChildOf(group),
    ));
    let menu = spawn_menu(&mut app);
    app.world_mut()
        .entity_mut(menu.button)
        .insert(ChildOf(group));

    click(&mut app, 2, 0);
    assert!(is_open(&app, &menu));
    for shift in [false, true] {
        if shift {
            press_key_with(&mut app, Key::Tab, KeyModifiers::none().with_shift(true));
        } else {
            press_key(&mut app, Key::Tab);
        }
        assert!(is_open(&app, &menu), "shift {shift}: open");
        let expected = if shift { menu.items[0] } else { menu.items[1] };
        assert_eq!(focused(&app), Some(expected), "shift {shift}");
    }
}

// A disabled row is inert to Enter, which propagates: what it reaches next
// is the menu button, whose own Enter toggles the menu shut. So the row
// activates nothing, and the menu closes around it.
#[test]
fn a_disabled_item_activates_nothing() {
    let mut app = app();
    app.init_resource::<Activated>();
    let menu = spawn_menu(&mut app);
    for &item in &menu.items {
        app.world_mut().entity_mut(item).observe(
            |activate: On<Activate>, mut activated: ResMut<Activated>| {
                activated.0.push(activate.entity);
            },
        );
    }
    click(&mut app, 2, 0);
    app.world_mut()
        .entity_mut(menu.items[0])
        .insert(InteractionDisabled);
    app.update();

    press_key(&mut app, Key::Enter);
    assert_eq!(app.world().resource::<Activated>().0, [] as [Entity; 0]);
    assert!(!is_open(&app, &menu));
}

// A press and its autorepeat in one drained batch, which is the only way a
// repeat reaches a menu item at all: activating closes the menu, and the
// close lands at the command flush after every observer in the batch has
// run. One intent commits once, so the second must not.
#[test]
fn holding_the_activation_key_commits_once() {
    let mut app = app();
    app.init_resource::<Activated>();
    let menu = spawn_menu(&mut app);
    app.world_mut().entity_mut(menu.items[0]).observe(
        |activate: On<Activate>, mut activated: ResMut<Activated>| {
            activated.0.push(activate.entity);
        },
    );
    click(&mut app, 2, 0);

    write_key(&mut app, Key::Enter);
    write_repeat(&mut app, Key::Enter);
    app.update();

    assert_eq!(app.world().resource::<Activated>().0, vec![menu.items[0]]);
}

// The stock arrows, on a menu long enough for the two directions to differ.
#[test]
fn the_default_arrows_step_each_way() {
    let mut app = app();
    let menu = spawn_menu_of(&mut app, &["Open", "Save", "Quit"]);
    click(&mut app, 2, 0);

    press_key(&mut app, Key::ArrowDown);
    assert_eq!(focused(&app), Some(menu.items[1]));
    press_key(&mut app, Key::ArrowUp);
    assert_eq!(focused(&app), Some(menu.items[0]));
    press_key(&mut app, Key::ArrowUp);
    assert_eq!(
        focused(&app),
        Some(menu.items[2]),
        "and up from the top wraps"
    );
}

// Focus leaving the menu closes it, and the press still lands.
#[test]
fn an_outside_press_closes_the_menu_and_acts() {
    let mut app = app();
    let menu = spawn_menu(&mut app);
    let bystander = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("other")),
            UiArea::Fixed(Rect::new(12, 6, 5, 1)),
            Hovered::default(),
        ))
        .id();
    click(&mut app, 2, 0);
    assert!(is_open(&app, &menu));

    press_at(&mut app, 13, 6);
    assert!(!is_open(&app, &menu));
    assert!(app.world().get::<Pressed>(bystander).is_some());
}

#[test]
fn open_menu_renders_over_content() {
    let mut app = app();
    app.world_mut().spawn((
        UiWidget::new(Paragraph::new("under under under")),
        UiArea::Fixed(Rect::new(0, 2, 20, 1)),
    ));
    spawn_menu(&mut app);
    click(&mut app, 2, 0);
    app.update();

    insta::assert_snapshot!(composed_frame(&app));
}

// An item presses and clicks by bevy's hover map, uncaptured, so a release
// that has left it is no click on it.
#[test]
fn a_release_off_an_item_activates_nothing() {
    let mut app = app();
    app.init_resource::<Activated>();
    let menu = spawn_menu(&mut app);
    app.world_mut().entity_mut(menu.items[0]).observe(
        |activate: On<Activate>, mut activated: ResMut<Activated>| {
            activated.0.push(activate.entity);
        },
    );
    click(&mut app, 2, 0);
    app.update();

    press_at(&mut app, 3, 2);
    plurimus_test::move_to(&mut app, 15, 6);
    plurimus_test::release_at(&mut app, 15, 6);

    assert_eq!(app.world().resource::<Activated>().0, [] as [Entity; 0]);
    assert!(app.world().get::<Pressed>(menu.items[0]).is_none());
}

//! Menu open timing: the opening tick must render and route like any
//! later tick.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::{On, ResMut, Resource};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_term::{MouseButton, MouseKind};
use plurimus_test::{click, composed_frame, send_mouse, write_mouse};
use plurimus_ui::UiArea;
use plurimus_widgets::{Activate, MenuOpen, WidgetsPlugin, menu_button, menu_item, menu_popup};

#[derive(Resource, Default)]
struct Activations(usize);

struct Menu {
    popup: Entity,
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(20, 8));
    app.init_resource::<Activations>();
    app.add_observer(|_: On<Activate>, mut log: ResMut<Activations>| log.0 += 1);
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_menu(app: &mut App) -> Menu {
    let world = app.world_mut();
    let button = world
        .spawn((menu_button("File"), UiArea::Fixed(Rect::new(1, 0, 8, 1))))
        .id();
    let popup = world.spawn((menu_popup(button), ChildOf(button))).id();
    world.spawn((menu_item("Open"), ChildOf(popup)));
    Menu { popup }
}

fn is_open(app: &App, menu: &Menu) -> bool {
    app.world().entity(menu.popup).contains::<MenuOpen>()
}

fn activations(app: &App) -> usize {
    app.world().resource::<Activations>().0
}

#[test]
fn the_opening_tick_renders_the_popup_in_place() {
    let mut app = app();
    let menu = spawn_menu(&mut app);
    app.update();
    click(&mut app, 2, 0);
    assert!(is_open(&app, &menu));
    let open_tick = composed_frame(&app);
    app.update();
    let settled = composed_frame(&app);
    assert_eq!(
        open_tick, settled,
        "the opening tick must already show the placed popup"
    );
}

#[test]
fn a_hidden_popup_is_not_interactive_at_its_placed_rect() {
    let mut app = app();
    let menu = spawn_menu(&mut app);
    app.update();
    app.update();
    click(&mut app, 3, 2);
    assert!(!is_open(&app, &menu), "nothing opened");
    assert_eq!(activations(&app), 0, "the closed menu's item cannot fire");
}

#[test]
fn a_same_batch_item_click_lands_after_the_open() {
    let mut app = app();
    let menu = spawn_menu(&mut app);
    app.update();
    send_mouse(&mut app, MouseKind::Moved, 2, 0);
    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), 2, 0);
    write_mouse(&mut app, MouseKind::Up(MouseButton::Left), 2, 0);
    write_mouse(&mut app, MouseKind::Moved, 3, 2);
    write_mouse(&mut app, MouseKind::Down(MouseButton::Left), 3, 2);
    write_mouse(&mut app, MouseKind::Up(MouseButton::Left), 3, 2);
    app.update();
    app.update();
    assert_eq!(activations(&app), 2, "button opened, then the item fired");
    assert!(
        !is_open(&app, &menu),
        "the item's activation closed the menu"
    );
}

#[test]
fn a_same_batch_press_after_a_dismissal_cannot_reach_the_closing_menu() {
    let mut app = app();
    let menu = spawn_menu(&mut app);
    app.update();
    click(&mut app, 2, 0);
    assert!(is_open(&app, &menu));
    write_mouse(&mut app, MouseKind::Moved, 15, 6);
    write_mouse(&mut app, MouseKind::Down(MouseButton::Left), 15, 6);
    write_mouse(&mut app, MouseKind::Up(MouseButton::Left), 15, 6);
    write_mouse(&mut app, MouseKind::Moved, 3, 2);
    write_mouse(&mut app, MouseKind::Down(MouseButton::Left), 3, 2);
    write_mouse(&mut app, MouseKind::Up(MouseButton::Left), 3, 2);
    app.update();
    app.update();
    assert!(!is_open(&app, &menu), "the outside press dismissed");
    assert_eq!(activations(&app), 1, "only the opening click activated");
}

#[test]
fn a_deferred_batch_with_a_second_flip_converges() {
    let mut app = app();
    let menu = spawn_menu(&mut app);
    app.update();
    // One batch: open, dismiss outside, open again.
    send_mouse(&mut app, MouseKind::Moved, 2, 0);
    for (x, y) in [(2, 0), (15, 6), (2, 0)] {
        write_mouse(&mut app, MouseKind::Moved, x, y);
        write_mouse(&mut app, MouseKind::Down(MouseButton::Left), x, y);
        write_mouse(&mut app, MouseKind::Up(MouseButton::Left), x, y);
    }
    for _ in 0..4 {
        app.update();
    }
    assert!(is_open(&app, &menu), "the trailing click reopened the menu");
}

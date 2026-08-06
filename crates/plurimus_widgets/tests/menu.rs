//! Menu open/close, keyboard, and dismissal flows, fully headless.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ChildOf, On, ResMut, Resource};
use bevy_input_focus::InputFocus;
use plurimus_core::ratatui_core::layout::{Position, Rect, Size};
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize};
use plurimus_input::{KeyCode, MouseButton, MouseKind};
use plurimus_test::{click, composed_frame, press_key, send_mouse};
use plurimus_ui::{Hovered, Pressed, ScrollArea, ScrollOffset, UiArea, UiWidget};
use plurimus_widgets::ratatui_widgets::paragraph::Paragraph;
use plurimus_widgets::{Activate, MenuOpen, WidgetsPlugin, menu_button, menu_item, menu_popup};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize { cols: 20, rows: 8 });
    app.world_mut().spawn(TerminalCamera::default());
    app
}

struct Menu {
    button: Entity,
    popup: Entity,
    items: [Entity; 2],
}

fn spawn_menu(app: &mut App) -> Menu {
    let world = app.world_mut();
    let button = world
        .spawn((menu_button("File"), UiArea::Fixed(Rect::new(1, 0, 8, 1))))
        .id();
    let popup = world.spawn((menu_popup(button), ChildOf(button))).id();
    let items = [
        world.spawn((menu_item("Open"), ChildOf(popup))).id(),
        world.spawn((menu_item("Save"), ChildOf(popup))).id(),
    ];
    Menu {
        button,
        popup,
        items,
    }
}

fn is_open(app: &App, menu: &Menu) -> bool {
    app.world().get::<MenuOpen>(menu.popup).is_some()
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

#[test]
fn wheel_outside_an_open_menu_dismisses_without_scrolling() {
    let mut app = app();
    let menu = spawn_menu(&mut app);
    let scroller = spawn_scroller(&mut app);
    click(&mut app, 2, 0);
    assert!(is_open(&app, &menu));

    send_mouse(&mut app, MouseKind::ScrollDown, 15, 6);

    assert!(!is_open(&app, &menu), "an outside tick dismisses");
    assert_eq!(offset_of(&app, scroller), Position::new(0, 0));
}

#[test]
fn wheel_over_an_open_menu_scrolls_nothing_and_keeps_it_open() {
    let mut app = app();
    let menu = spawn_menu(&mut app);
    let scroller = spawn_scroller(&mut app);
    click(&mut app, 2, 0);

    send_mouse(&mut app, MouseKind::ScrollDown, 2, 1);

    assert!(is_open(&app, &menu), "a tick on the menu leaves it open");
    assert_eq!(offset_of(&app, scroller), Position::new(0, 0));
}

#[test]
fn wheel_scrolls_normally_with_no_menu_open() {
    let mut app = app();
    spawn_menu(&mut app);
    let scroller = spawn_scroller(&mut app);

    send_mouse(&mut app, MouseKind::ScrollDown, 15, 6);

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
    assert_eq!(
        app.world().resource::<InputFocus>().get(),
        Some(menu.items[0])
    );

    click(&mut app, 2, 0);
    assert!(!is_open(&app, &menu));
    assert_eq!(
        app.world().resource::<InputFocus>().get(),
        Some(menu.button)
    );
}

#[derive(Resource, Default)]
struct Activated(Vec<Entity>);

#[test]
fn item_click_emits_activate_and_closes() {
    let mut app = app();
    app.init_resource::<Activated>();
    let menu = spawn_menu(&mut app);
    for item in menu.items {
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
}

#[test]
fn arrows_wrap_enter_activates_escape_closes() {
    let mut app = app();
    app.init_resource::<Activated>();
    let menu = spawn_menu(&mut app);
    let items = menu.items;
    app.world_mut().entity_mut(items[1]).observe(
        |activate: On<Activate>, mut activated: ResMut<Activated>| {
            activated.0.push(activate.entity);
        },
    );
    click(&mut app, 2, 0);

    press_key(&mut app, KeyCode::Down);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(items[1]));
    press_key(&mut app, KeyCode::Down);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(items[0]));
    press_key(&mut app, KeyCode::Up);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(items[1]));

    press_key(&mut app, KeyCode::Enter);
    assert_eq!(app.world().resource::<Activated>().0, vec![items[1]]);
    assert!(!is_open(&app, &menu));

    click(&mut app, 2, 0);
    assert!(is_open(&app, &menu));
    press_key(&mut app, KeyCode::Esc);
    assert!(!is_open(&app, &menu));
    assert_eq!(
        app.world().resource::<InputFocus>().get(),
        Some(menu.button)
    );
}

#[test]
fn outside_click_dismisses_and_swallows() {
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

    send_mouse(&mut app, MouseKind::Moved, 13, 6);
    send_mouse(&mut app, MouseKind::Down(MouseButton::Left), 13, 6);
    assert!(!is_open(&app, &menu));
    assert!(app.world().get::<Pressed>(bystander).is_none());
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

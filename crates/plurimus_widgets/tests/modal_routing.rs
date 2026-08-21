//! What a pointer inside an open modal overlay may reach.
//!
//! The guard answers "inside" from the overlay's own rect, so an overlay
//! confines the pointer to its subtree instead of relying on every child
//! being marked as modal.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::{On, ResMut, Resource};
use plurimus_core::ratatui_core::layout::Rect;
use plurimus_core::{CorePlugin, TerminalCamera, TerminalSize, UiWidget};
use plurimus_test::click;
use plurimus_ui::{ComputedWidgetArea, Hovered, PointerPress, UiArea};
use plurimus_widgets::ratatui_widgets::paragraph::Paragraph;
use plurimus_widgets::{MenuOpen, WidgetsPlugin, menu_button, menu_item, menu_popup};

#[derive(Resource, Default)]
struct Presses(Vec<Entity>);

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetsPlugin));
    app.insert_resource(TerminalSize::new(20, 8));
    app.init_resource::<Presses>();
    app.add_observer(|press: On<PointerPress>, mut log: ResMut<Presses>| log.0.push(press.entity));
    app.world_mut().spawn(TerminalCamera::default());
    app
}

fn spawn_menu(app: &mut App) -> Entity {
    let world = app.world_mut();
    let button = world
        .spawn((menu_button("File"), UiArea::Fixed(Rect::new(1, 0, 8, 1))))
        .id();
    let popup = world.spawn((menu_popup(button), ChildOf(button))).id();
    world.spawn((menu_item("Open"), ChildOf(popup)));
    world.spawn((menu_item("Save"), ChildOf(popup)));
    popup
}

// Everything the popup covers, so a press falling through the overlay has
// somewhere to land.
fn spawn_beneath(app: &mut App) -> Entity {
    app.world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("beneath")),
            UiArea::Fixed(Rect::new(0, 1, 20, 7)),
            Hovered::default(),
        ))
        .id()
}

fn is_open(app: &App, popup: Entity) -> bool {
    app.world().entity(popup).contains::<MenuOpen>()
}

fn popup_area(app: &App, popup: Entity) -> Rect {
    app.world().get::<ComputedWidgetArea>(popup).unwrap().0
}

fn was_pressed(app: &App, entity: Entity) -> bool {
    app.world().resource::<Presses>().0.contains(&entity)
}

// The popup carries `ModalityToggle` but no `Hovered`, so it never wins
// press arbitration: before the guard hit-tested geometry, a press on its
// own border resolved to whatever sat beneath and dismissed the menu.
#[test]
fn a_press_on_the_popup_frame_keeps_the_menu_open() {
    let mut app = app();
    let popup = spawn_menu(&mut app);
    let beneath = spawn_beneath(&mut app);
    click(&mut app, 2, 0);
    assert!(is_open(&app, popup));
    // The popup's rect settles the frame after it stops being hidden,
    // which is the frame the toggle click's deferral already waits for.
    app.update();
    let frame = popup_area(&app, popup);

    click(&mut app, frame.x, frame.y);

    assert!(is_open(&app, popup), "the frame is not a dismiss button");
    assert!(
        !was_pressed(&app, beneath),
        "and nothing beneath was pressed"
    );
}

#[test]
fn a_press_inside_the_popup_routes_to_an_unmarked_child() {
    let mut app = app();
    let popup = spawn_menu(&mut app);
    let beneath = spawn_beneath(&mut app);
    click(&mut app, 2, 0);
    app.update();
    let frame = popup_area(&app, popup);
    let footer = Rect::new(frame.x, frame.y + frame.height - 1, frame.width, 1);
    let child = app
        .world_mut()
        .spawn((
            UiWidget::new(Paragraph::new("row")),
            UiArea::Fixed(footer),
            Hovered::default(),
            ChildOf(popup),
        ))
        .id();
    app.update();

    click(&mut app, footer.x, footer.y);

    assert!(was_pressed(&app, child), "an unmarked child still routes");
    assert!(is_open(&app, popup), "and pressing it dismissed nothing");
    assert!(!was_pressed(&app, beneath));
}
